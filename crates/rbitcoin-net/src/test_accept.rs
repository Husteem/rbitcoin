//! `testmempoolaccept`: one row per submitted transaction, in submission
//! order. JSON-RPC and Esplora render the same rows.

use std::collections::HashSet;
use std::sync::Arc;

use bitcoin::{Transaction, Txid, Wtxid};
use rbitcoin_consensus::policy::fee_exceeds_max;
use rbitcoin_mempool::{ActiveMempool, TestedMembers, MAX_PACKAGE_WEIGHT};

use crate::tx_relay::failed_in_scripts;
use crate::{AcceptError, AcceptResult, MempoolHub};

pub struct TestAcceptRow {
    pub txid: Txid,
    pub wtxid: Wtxid,
    pub verdict: TestAcceptVerdict,
}

pub enum TestAcceptVerdict {
    Allowed {
        fee_sat: u64,
        weight: u64,
        vsize_bip141: u64,
    },
    /// Core `reject-reason` and `reject-details`.
    Rejected {
        reason: String,
        details: Option<String>,
    },
    /// The whole package failed before any member ran. Core omits `allowed`.
    PackageError(&'static str),
}

impl TestAcceptRow {
    fn of(tx: &Transaction, verdict: TestAcceptVerdict) -> Self {
        Self {
            txid: tx.compute_txid(),
            wtxid: tx.compute_wtxid(),
            verdict,
        }
    }

    fn rejected(tx: &Transaction, reason: &str) -> Self {
        Self::of(
            tx,
            TestAcceptVerdict::Rejected {
                reason: reason.to_string(),
                details: None,
            },
        )
    }

    /// Whether the row says `allowed: true`.
    fn is_allowed(&self) -> bool {
        matches!(self.verdict, TestAcceptVerdict::Allowed { .. })
    }

    fn admitted(tx: &Transaction, r: &AcceptResult, max_feerate_sat_kvb: u64) -> Self {
        if fee_exceeds_max(r.fee_sat, r.weight, max_feerate_sat_kvb) {
            return Self::rejected(tx, "max-fee-exceeded");
        }
        Self::of(
            tx,
            TestAcceptVerdict::Allowed {
                fee_sat: r.fee_sat,
                weight: r.weight,
                vsize_bip141: tx.vsize() as u64,
            },
        )
    }

    fn refused(tx: &Transaction, e: &AcceptError) -> Self {
        Self::of(
            tx,
            TestAcceptVerdict::Rejected {
                reason: accept_reject_reason(e),
                details: accept_reject_details(e, tx),
            },
        )
    }
}

/// API submit admits versions 1, 2, and 3 (BIP 431 TRUC). P2P admits any
/// `nVersion`.
pub fn submit_tx_version_nonstandard(tx: &Transaction) -> bool {
    !(1..=3).contains(&tx.version.0)
}

/// Two members spend the same outpoint, or a member has no inputs. Core
/// `IsConsistentPackage` cannot check a member without inputs, and it adds a
/// member's inputs only after checking all of them, so an input repeated
/// inside one tx is left to that tx's `bad-txns-inputs-duplicate`.
pub fn package_has_conflicts(txs: &[Transaction]) -> bool {
    let mut spent = HashSet::new();
    for tx in txs {
        if tx.input.is_empty() || tx.input.iter().any(|i| spent.contains(&i.previous_output)) {
            return true;
        }
        spent.extend(tx.input.iter().map(|i| i.previous_output));
    }
    false
}

/// Core `reject-reason` text for a mempool reject.
pub fn accept_reject_reason(e: &impl std::fmt::Display) -> String {
    let owned = e.to_string();
    let s = owned.strip_prefix("policy: ").unwrap_or(owned.as_str());
    if s == "coinbase immature" {
        return "bad-txns-premature-spend-of-coinbase".into();
    }
    if s.starts_with("missing prevout") || s.starts_with("orphaned ") {
        return "missing-inputs".into();
    }
    if s.starts_with("duplicate ") {
        return "txn-already-in-mempool".into();
    }
    if s == "inputs-duplicate" {
        return "bad-txns-inputs-duplicate".into();
    }
    if s == "not final" {
        return "non-final".into();
    }
    if s == "negative fee" {
        return "bad-txns-in-belowout".into();
    }
    if s == "non-BIP68-final" {
        return "non-BIP68-final".into();
    }
    if s == "rbf insufficient fee" {
        return "insufficient fee".into();
    }
    if s == "rbf insufficient fee (including sibling eviction)" {
        return "insufficient fee (including sibling eviction)".into();
    }
    if s == "min relay fee" {
        return "min relay fee not met".into();
    }
    if s == "mempool min fee" {
        return "mempool min fee not met".into();
    }
    if let Some(rest) = s.strip_prefix("script: ") {
        let rest = rest
            .strip_prefix("script verification failed: ")
            .unwrap_or(rest);
        let paren = rbitcoin_consensus::script_flag_paren(rest);
        return format!("mempool-script-verify-flag-failed ({paren})");
    }
    s.to_string()
}

/// Core `reject-details` for mempool rejects.
fn accept_reject_details(e: &impl std::fmt::Display, tx: &Transaction) -> Option<String> {
    let reason = accept_reject_reason(e);
    if matches!(
        reason.as_str(),
        "txn-already-in-mempool"
            | "txn-same-nonwitness-data-in-mempool"
            | "bip125-replacement-disallowed"
    ) {
        return Some(reason);
    }
    if !reason.starts_with("mempool-script-verify-flag-failed") {
        return None;
    }
    let vin = 0usize;
    let inp = tx.input.get(vin)?;
    let prev = inp.previous_output;
    Some(format!(
        "{reason}, input {vin} of {} (wtxid {}), spending {}:{}",
        tx.compute_txid(),
        tx.compute_wtxid(),
        prev.txid,
        prev.vout
    ))
}

fn package_error(txs: &[Transaction], msg: &'static str) -> Vec<TestAcceptRow> {
    txs.iter()
        .map(|tx| TestAcceptRow::of(tx, TestAcceptVerdict::PackageError(msg)))
        .collect()
}

/// Core `IsWellFormedPackage`, in Core's order. A failure there is the
/// whole package's. Every surface caps the count at 25 before the walk.
fn package_precheck(txs: &[Transaction]) -> Option<&'static str> {
    let weight: u64 = txs.iter().map(|tx| tx.weight().to_wu()).sum();
    let mut ids = HashSet::new();
    if weight > MAX_PACKAGE_WEIGHT {
        Some("package-too-large")
    } else if txs.iter().any(|tx| !ids.insert(tx.compute_txid())) {
        Some("package-contains-duplicates")
    } else if !ActiveMempool::package_is_sorted(txs) {
        Some("package-not-sorted")
    } else if package_has_conflicts(txs) {
        Some("conflict-in-package")
    } else {
        None
    }
}

