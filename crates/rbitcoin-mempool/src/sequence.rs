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
}

/// A row, and the sequence the removed entry's insert took. That names one
/// stay in the pool, so a retraction cannot reach another stay of the txid.
struct Held {
    row: RemovedRow,
    admitted: u64,
}

impl Default for PoolSequence {
    fn default() -> Self {
        Self {
            next: 1,
            oldest: 1,
            rows: VecDeque::new(),
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
            }
        }
        self.rows.push_back(Held {
            row: RemovedRow { seq, txid, reason },
            admitted,
        });
    }

    /// Drop the newest `replaced` row by `by` for each of `victims`. That
    /// replacement did not stay, so it replaced nothing. The sequence keeps
    /// its numbers: membership did change.
    pub fn retract_replaced<'a>(&mut self, by: Txid, victims: impl IntoIterator<Item = &'a Txid>) {
        let mut left: Vec<Txid> = victims.into_iter().copied().collect();
        let mut i = self.rows.len();
        while i > 0 && !left.is_empty() {
            i -= 1;
            let row = self.rows[i].row;
            if row.reason != (RemovalReason::Replaced { by }) {
                continue;
            }
            if let Some(k) = left.iter().position(|v| *v == row.txid) {
                left.swap_remove(k);
                self.rows.remove(i);
            }
        }
    }

    /// Drop the row for the stay of `txid` whose insert took `admitted`. An
    /// admit that left before it committed, or that a rollback takes back,
    /// was never in the pool. A later stay of the same txid keeps its row.
    pub fn retract_admit(&mut self, txid: Txid, admitted: u64) {
        let mut i = self.rows.len();
        while i > 0 && self.rows[i - 1].row.seq > admitted {
            i -= 1;
            if self.rows[i].row.txid == txid && self.rows[i].admitted == admitted {
                self.rows.remove(i);
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
