//! Global transaction INV rate limiting (Core `#34628`).
//!
//! One count bucket and one size bucket are shared by every peer. A released
//! transaction is queued for the direction it will be announced to. Outbound
//! connections refill both buckets at 2.5x the inbound rate. The constants and
//! floors follow Core's `InvToSendBucket`.

use bitcoin::{Txid, Wtxid};
use std::collections::{BTreeMap, HashSet, VecDeque};

/// Core `INVENTORY_BROADCAST_PER_SECOND`.
pub const INVENTORY_BROADCAST_PER_SECOND: f64 = 14.0;
/// Core `INVENTORY_BROADCAST_TARGET` (14 tx/s * the old 5 s interval).
pub const INVENTORY_BROADCAST_TARGET: f64 = INVENTORY_BROADCAST_PER_SECOND * 5.0;
/// Core `INBOUND_COUNT_SECONDS`: count bucket capacity and starting value.
pub const INBOUND_COUNT_SECONDS: f64 = 30.0;
/// Core `SIZE_INIT`: 12 MB.
pub const SIZE_INIT: f64 = 12_000_000.0;
/// Core `SIZE_CAP`: 50 MB.
pub const SIZE_CAP: f64 = 50_000_000.0;
/// Core `SIZE_REFILL`: 20 kB/s.
pub const SIZE_REFILL: f64 = 20_000.0;
/// Core count floor, `-INVENTORY_BROADCAST_TARGET`.
pub const COUNT_FLOOR: f64 = -INVENTORY_BROADCAST_TARGET;
/// Core size floor.
pub const SIZE_FLOOR: f64 = -50_000.0;
/// Core `OUTBOUND_INVENTORY_BUCKET_MULTIPLIER` (5 s / 2 s).
pub const OUTBOUND_MULTIPLIER: f64 = 2.5;
/// Core `INVENTORY_BUCKET_BACKLOG_CAPACITY`.
pub const BACKLOG_CAPACITY: usize = 300;

/// A generic token bucket. Tokens accrue at `rate` per second up to `cap`.
#[derive(Debug, Clone, Copy)]
pub struct TokenBucket {
    rate: f64,
    cap: f64,
    floor: f64,
    value: f64,
    last_ms: Option<u64>,
}

impl TokenBucket {
    pub fn new(rate: f64, value: f64, cap: f64, floor: f64) -> Self {
        Self {
            rate,
            cap,
            floor,
            value: value.min(cap),
            last_ms: None,
        }
    }

    /// Refill for the elapsed time. The first call only sets the baseline.
    pub fn refill(&mut self, now_ms: u64) {
        if let Some(last) = self.last_ms {
            if now_ms > last && self.value < self.cap {
                let dt = (now_ms - last) as f64 / 1000.0;
                self.value = (self.value + self.rate * dt).min(self.cap);
            }
        }
        self.last_ms = Some(now_ms);
    }

    pub fn value(&self) -> f64 {
        self.value
    }

    #[doc(hidden)]
    pub fn set_value(&mut self, value: f64) {
        self.value = value.min(self.cap);
    }

    /// Consume `n` tokens. `true` while the balance stays above `floor`.
    pub fn decrement(&mut self, n: f64) -> bool {
        self.value -= n;
        self.value > self.floor
    }
}

/// Which peer direction a release belongs to.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum InvDirection {
    Inbound,
    Outbound,
}

impl InvDirection {
    pub fn for_inbound(inbound: bool) -> Self {
        if inbound {
            Self::Inbound
        } else {
            Self::Outbound
        }
    }
}

/// One direction's count+size buckets and its pending wtxids.
#[derive(Debug)]
pub struct InvBucket {
    pub count: TokenBucket,
    pub size: TokenBucket,
    pub backlog: VecDeque<(Wtxid, u32)>,
    /// Wtxids already in `backlog`, so a rebroadcast or unbroadcast note does
    /// not spend a second pair of tokens on the same tx.
    queued: HashSet<Wtxid>,
}

impl InvBucket {
    pub fn new(multiplier: f64) -> Self {
        let count_cap = INVENTORY_BROADCAST_PER_SECOND * INBOUND_COUNT_SECONDS;
        Self {
            count: TokenBucket::new(
                INVENTORY_BROADCAST_PER_SECOND * multiplier,
                count_cap,
                count_cap,
                COUNT_FLOOR,
            ),
            size: TokenBucket::new(SIZE_REFILL * multiplier, SIZE_INIT, SIZE_CAP, SIZE_FLOOR),
            backlog: VecDeque::new(),
            queued: HashSet::new(),
        }
    }

    pub fn enqueue(&mut self, wtxid: Wtxid, size: u32) {
        if !self.queued.insert(wtxid) {
            return;
        }
        self.backlog.push_back((wtxid, size));
    }

    pub fn dequeue(&mut self) -> Option<(Wtxid, u32)> {
        let entry = self.backlog.pop_front()?;
        self.queued.remove(&entry.0);
        Some(entry)
    }

