//! Shared append bookkeeping for the locator tables.
//!
//! `create_loc` (two offsets per create) and `delta_loc` (one offset per
//! record) are the same table: a packed `loc` file, an overflow file, a
//! checkpoint file, and RAM mirrors of both. The staging shape differs only
//! in the per-row payload types, so the invariant checks, the write order,
//! and the RAM publish live here once.

use crate::delta_loc::loc_window;
use crate::error::StoreError;
use crate::file::TableFile;
use rbitcoin_primitives::Fk;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::RwLock;

/// An overflow row in either table: first field is the raw create fk.
pub(crate) trait OvfRow {
    fn fk_raw(&self) -> u64;
}

impl OvfRow for (u64, u32, u32) {
    fn fk_raw(&self) -> u64 {
        self.0
    }
}

impl OvfRow for (u64, u32) {
    fn fk_raw(&self) -> u64 {
        self.0
    }
}

/// Borrowed view of one encoded, unpublished append.
pub(crate) struct StagedLocRef<'a, O, L> {
    pub base: u64,
    pub n: u64,
    pub loc_off: u64,
    pub loc_bytes: &'a [u8],
    pub ovf_at: u64,
    pub ovf_bytes: &'a [u8],
    pub new_ovf: &'a [O],
    pub off_at: u64,
    pub off_bytes: &'a [u8],
    pub new_offs: &'a [(u64, L)],
}

/// The three locator files plus their RAM mirrors.
pub(crate) struct LocParts<'a, O, L> {
    pub loc: &'a TableFile,
    pub ovf: &'a TableFile,
    pub off: &'a TableFile,
    pub ovf_rows: &'a RwLock<Vec<O>>,
    pub checkpoints: &'a RwLock<Vec<L>>,
}

/// Check the RAM-order invariants, then write the loc, overflow, and
/// checkpoint tails in that order.
pub(crate) fn write_staged<O: OvfRow, L>(
    parts: LocParts<'_, O, L>,
    staged: &StagedLocRef<'_, O, L>,
    ovf_order_err: &'static str,
    index_err: &'static str,
) -> Result<(), StoreError> {
    if let Some(row) = staged.new_ovf.first() {
        let rows = parts.ovf_rows.read().unwrap_or_else(|e| e.into_inner());
        if let Some(last) = rows.last() {
            if row.fk_raw() <= last.fk_raw() {
                return Err(StoreError::Corrupt(ovf_order_err));
            }
        }
    }
    if let Some(&(w, _)) = staged.new_offs.first() {
        let cps = parts.checkpoints.read().unwrap_or_else(|e| e.into_inner());
        if w as usize != cps.len() {
            return Err(StoreError::Corrupt(index_err));
        }
    }
    parts
        .loc
        .write_uncommitted(staged.loc_off, staged.loc_bytes)?;
    if !staged.ovf_bytes.is_empty() {
        parts
            .ovf
            .write_uncommitted(staged.ovf_at, staged.ovf_bytes)?;
    }
    if !staged.off_bytes.is_empty() {
        parts
            .off
            .write_uncommitted(staged.off_at, staged.off_bytes)?;
    }
    Ok(())
}

/// Publish RAM for this append. Header pwrites run only while `durable` is
/// still true, one per grown file. A file whose header is behind its RAM end
/// is persisted first, including an overflow file this batch does not append.
/// A failed header pwrite returns false and skips the rest. RAM still
/// advances.
#[must_use]
pub(crate) fn commit_staged<O: OvfRow + Copy, L: Copy>(
    parts: LocParts<'_, O, L>,
    count: &AtomicU64,
    staged: StagedLocRef<'_, O, L>,
    durable: bool,
) -> bool {
    let loc_end = staged.loc_off + staged.loc_bytes.len() as u64;
    let ovf_end =
        (!staged.ovf_bytes.is_empty()).then(|| staged.ovf_at + staged.ovf_bytes.len() as u64);
    let off_end =
        (!staged.off_bytes.is_empty()).then(|| staged.off_at + staged.off_bytes.len() as u64);
    let durable = TableFile::commit_locator_file_ends(
        parts.loc, parts.ovf, parts.off, durable, loc_end, ovf_end, off_end,
    );
    if !staged.ovf_bytes.is_empty() {
        parts
            .ovf_rows
            .write()
            .unwrap_or_else(|e| e.into_inner())
            .extend_from_slice(staged.new_ovf);
    }
    if !staged.off_bytes.is_empty() {
        let mut cps = parts.checkpoints.write().unwrap_or_else(|e| e.into_inner());
        for &(w, level) in staged.new_offs {
            debug_assert_eq!(w as usize, cps.len());
            cps.push(level);
        }
    }
    count.store(staged.base + staged.n, Ordering::Release);
    durable
}

/// Resolved `(input index, fk)` jobs for one range read, sorted by fk.
pub(crate) fn loc_jobs(fks: &[Fk], count: u64) -> Vec<(usize, u64)> {
    let mut jobs: Vec<(usize, u64)> = Vec::new();
    for (i, fk) in fks.iter().enumerate() {
        let Some(id) = fk.get() else { continue };
        if id == 0 || id > count {
            continue;
        }
        jobs.push((i, id));
    }
    jobs.sort_unstable_by_key(|(_, id)| *id);
    jobs
}

/// `(window, job_lo, job_hi)` runs over sorted `jobs`, one per loc window.
pub(crate) fn loc_window_groups(
    jobs: &[(usize, u64)],
) -> impl Iterator<Item = (u64, usize, usize)> + '_ {
    let mut lo = 0usize;
    std::iter::from_fn(move || {
        if lo >= jobs.len() {
            return None;
        }
        let w = loc_window(jobs[lo].1);
        let mut hi = lo + 1;
        while hi < jobs.len() && loc_window(jobs[hi].1) == w {
            hi += 1;
        }
        let out = (w, lo, hi);
        lo = hi;
        Some(out)
    })
}
