//! Spender-meta annotation for Class A.
//!
//! Confirm decides sole, multi or promote from the slot it already knows and
//! notes the result in the spend delta. Nothing here reads or writes
//! `spent.body`: a fold is its only writer.

use crate::compact::output_flags;
use crate::error::StoreError;
use crate::spender_table::SpenderTable;
use crate::tx_table::{encode_spent_slot, OutputRecord, TxTable};
use rbitcoin_primitives::Fk;

#[doc(hidden)]
pub const META_LEN: usize = OutputRecord::SPENT_SLOT_LEN;

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

/// Note each spend at its absolute `spent.body` offset, from the slot
/// `known[i]` that confirm already holds for `abs_edges[i]`.
///
/// A slot that already names this spender is noted again: a freeze after a
/// disconnect may have zeroed that note, and the delta must hold the decision
/// confirm acts on. A slot past the published body is returned as cold.
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
    let mut cold = Vec::new();
    let mut notes: Vec<(u64, [u8; META_LEN])> = Vec::with_capacity(order.len());
    for &i in &order {
        let (abs, cfk, vout, sfk, vin) = abs_edges[i];
        if !crate::spend_delta::is_historical(body_pub, abs) {
            cold.push((cfk, vout, sfk, vin));
            continue;
        }
        let (field, flags, field_vin) = known[i];
        let meta = match decide_annotate(field, flags, field_vin, sfk, vin, spenders)? {
            AnnotateOp::Skip => encode_spent_slot(flags, field, field_vin)?,
            AnnotateOp::Write(meta) => meta,
        };
        notes.push((abs, meta));
    }
    txs.note_spend_slots(&notes)?;
    Ok(cold)
}