    #[doc(hidden)]
    pub fn set_tokens(&mut self, count: f64, size: f64) {
        self.count.set_value(count);
        self.size.set_value(size);
    }

    pub fn avail(&self) -> bool {
        !self.backlog.is_empty() && self.size.value() > 0.0 && self.count.value() > 0.0
    }

    pub fn refill(&mut self, now_ms: u64) {
        self.count.refill(now_ms);
        self.size.refill(now_ms);
    }

    /// Shrink a drained backlog that grew far past the normal capacity.
    pub fn shrink_if_idle(&mut self) {
        if self.backlog.is_empty() && self.backlog.capacity() > BACKLOG_CAPACITY {
            let mut empty = VecDeque::with_capacity(BACKLOG_CAPACITY);
            std::mem::swap(&mut self.backlog, &mut empty);
        }
    }
}

/// Both directions' buckets and the released wtxid logs consumed by sessions.
#[derive(Debug)]
pub struct InvBuckets {
    pub inbound: InvBucket,
    pub outbound: InvBucket,
    pub released_inbound: BTreeMap<u64, (Txid, Wtxid)>,
    pub released_outbound: BTreeMap<u64, (Txid, Wtxid)>,
    pub next_release: u64,
}

impl Default for InvBuckets {
    fn default() -> Self {
        Self::new()
    }
}

impl InvBuckets {
    pub fn new() -> Self {
        Self {
            inbound: InvBucket::new(1.0),
            outbound: InvBucket::new(OUTBOUND_MULTIPLIER),
            released_inbound: BTreeMap::new(),
            released_outbound: BTreeMap::new(),
            // Cursors start at 0 ("nothing read"), so the first release is 1.
            next_release: 1,
        }
    }

    pub fn bucket_mut(&mut self, dir: InvDirection) -> &mut InvBucket {
        match dir {
            InvDirection::Inbound => &mut self.inbound,
            InvDirection::Outbound => &mut self.outbound,
        }
    }

    pub fn released(&self, dir: InvDirection) -> &BTreeMap<u64, (Txid, Wtxid)> {
        match dir {
            InvDirection::Inbound => &self.released_inbound,
            InvDirection::Outbound => &self.released_outbound,
        }
    }

    fn released_mut(&mut self, dir: InvDirection) -> &mut BTreeMap<u64, (Txid, Wtxid)> {
        match dir {
            InvDirection::Inbound => &mut self.released_inbound,
            InvDirection::Outbound => &mut self.released_outbound,
        }
    }

    pub fn backlog_len(&self, dir: InvDirection) -> usize {
        match dir {
            InvDirection::Inbound => self.inbound.backlog.len(),
            InvDirection::Outbound => self.outbound.backlog.len(),
        }
    }

    pub fn push_released(&mut self, dir: InvDirection, txid: Txid, wtxid: Wtxid) {
        let seq = self.next_release;
        self.next_release = self.next_release.saturating_add(1);
        self.released_mut(dir).insert(seq, (txid, wtxid));
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use bitcoin::hashes::Hash;

    #[test]
    fn count_floor_is_the_old_broadcast_target() {
        assert_eq!(COUNT_FLOOR, -70.0);
    }

    #[test]
    fn outbound_rate_is_faster_but_capacity_is_not() {
        let inbound = InvBucket::new(1.0);
        let outbound = InvBucket::new(OUTBOUND_MULTIPLIER);
        assert_eq!(inbound.count.value(), outbound.count.value());
        assert_eq!(inbound.size.value(), outbound.size.value());
        assert!(outbound.count.rate > inbound.count.rate);
        assert!(outbound.size.rate > inbound.size.rate);
    }

    #[test]
    fn first_refill_only_sets_the_baseline() {
        let mut bucket = TokenBucket::new(14.0, 1.0, 420.0, -70.0);
        bucket.refill(1_000);
        assert_eq!(bucket.value(), 1.0);
        bucket.refill(2_000);
        assert_eq!(bucket.value(), 15.0);
        bucket.refill(100_000);
        assert_eq!(bucket.value(), 420.0);
    }

    #[test]
    fn enqueue_dedupes_a_wtxid_already_in_the_backlog() {
        let mut bucket = InvBucket::new(1.0);
        let w = Wtxid::from_byte_array([7; 32]);
        bucket.enqueue(w, 100);
        bucket.enqueue(w, 100);
        assert_eq!(bucket.backlog.len(), 1, "one tx spends one pair of tokens");
        assert_eq!(bucket.dequeue().map(|(got, _)| got), Some(w));
        bucket.enqueue(w, 100);
        assert_eq!(
            bucket.backlog.len(),
            1,
            "a later rebroadcast may re-enqueue"
        );
    }

    #[test]
    fn decrement_can_go_below_zero_down_to_the_floor() {
        let mut bucket = TokenBucket::new(14.0, 0.5, 420.0, -70.0);
        assert!(bucket.decrement(70.0));
        assert_eq!(bucket.value(), -69.5);
        assert!(!bucket.decrement(1.0));
        assert_eq!(bucket.value(), -70.5);
    }
}
