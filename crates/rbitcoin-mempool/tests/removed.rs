use bitcoin::absolute::LockTime;
use bitcoin::hashes::Hash;
use bitcoin::transaction::Version;
use bitcoin::{Amount, OutPoint, ScriptBuf, Sequence, Transaction, TxIn, TxOut, Txid, Witness};
use rbitcoin_mempool::accept::MapUtxoProvider;
use rbitcoin_mempool::{ActiveMempool, ChainTipCtx, Coin};
use std::collections::HashMap;

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
    mp.remove_live_txids(&[b.compute_txid()]).unwrap();
    assert_eq!(mp.mempool_sequence(), 6);
    mp.flush().unwrap();
    drop(mp);

    let mut mp = ActiveMempool::open_or_create(&dir).unwrap();
    assert_eq!(mp.live_count(), 1);
    assert_eq!(mp.mempool_sequence(), 1, "loaded records are not changes");
    mp.compact().unwrap();
    assert_eq!(mp.mempool_sequence(), 1, "a rebuild is not a change");
}
