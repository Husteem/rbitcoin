//! Core's max-fee look for a client broadcast: resolve a tx's prevouts from
//! the package, the mempool, and the confirmed UTXO set, then compare its fee
//! rate with `maxfeerate`. RPC `sendrawtransaction`/`submitpackage`, Electrum
//! and Esplora all need the same answer.

use bitcoin::{OutPoint, Transaction, TxOut};
use rbitcoin_consensus::policy::{fee_exceeds_max, feerate_exceeds_max, is_unspendable};
use rbitcoin_mempool::UtxoProvider;

use crate::MempoolHub;

#[derive(Debug)]
pub enum TxFeeLook {
    Fee(u64),
    MissingPrevout,
    Overflow,
}

#[doc(hidden)]
pub fn fold_tx_fee_sat(
    in_vals: impl IntoIterator<Item = Option<u64>>,
    out_sum: Option<u64>,
) -> TxFeeLook {
    let mut in_sum = 0u64;
    for v in in_vals {
        let Some(v) = v else {
            return TxFeeLook::MissingPrevout;
        };
        in_sum = match in_sum.checked_add(v) {
            Some(s) => s,
            None => return TxFeeLook::Overflow,
        };
    }
    let Some(out_sum) = out_sum else {
        return TxFeeLook::Overflow;
    };
    match in_sum.checked_sub(out_sum) {
        Some(fee) => TxFeeLook::Fee(fee),
        None => TxFeeLook::Overflow,
    }
}

#[doc(hidden)]
pub fn tx_output_sum_sat(tx: &Transaction) -> Option<u64> {
    tx.output
        .iter()
        .try_fold(0u64, |a, o| a.checked_add(o.value.to_sat()))
}

fn prevout_txout(hub: &MempoolHub, op: &OutPoint) -> Option<TxOut> {
    if let Some(out) = hub.live_output(op) {
        // Admission still loads this output. Pricing it turns a 1 sat spend
        // into the fee cap and hides the script reject.
        if is_unspendable(out.script_pubkey.as_bytes()) {
            return None;
        }
        return Some(out);
    }
    hub.utxo_provider().get_txout(op)
}

fn tx_fee_sat_from_prevouts(hub: &MempoolHub, tx: &Transaction) -> TxFeeLook {
    fold_tx_fee_sat(
        tx.input
            .iter()
            .map(|inp| prevout_txout(hub, &inp.previous_output).map(|o| o.value.to_sat())),
        tx_output_sum_sat(tx),
    )
}

#[doc(hidden)]
pub fn tx_fee_exceeds_max(hub: &MempoolHub, tx: &Transaction, max_sat_kvb: u64) -> bool {
    match tx_fee_sat_from_prevouts(hub, tx) {
        TxFeeLook::Fee(fee) => fee_exceeds_max(fee, tx.weight().to_wu(), max_sat_kvb),
        TxFeeLook::Overflow => max_sat_kvb != 0,
        TxFeeLook::MissingPrevout => false,
    }
}

fn prevout_txout_in_package(
    hub: &MempoolHub,
    op: &OutPoint,
    package: &[Transaction],
) -> Option<TxOut> {
    for tx in package {
        if tx.compute_txid() == op.txid {
            return tx.output.get(op.vout as usize).cloned();
        }
    }
    prevout_txout(hub, op)
}

/// Core `submitpackage`: the member's modified fee over its sigop-adjusted
/// vsize is above `max_sat_kvb`. `None` when an input resolves in neither
/// the package, the mempool nor the chain: the member fails alone.
#[doc(hidden)]
pub fn package_tx_fee_exceeds_max(
    hub: &MempoolHub,
    tx: &Transaction,
    package: &[Transaction],
    max_sat_kvb: u64,
) -> Option<bool> {
    let prevouts = tx
        .input
        .iter()
        .map(|inp| prevout_txout_in_package(hub, &inp.previous_output, package))
        .collect::<Option<Vec<_>>>()?;
    let fee = prevouts
        .iter()
        .try_fold(0u64, |a, o| a.checked_add(o.value.to_sat()))
        .zip(tx_output_sum_sat(tx))
        .and_then(|(in_sum, out_sum)| in_sum.checked_sub(out_sum));
    // An overflowing or negative fee is over any cap but zero.
    Some(match fee {
        Some(fee) => feerate_exceeds_max(
            fee.saturating_add_signed(hub.fee_delta(&tx.compute_txid())),
            hub.sigops_adjusted_weight(tx, &prevouts),
            max_sat_kvb,
        ),
        None => max_sat_kvb != 0,
    })
}
