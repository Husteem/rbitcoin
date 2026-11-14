//! Core's `mempool_sequence`: one number per change to the live graph.

/// The value the next graph change takes. Starts at 1. An insert or a
/// remove that commits advances it by one. Records loaded at open and a
/// compact rebuild are not changes.
pub struct PoolSequence {
    next: u64,
}

impl Default for PoolSequence {
    fn default() -> Self {
        Self { next: 1 }
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
}