impl MempoolHub {
    /// Test-accept `txs` without touching the pool, with script verify
    /// awaited on the caller. Two or more are a package: each member that
    /// passes joins [`TestedMembers`], so a later child sees its parent
    /// without the parent entering the pool.
    pub async fn test_accept_rows_async(
        self: &Arc<Self>,
        txs: Vec<Transaction>,
        max_feerate_sat_kvb: u64,
    ) -> Vec<TestAcceptRow> {
        if txs.len() >= 2 {
            if let Some(error) = package_precheck(&txs) {
                return package_error(&txs, error);
            }
            return self
                .test_accept_package_rows_async(txs, max_feerate_sat_kvb)
                .await;
        }
        let Some(tx) = txs.into_iter().next() else {
            return Vec::new();
        };
        let row = if submit_tx_version_nonstandard(&tx) {
            TestAcceptRow::rejected(&tx, "version")
        } else {
            match self.test_accept_async(tx.clone(), None).await {
                Ok(r) => TestAcceptRow::admitted(&tx, &r, max_feerate_sat_kvb),
                Err(e) => TestAcceptRow::refused(&tx, &e),
            }
        };
        vec![row]
    }

    /// Core's package order, member by member. A member that passes every
    /// check before scripts joins [`TestedMembers`], so a later member sees
    /// it as a parent, and it counts toward its cluster even if its scripts
    /// or `maxfeerate` then fail. Once the walk is done, each cluster those
    /// members make with the pool is checked under one read lock, and every
    /// member of an over-cap cluster is `too-large-cluster`: Core asks the
    /// cluster before scripts, and `testmempoolaccept` asks `maxfeerate` only
    /// after validation. Outside such a cluster, a member that spends one
    /// whose row is not allowed is `missing-inputs`, as that parent would not
    /// enter. A member that fails before scripts, such as one already in the
    /// pool, keeps its reason and is not counted; a later member that spends
    /// a pooled one joins its pool cluster.
    async fn test_accept_package_rows_async(
        self: &Arc<Self>,
        txs: Vec<Transaction>,
        max_feerate_sat_kvb: u64,
    ) -> Vec<TestAcceptRow> {
        let mut tested = Arc::new(TestedMembers::default());
        let mut out = Vec::with_capacity(txs.len());
        let mut counted = Vec::new();
        let mut counted_txs = Vec::new();
        for (i, tx) in txs.into_iter().enumerate() {
            if submit_tx_version_nonstandard(&tx) {
                out.push(TestAcceptRow::rejected(&tx, "version"));
                continue;
            }
            let outcome = self
                .test_accept_async(tx.clone(), Some(Arc::clone(&tested)))
                .await;
            let passed_prechecks = match &outcome {
                Ok(_) => true,
                Err(e) => failed_in_scripts(e),
            };
            out.push(match outcome {
                Ok(r) => TestAcceptRow::admitted(&tx, &r, max_feerate_sat_kvb),
                Err(e) => TestAcceptRow::refused(&tx, &e),
            });
            if passed_prechecks {
                Arc::make_mut(&mut tested).push(Arc::new(tx.clone()));
                counted.push(i);
                counted_txs.push(tx);
            }
        }
        if counted_txs.is_empty() {
            return out;
        }
        let hub = Arc::clone(self);
        let (over_cap, counted_txs) = crate::reactor::off_pool(move || {
            (hub.package_clusters_over_cap(&counted_txs), counted_txs)
        })
        .await;
        let mut refused: HashSet<Txid> = HashSet::new();
        for (k, (&i, tx)) in counted.iter().zip(&counted_txs).enumerate() {
            if over_cap.contains(&k) {
                out[i] = TestAcceptRow::rejected(tx, "too-large-cluster");
                continue;
            }
            if tx
                .input
                .iter()
                .any(|inp| refused.contains(&inp.previous_output.txid))
            {
                out[i] = TestAcceptRow::rejected(tx, "missing-inputs");
            }
            if !out[i].is_allowed() {
                refused.insert(out[i].txid);
            }
        }
        out
    }
}
