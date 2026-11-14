use bitcoin::absolute::LockTime;
use bitcoin::hashes::Hash;
use bitcoin::transaction::Version;
use bitcoin::{Amount, OutPoint, ScriptBuf, Sequence, Transaction, TxIn, TxOut, Txid, Witness};
use rbitcoin_mempool::accept::MapUtxoProvider;
use rbitcoin_mempool::{
    ActiveMempool, ChainTipCtx, Coin, PoolSequence, Removal, RemovalReason, RemovedRow,
    REMOVED_ROWS,
};
use std::collections::{BTreeSet, HashMap};

const TIP_OK: ChainTipCtx = ChainTipCtx {
    height: 1_000_000,
    mtp: u32::MAX,
};

fn tmp_dir() -> rbitcoin_store::testutil::TempDir {
    rbitcoin_store::testutil::TempDir::labeled("mempool-removed").unwrap()
}

fn chain_coins(n: u8) -> (Vec<OutPoint>, MapUtxoProvider) {
    let mut map = HashMap::new();
    let mut ops = Vec::new();
    for i in 0..n {
        let op = OutPoint {
            txid: Txid::from_byte_array([i.wrapping_add(1); 32]),
            vout: 0,
        };
        let coin = Coin {
            txout: TxOut {
                value: Amount::from_sat(100_000),
                script_pubkey: ScriptBuf::from_bytes(vec![0x51]),
            },
            create_height: 1,
            create_mtp: 0,
            is_coinbase: false,
            create_fk: None,
        };
        map.insert(op, coin);
        ops.push(op);
    }
    (ops, MapUtxoProvider { map })
}

fn spend(op: OutPoint, out_value: u64) -> Transaction {
    Transaction {
        version: Version::TWO,
        lock_time: LockTime::ZERO,
        input: vec![TxIn {
            previous_output: op,
            script_sig: ScriptBuf::new(),
            sequence: Sequence::ENABLE_RBF_NO_LOCKTIME,
            witness: Witness::new(),
        }],
        output: vec![TxOut {
            value: Amount::from_sat(out_value),
            script_pubkey: ScriptBuf::from_bytes(vec![0x51]),
        }],
    }
}

/// Core's `mempool_sequence`: the value the next change takes.
#[test]
fn the_sequence_starts_at_one_and_moves_once_per_add_and_removal() {
    let dir = tmp_dir();
    let (ops, utxos) = chain_coins(2);
    let mut mp = ActiveMempool::open_or_create(&dir).unwrap();
    assert_eq!(mp.mempool_sequence(), 1);
    let a = spend(ops[0], 99_000);
    let b = spend(ops[1], 99_000);
    mp.accept_tx(&a, &utxos, TIP_OK).expect("a");
    mp.accept_tx(&b, &utxos, TIP_OK).expect("b");
    assert_eq!(mp.mempool_sequence(), 3);
    mp.accept_tx(&spend(ops[0], 90_000), &utxos, TIP_OK)
        .expect("replacement");
    assert_eq!(mp.mempool_sequence(), 5, "one removal, one add");
    mp.remove_live_txids(&[b.compute_txid()], BLOCK).unwrap();
    assert_eq!(mp.mempool_sequence(), 6);
    let rows = mp.removed_rows();
    mp.compact().unwrap();
    assert_eq!(mp.mempool_sequence(), 6, "a rebuild is not a change");
    assert_eq!(mp.removed_rows(), rows, "a rebuild writes no row");
    mp.flush().unwrap();
    drop(mp);

    let mp = ActiveMempool::open_or_create(&dir).unwrap();
    assert_eq!(mp.live_count(), 1);
    assert_eq!(mp.mempool_sequence(), 1, "loaded records are not changes");
    assert_eq!(mp.removed_rows().oldest, 1);
    assert!(mp.removed_rows().rows.is_empty());
}

const BLOCK: Removal = Removal::Committed(RemovalReason::Block);

fn out0(tx: &Transaction) -> OutPoint {
    OutPoint {
        txid: tx.compute_txid(),
        vout: 0,
    }
}

