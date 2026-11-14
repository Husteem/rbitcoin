//! Core's `mempool_sequence`, and the recent removals a client can match
//! against two listings.

use bitcoin::Txid;
use std::collections::VecDeque;

/// Why a transaction left the pool: Core's `MemPoolRemovalReason`.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum RemovalReason {
    /// Older than `-mempoolexpiry`.
    Expiry,
    /// A worst chunk dropped for a slot or the weight budget.
    SizeLimit,
    /// No longer valid at the tip after a disconnect.
    Reorg,
    /// Mined in a connected block.
    Block,
    /// Spends an outpoint a connected block spent.
    Conflict,
    /// A conflicting admit took its place.
    Replaced { by: Txid },
}

impl RemovalReason {
    /// Core's name for the reason.
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Expiry => "expiry",
            Self::SizeLimit => "sizelimit",
            Self::Reorg => "reorg",
            Self::Block => "block",
            Self::Conflict => "conflict",
            Self::Replaced { .. } => "replaced",
        }
    }
}

/// What one removal tells a client. Either way it is a change, and the
/// sequence advances.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Removal {
    /// A committed removal. It writes one row with its reason.
    Committed(RemovalReason),
    /// Takes back what the node staged: a rolled-back package or 1p1c
    /// parent, a `testmempoolaccept` dry-run admit, or a record dropped at
    /// open before the pool is served. It writes no row.
    Undo,
}

/// One committed removal and the sequence it took.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct RemovedRow {
    pub seq: u64,
    pub txid: Txid,
    pub reason: RemovalReason,
}

/// The rows still held, oldest first. Every committed removal numbered
/// `oldest` or later is in `rows`.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RemovedRows {
    pub oldest: u64,
    pub rows: Vec<RemovedRow>,
}

/// The smallest transaction the pool admits: one input with an empty
/// scriptSig, one output with an empty script, no witness. 60 bytes.
const SMALLEST_TX_WEIGHT: u64 = 240;

/// Rows held. A maximal block of the smallest transactions removes
/// `MAX_BLOCK_WEIGHT / SMALLEST_TX_WEIGHT` (16,666), the largest removal the
/// node makes in normal running. A full ring is about 1.3 MB (80-byte rows).
/// Past it the oldest row drops.
pub const REMOVED_ROWS: usize =
    (rbitcoin_consensus::block::MAX_BLOCK_WEIGHT / SMALLEST_TX_WEIGHT) as usize;

/// The value the next graph change takes, and the committed removals.
///
/// Starts at 1. An insert or a remove that commits advances it by one.
/// Records loaded at open and a compact rebuild are not changes.
pub struct PoolSequence {
    next: u64,
    oldest: u64,
    rows: VecDeque<Held>,
    /// Rows held with `replacer_gone` set. Zero on the admit path, so a
    /// commit does not scan the ring.
    replacer_gone: usize,
}

/// A row, and the sequence the removed entry's insert took. That names one
/// stay in the pool, so a retraction cannot reach another stay of the txid.
struct Held {
    row: RemovedRow,
    admitted: u64,
    /// A `replaced` row whose replacer did not stay. The row goes when its
    /// txid is back in the pool. Until then the transaction is out, and the
    /// row says what took it out.
    replacer_gone: bool,
}

impl Default for PoolSequence {
    fn default() -> Self {
        Self {
            next: 1,
            oldest: 1,
            rows: VecDeque::new(),
            replacer_gone: 0,
        }
    }
}

impl PoolSequence {
    pub fn next(&self) -> u64 {
        self.next
    }

    /// Number one committed graph change. Returns the number it took.
    pub fn advance(&mut self) -> u64 {
        let seq = self.next;
        self.next = seq.saturating_add(1);
        seq
    }

    /// Number one graph remove, and keep a row when it is committed.
    /// `admitted` is the sequence the entry's insert took.
    pub fn note_removed(&mut self, txid: Txid, admitted: u64, removal: Removal) {
        let seq = self.advance();
        let Removal::Committed(reason) = removal else {
            return;
        };
        if self.rows.len() == REMOVED_ROWS {
            if let Some(dropped) = self.rows.pop_front() {
                self.oldest = dropped.row.seq.saturating_add(1);
                self.forget(&dropped);
            }
        }
        self.rows.push_back(Held {
            row: RemovedRow { seq, txid, reason },
            admitted,
            replacer_gone: false,
        });
    }

    /// A row left the ring.
    fn forget(&mut self, gone: &Held) {
        if gone.replacer_gone {
            self.replacer_gone -= 1;
        }
    }

    /// `by` did not stay. Mark the `replaced` rows it wrote, the ones
    /// numbered in `attempt`: from the sequence when that admit of `by`
    /// began up to the one its insert took. One admit holds the pool for
    /// that whole span, so only its own victims are numbered there, and no
    /// other replacement by the same txid is. A marked row goes when its
    /// victim is back ([`Self::note_returned`]).
    pub fn note_replacer_gone(&mut self, by: Txid, attempt: std::ops::Range<u64>) {
        let mut i = self.rows.len();
        while i > 0 && self.rows[i - 1].row.seq >= attempt.start {
            i -= 1;
            let held = &mut self.rows[i];
            if held.row.seq < attempt.end
                && held.row.reason == (RemovalReason::Replaced { by })
                && !held.replacer_gone
            {
                held.replacer_gone = true;
                self.replacer_gone += 1;
            }
        }
    }

    /// `txid` is in the pool again. A replacement that did not stay
    /// replaced nothing, so its rows for `txid` go. The sequence keeps its
    /// numbers: membership did change.
    pub fn note_returned(&mut self, txid: Txid) {
        let mut i = self.rows.len();
        while i > 0 && self.replacer_gone > 0 {
            i -= 1;
            if self.rows[i].replacer_gone && self.rows[i].row.txid == txid {
                self.rows.remove(i);
                self.replacer_gone -= 1;
            }
        }
    }

    /// Drop the size-limit row for the stay of `txid` whose insert took
    /// `admitted`. An admit that a trim evicted before it committed, or
    /// before its package was rolled back, was never accepted. A block, a
    /// replacement, an expiry or a conflict that took that stay out is why
    /// it left, so that row stays. So does the row of another stay.
    pub fn retract_evicted(&mut self, txid: Txid, admitted: u64) {
        let mut i = self.rows.len();
        while i > 0 && self.rows[i - 1].row.seq > admitted {
            i -= 1;
            let held = &self.rows[i];
            if held.row.txid == txid && held.admitted == admitted {
                if held.row.reason == RemovalReason::SizeLimit {
                    self.rows.remove(i);
                }
                return;
            }
        }
    }

    pub fn removed_rows(&self) -> RemovedRows {
        RemovedRows {
            oldest: self.oldest,
            rows: self.rows.iter().map(|held| held.row).collect(),
        }
    }
}
