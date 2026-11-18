//! Spender-meta annotation for Class A.
//!
//! Confirm decides sole, multi or promote from the slot it already knows and
//! logs every published slot, including the short tail. Nothing here writes
//! `spent.body`.

use crate::compact::output_flags;
use crate::error::StoreError;
use crate::spender_table::SpenderTable;
use crate::tx_table::{encode_spent_slot, OutputRecord, TxTable};
use rbitcoin_primitives::Fk;

#[doc(hidden)]
pub const META_LEN: usize = OutputRecord::SPENT_SLOT_LEN;

#[doc(hidden)]
pub type SpentAbsWrite = (u64, Fk, u32, Fk, u32, [u8; META_LEN]);

/// Decision from structural snapshot + spend_fk (no body read).
#[doc(hidden)]
pub enum AnnotateOp {
    Skip,
    Write([u8; META_LEN]),
}

#[doc(hidden)]
pub fn decide_annotate(
    field: Fk,
    flags: u8,
    field_vin: u32,
    spend_fk: Fk,
    spend_vin: u32,
    spenders: &SpenderTable,
) -> Result<AnnotateOp, StoreError> {
    let multi = flags & output_flags::MULTI_SPENDER != 0;
    if !multi && field.is_null() {
        let meta = encode_spent_slot(flags & !output_flags::MULTI_SPENDER, spend_fk, spend_vin)?;
        return Ok(AnnotateOp::Write(meta));
    }
    if !multi && field == spend_fk && field_vin == spend_vin {
        return Ok(AnnotateOp::Skip);
    }
    if !multi {
        let e1 = spenders.append(field, field_vin, Fk::NULL)?;
        let e2 = spenders.append(spend_fk, spend_vin, e1)?;
        let meta = encode_spent_slot(flags | output_flags::MULTI_SPENDER, e2, 0)?;
        return Ok(AnnotateOp::Write(meta));
    }
    if multi_list_contains(spenders, field, spend_fk, spend_vin)? {
        return Ok(AnnotateOp::Skip);
    }
    let e = spenders.append(spend_fk, spend_vin, field)?;
    let meta = encode_spent_slot(flags | output_flags::MULTI_SPENDER, e, 0)?;
    Ok(AnnotateOp::Write(meta))
}

#[doc(hidden)]
pub fn multi_list_contains(
    spenders: &SpenderTable,
    head: Fk,
    spend_fk: Fk,
    spend_vin: u32,
) -> Result<bool, StoreError> {
    let mut cur = head;
    let mut n = 0u32;
    while let Some(id) = cur.get() {
        n = n.saturating_add(1);
        if n > 1_000_000 {
            return Err(StoreError::Corrupt("invariant: spender multi-list cycle"));
        }
        let (sfk, vin, next) = spenders.get(Fk(id))?;
        if sfk == spend_fk && vin == spend_vin {
            return Ok(true);
        }
        cur = next;
    }
    Ok(false)
}

/// Pure-write annotate: `known[i]` is structural `(field, flags)` for `abs_edges[i]`.
///
/// A published slot, including the short tail, is a directory insert.
/// Returns edges past the published body as cold.
pub fn put_spend_batch_by_abs_meta_known(
    txs: &TxTable,
    spenders: &SpenderTable,
    abs_edges: &[(u64, Fk, u32, Fk, u32)],
    known: &[(Fk, u8, u32)],
) -> Result<Vec<(Fk, u32, Fk, u32)>, StoreError> {
    if abs_edges.is_empty() {
        return Ok(Vec::new());
    }
    if abs_edges.len() != known.len() {
        return Err(StoreError::Corrupt("spend annotate known length mismatch"));
    }
    for &(_, _, _, sfk, _) in abs_edges {
        if sfk.is_null() {
            return Err(StoreError::InvalidFk);
        }
    }

    let mut order: Vec<usize> = (0..abs_edges.len()).collect();
    order.sort_unstable_by_key(|&i| abs_edges[i].0);

    let body_pub = txs.spent.body_published_len();
    let mut cold: Vec<(Fk, u32, Fk, u32)> = Vec::new();
    let mut writes: Vec<SpentAbsWrite> = Vec::with_capacity(order.len());

    for &i in &order {
        let (abs, cfk, vout, sfk, vin) = abs_edges[i];
        if abs.saturating_add(META_LEN as u64) > body_pub {
            cold.push((cfk, vout, sfk, vin));
            continue;
        }
        let (field, flags, field_vin) = known[i];
        match decide_annotate(field, flags, field_vin, sfk, vin, spenders)? {
            AnnotateOp::Skip => {
                // `keep` can drop the image after this batch decided to skip.
                // A slot the log still serves is written again. The body is
                // read only on a miss, so a torn page does not fail that rewrite.
                let rewrite = txs.spend_log_serves(abs)
                    || !txs.body_slot_matches(abs, field, flags, field_vin)?;
                if rewrite {
                    let meta = encode_spent_slot(flags, field, field_vin)?;
                    writes.push((abs, cfk, vout, sfk, vin, meta));
                }
            }
            AnnotateOp::Write(meta) => writes.push((abs, cfk, vout, sfk, vin, meta)),
        }
    }

    if writes.is_empty() {
        return Ok(cold);
    }
    let more = apply_prepared_spend_writes(txs, &writes)?;
    cold.extend(more);
    Ok(cold)
}

/// Every slot inside the published body lands in the page directory.
///
/// A slot past that length is cold. This path does not write `spent.body`.
fn apply_prepared_spend_writes(
    txs: &TxTable,
    writes: &[SpentAbsWrite],
) -> Result<Vec<(Fk, u32, Fk, u32)>, StoreError> {
    let published = txs.spent.body_published_len();
    let mut cold = Vec::new();
    let mut hist: Vec<(u64, [u8; META_LEN])> = Vec::new();
    for w in writes {
        if crate::spend_page_log::is_historical(published, w.0) {
            hist.push((w.0, w.5));
        } else {
            cold.push((w.1, w.2, w.3, w.4));
        }
    }
    txs.note_historical_spend_slots(&hist)?;
    Ok(cold)
}