fn row(seq: u64, tx: &Transaction, reason: RemovalReason) -> RemovedRow {
    RemovedRow {
        seq,
        txid: tx.compute_txid(),
        reason,
    }
}

/// Rows written since `from`, as a set: one call may unlink in any order.
fn rows_from(mp: &ActiveMempool, from: u64) -> BTreeSet<(u64, Txid, RemovalReason)> {
    mp.removed_rows()
        .rows
        .into_iter()
        .filter(|r| r.seq >= from)
        .map(|r| (r.seq, r.txid, r.reason))
        .collect()
}

fn two_rows(
    from: u64,
    a: &Transaction,
    b: &Transaction,
    reason: RemovalReason,
) -> [BTreeSet<(u64, Txid, RemovalReason)>; 2] {
    let (a, b) = (a.compute_txid(), b.compute_txid());
    [
        BTreeSet::from([(from, a, reason), (from + 1, b, reason)]),
        BTreeSet::from([(from, b, reason), (from + 1, a, reason)]),
    ]
}

#[test]
fn each_pool_removal_keeps_its_reason_and_sequence() {
    let dir = tmp_dir();
    let (ops, utxos) = chain_coins(6);
    let mut mp = ActiveMempool::open_or_create(&dir).unwrap();

    let parent = spend(ops[0], 99_000);
    let child = spend(out0(&parent), 98_000);
    mp.accept_tx(&parent, &utxos, TIP_OK).expect("parent");
    mp.accept_tx(&child, &utxos, TIP_OK).expect("child");
    let at = mp.mempool_sequence();
    let bump = spend(ops[0], 90_000);
    mp.accept_tx(&bump, &utxos, TIP_OK).expect("replacement");
    let by = RemovalReason::Replaced {
        by: bump.compute_txid(),
    };
    assert!(
        two_rows(at, &parent, &child, by).contains(&rows_from(&mp, at)),
        "the conflict and its descendant, replaced by the admit: {:?}",
        mp.removed_rows()
    );

    let mined = spend(ops[1], 99_000);
    let loser = spend(ops[2], 99_000);
    let loser_child = spend(out0(&loser), 98_000);
    for tx in [&mined, &loser, &loser_child] {
        mp.accept_tx(tx, &utxos, TIP_OK).expect("admit");
    }
    let at = mp.mempool_sequence();
    mp.remove_block_txids(&[mined.compute_txid()], &[ops[2]])
        .unwrap();
    assert_eq!(
        mp.removed_rows().rows.iter().rev().nth(2),
        Some(&row(at, &mined, RemovalReason::Block)),
        "the listed tx first"
    );
    assert!(
        two_rows(at + 1, &loser, &loser_child, RemovalReason::Conflict)
            .contains(&rows_from(&mp, at + 1))
    );

    let stale = spend(ops[3], 99_000);
    mp.accept_tx(&stale, &utxos, TIP_OK).expect("stale");
    let at = mp.mempool_sequence();
    mp.remove_live_txids(
        &[stale.compute_txid()],
        Removal::Committed(RemovalReason::Expiry),
    )
    .unwrap();
    assert_eq!(
        mp.removed_rows().rows.last(),
        Some(&row(at, &stale, RemovalReason::Expiry))
    );

    let orphaned = spend(ops[4], 99_000);
    mp.accept_tx(&orphaned, &utxos, TIP_OK).expect("orphaned");
    let mut reorged = MapUtxoProvider {
        map: utxos.map.clone(),
    };
    reorged.map.remove(&ops[4]);
    let at = mp.mempool_sequence();
    mp.evict_nonfinal(&reorged, TIP_OK);
    assert_eq!(
        mp.removed_rows().rows.last(),
        Some(&row(at, &orphaned, RemovalReason::Reorg)),
        "an input the reorg took away"
    );

    let cheap = spend(ops[5], 99_900);
    mp.accept_tx(&cheap, &utxos, TIP_OK).expect("cheap");
    mp.max_weight = mp.graph.total_weight() - 1;
    let at = mp.mempool_sequence();
    mp.evict_to_budget(None).unwrap();
    assert_eq!(
        mp.removed_rows().rows.last(),
        Some(&row(at, &cheap, RemovalReason::SizeLimit)),
        "the worst chunk leaves for the budget"
    );
    assert_eq!(mp.removed_rows().oldest, 1);
}

#[test]
fn a_replacement_that_does_not_commit_leaves_no_replaced_row() {
    let dir = tmp_dir();
    let (ops, utxos) = chain_coins(1);
    let mut mp = ActiveMempool::open_or_create(&dir).unwrap();
    let victim = spend(ops[0], 99_000);
    let id = mp.accept_tx(&victim, &utxos, TIP_OK).expect("victim").txid;
    let at = mp.mempool_sequence();
    mp.store.fail_next_append = true;
    assert!(mp
        .accept_tx(&spend(ops[0], 90_000), &utxos, TIP_OK)
        .is_err());
    assert!(mp.graph.contains(&id), "the victim is restored");
    assert!(mp.removed_rows().rows.is_empty(), "{:?}", mp.removed_rows());
    assert_eq!(mp.mempool_sequence(), at + 2, "out and back in");
}

#[test]
fn an_undo_writes_no_row_but_moves_the_sequence() {
    let dir = tmp_dir();
    let (ops, utxos) = chain_coins(2);
    let mut mp = ActiveMempool::open_or_create(&dir).unwrap();
    let victim = spend(ops[0], 99_000);
    mp.accept_tx(&victim, &utxos, TIP_OK).expect("victim");
    let bump = mp
        .accept_tx(&spend(ops[0], 90_000), &utxos, TIP_OK)
        .expect("replacement");
    assert_eq!(mp.removed_rows().rows.len(), 1);
    let at = mp.mempool_sequence();
    mp.rollback_accepted_package(std::slice::from_ref(&bump), &utxos, TIP_OK);
    assert!(mp.graph.contains(&victim.compute_txid()));
    assert!(!mp.graph.contains(&bump.txid));
    assert!(
        mp.removed_rows().rows.is_empty(),
        "a rolled-back admit replaced nothing: {:?}",
        mp.removed_rows()
    );
    assert_eq!(mp.mempool_sequence(), at + 2, "out, then the victim back");

    let staged = spend(ops[1], 99_000);
    mp.accept_tx(&staged, &utxos, TIP_OK).expect("staged");
    let at = mp.mempool_sequence();
    mp.remove_live_txids(&[staged.compute_txid()], Removal::Undo)
        .unwrap();
    assert_eq!(mp.mempool_sequence(), at + 1);
    assert!(mp.removed_rows().rows.is_empty());

    // A listed transaction that left for good keeps that row when a later
    // staged copy of it is taken back. The listed one left for that reason.
    mp.accept_tx(&staged, &utxos, TIP_OK).expect("listed");
    mp.remove_live_txids(&[staged.compute_txid()], BLOCK)
        .unwrap();
    let kept = mp.removed_rows();
    assert_eq!(kept.rows.len(), 1);
    mp.accept_tx(&staged, &utxos, TIP_OK).expect("staged copy");
    mp.remove_live_txids(&[staged.compute_txid()], Removal::Undo)
        .unwrap();
    assert_eq!(mp.removed_rows(), kept);
}

#[test]
fn the_ring_drops_its_oldest_row_past_the_cap() {
    let mut seq = PoolSequence::default();
    let txid = |i: usize| Txid::from_byte_array([(i % 251) as u8; 32]);
    for i in 0..REMOVED_ROWS {
        seq.note_removed(txid(i), BLOCK);
    }
    let full = seq.removed_rows();
    assert_eq!((full.oldest, full.rows.len()), (1, REMOVED_ROWS));
    seq.note_removed(txid(REMOVED_ROWS), BLOCK);
    seq.note_removed(txid(REMOVED_ROWS + 1), BLOCK);
    let past = seq.removed_rows();
    assert_eq!(past.rows.len(), REMOVED_ROWS);
    assert_eq!(past.rows[0].seq, 3, "rows 1 and 2 dropped");
    assert_eq!(past.oldest, 3);
    assert_eq!(seq.next(), REMOVED_ROWS as u64 + 3);
}
