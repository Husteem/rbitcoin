use bitcoin::absolute::LockTime;
use bitcoin::block::{Header, Version as BlockVersion};
use bitcoin::hashes::Hash;
use bitcoin::{Block, OutPoint};

use bitcoin::transaction::Version;
use bitcoin::{Amount, ScriptBuf, Sequence, Transaction, TxIn, TxOut, Txid, Witness};

#[test]
fn wire_filter_missing_prevout_is_corrupt() {
    let (_dir, q) = rbitcoin_query::testutil::tiny_query_labeled("filter-missing-prevout");
    let spend = Transaction {
        version: Version::ONE,
        lock_time: LockTime::ZERO,
        input: vec![TxIn {
            previous_output: OutPoint {
                txid: Txid::from_byte_array([0x7a; 32]),
                vout: 1,
            },
            script_sig: ScriptBuf::new(),
            sequence: Sequence::MAX,
            witness: Witness::new(),
        }],
        output: vec![TxOut {
            value: Amount::from_sat(1),
            script_pubkey: ScriptBuf::from_bytes(vec![0x51]),
        }],
    };
    let block = Block {
        header: Header {
            version: BlockVersion::ONE,
            prev_blockhash: bitcoin::BlockHash::from_byte_array([0; 32]),
            merkle_root: bitcoin::TxMerkleNode::from_byte_array([0; 32]),
            time: 2,
            bits: bitcoin::CompactTarget::from_consensus(0x207fffff),
            nonce: 0,
        },
        txdata: vec![spend],
    };
    let err = q.basic_filter_from_wire_block(&block).unwrap_err();
    assert!(
        err.to_string().contains("blockfilter prevout missing"),
        "{err}"
    );
}

/// GCS keys are the block hash. Membership uses rust-bitcoin's own key schedule.
#[test]
fn basic_filter_keys_match_the_block_hash() {
    let hash: [u8; 32] = std::array::from_fn(|i| (i as u8).wrapping_add(1));
    let a: &[u8] = &[0x51];
    let b: &[u8] = &[0x52, 0x21];
    let filter = rbitcoin_query::basic_filter_from_scripts(&hash, [a, b], std::iter::empty())
        .expect("encode");
    let block_hash = bitcoin::BlockHash::from_byte_array(hash);
    assert!(filter
        .match_any(&block_hash, [a, b].into_iter())
        .expect("match"));
    let miss: &[u8] = &[0xff, 0x00];
    assert!(!filter
        .match_any(&block_hash, [miss].into_iter())
        .expect("miss"));
    let again = rbitcoin_query::basic_filter_from_scripts(&hash, [a, b], std::iter::empty())
        .expect("encode again");
    assert_eq!(filter.content, again.content);
}

/// Elements are encoded borrowed: rust-bitcoin's writer copies each one into
/// an ordered set. The bytes still match that writer, duplicates and an empty
/// element included.
#[test]
fn basic_filter_matches_rust_bitcoin_without_a_copy_per_element() {
    use bitcoin::bip158::GcsFilterWriter;
    use rbitcoin_query::block_filter::{encode_basic_filter, BASIC_FILTER_M, BASIC_FILTER_P};

    let hash: [u8; 32] = std::array::from_fn(|i| (i as u8) ^ 0x5a);
    let mut scripts: Vec<Vec<u8>> = (0..2000u32)
        .map(|i| {
            let mut s = vec![0x51u8; 22];
            s[..4].copy_from_slice(&i.to_le_bytes());
            s
        })
        .collect();
    let dups: Vec<Vec<u8>> = scripts[..50].to_vec();
    scripts.extend(dups);
    scripts.push(Vec::new());

    let k0 = u64::from_le_bytes(hash[0..8].try_into().unwrap());
    let k1 = u64::from_le_bytes(hash[8..16].try_into().unwrap());
    let mut want = Vec::new();
    {
        let mut w = GcsFilterWriter::new(&mut want, k0, k1, BASIC_FILTER_M, BASIC_FILTER_P);
        for s in &scripts {
            w.add_element(s);
        }
        w.finish().unwrap();
    }

    let (got, asked) = crate::alloc_count::asked_by(|| {
        encode_basic_filter(&hash, scripts.iter().map(|s| s.as_slice())).unwrap()
    });
    assert_eq!(got.content, want);
    assert!(
        asked.calls < 64,
        "{} allocator calls for {} elements",
        asked.calls,
        scripts.len()
    );
}

#[test]
fn torn_filter_body_is_written_back() {
    let _gate = rbitcoin_consensus::script_pool::steal_test_gate();
    let (_dir, q) = rbitcoin_query::testutil::tiny_query_labeled("filter-repair");
    q.set_block_filter_index(true).unwrap();
    rbitcoin_query::prepare_live_indexes(&q).unwrap();
    let params = rbitcoin_consensus::ChainParams::regtest();
    let ms = rbitcoin_consensus::Milestone::height(u32::MAX);
    let genesis = rbitcoin_consensus::genesis_block(&params);
    rbitcoin_query::accept_and_connect_block(
        &q,
        &params,
        rbitcoin_primitives::Height(0),
        &genesis,
        ms,
    )
    .unwrap();
    let b1 =
        rbitcoin_consensus::mine_empty_regtest(genesis.block_hash(), genesis.header.time + 600, 1);
    rbitcoin_query::accept_and_connect_block(&q, &params, rbitcoin_primitives::Height(1), &b1, ms)
        .unwrap();
    let good = q.basic_filter_at(1).unwrap().unwrap().0;
    assert!(!good.is_empty());

    let body = rbitcoin_store::BlockFilterTable::seg_dirs(q.store().cold_files_dir()).body_path(0);
    let mut raw = std::fs::read(&body).unwrap();
    for b in raw.iter_mut().skip(rbitcoin_store::file::FILE_HEADER_LEN) {
        *b ^= 0xff;
    }
    std::fs::write(&body, &raw).unwrap();
    rbitcoin_store::block_filter::fail_next_block_filter_repair(&body);

    let got = q
        .basic_filter_at(1)
        .expect("a torn body must be repaired")
        .expect("height 1 is sealed")
        .0;
    assert_eq!(got, good, "the repaired body must match the index hash");
    let stored = q
        .block_filter_table()
        .unwrap()
        .filter(rbitcoin_primitives::Height(1))
        .unwrap()
        .unwrap()
        .0;
    assert_eq!(
        stored, good,
        "repair_body must write the recomputed bytes back"
    );
    let again = q.basic_filter_at(1).unwrap().unwrap().0;
    assert_eq!(
        again, good,
        "a later by-height read must see the write-back"
    );

    std::fs::write(&body, &raw).unwrap();
    rbitcoin_store::block_filter::fail_next_block_filter_repair(&body);
    rbitcoin_store::block_filter::fail_next_block_filter_repair(&body);
    let err = q.basic_filter_at(1).unwrap_err();
    assert!(
        err.to_string().contains("blockfilter repair slot moved"),
        "{err}"
    );
    let still = q
        .block_filter_table()
        .unwrap()
        .filter(rbitcoin_primitives::Height(1))
        .unwrap()
        .unwrap()
        .0;
    assert_ne!(
        still, good,
        "a repair that keeps seeing a moved slot must not write"
    );
}

#[test]
fn by_hash_refuses_a_body_the_index_hash_rejected() {
    let _gate = rbitcoin_consensus::script_pool::steal_test_gate();
    let (_dir, q) = rbitcoin_query::testutil::tiny_query_labeled("filter-hash-reject");
    q.set_block_filter_index(true).unwrap();
    rbitcoin_query::prepare_live_indexes(&q).unwrap();
    let params = rbitcoin_consensus::ChainParams::regtest();
    let ms = rbitcoin_consensus::Milestone::height(u32::MAX);
    let genesis = rbitcoin_consensus::genesis_block(&params);
    rbitcoin_query::accept_and_connect_block(
        &q,
        &params,
        rbitcoin_primitives::Height(0),
        &genesis,
        ms,
    )
    .unwrap();
    let b1 =
        rbitcoin_consensus::mine_empty_regtest(genesis.block_hash(), genesis.header.time + 600, 1);
    rbitcoin_query::accept_and_connect_block(&q, &params, rbitcoin_primitives::Height(1), &b1, ms)
        .unwrap();
    let hash = b1.block_hash().to_byte_array();
    let idx = rbitcoin_store::BlockFilterTable::seg_dirs(q.store().cold_files_dir()).idx_path(0);
    let mut raw = std::fs::read(&idx).unwrap();
    let at = (rbitcoin_store::block_filter::HDR + rbitcoin_store::block_filter::SLOT + 16) as usize;
    for b in raw[at..at + 32].iter_mut() {
        *b ^= 0xff;
    }
    std::fs::write(&idx, &raw).unwrap();
    let err = q.basic_filter_for_hash(&hash).unwrap_err();
    assert!(
        err.to_string().contains("blockfilter body does not match"),
        "{err}"
    );
}

#[test]
fn by_hash_drops_a_row_when_the_height_moved() {
    let _gate = rbitcoin_consensus::script_pool::steal_test_gate();
    let (_dir, q) = rbitcoin_query::testutil::tiny_query_labeled("filter-height-moved");
    q.set_block_filter_index(true).unwrap();
    rbitcoin_query::prepare_live_indexes(&q).unwrap();
    let params = rbitcoin_consensus::ChainParams::regtest();
    let ms = rbitcoin_consensus::Milestone::height(u32::MAX);
    let genesis = rbitcoin_consensus::genesis_block(&params);
    rbitcoin_query::accept_and_connect_block(
        &q,
        &params,
        rbitcoin_primitives::Height(0),
        &genesis,
        ms,
    )
    .unwrap();
    let b1 =
        rbitcoin_consensus::mine_empty_regtest(genesis.block_hash(), genesis.header.time + 600, 1);
    rbitcoin_query::accept_and_connect_block(&q, &params, rbitcoin_primitives::Height(1), &b1, ms)
        .unwrap();
    let hash = b1.block_hash().to_byte_array();
    let good = q.basic_filter_for_hash(&hash).unwrap().unwrap().0;
    let sentinel = vec![0xab; 8];
    rbitcoin_query::block_filter::lie_next_basic_filter_body(sentinel.clone());
    rbitcoin_query::chain_view::forget_height_of_hash_on_call(2);
    let got = q.basic_filter_for_hash(&hash).unwrap().unwrap().0;
    assert_ne!(got, sentinel, "the moved height must not return that row");
    assert_eq!(got, good);
}

#[test]
fn by_hash_counts_a_block_that_left_the_chain_during_the_read() {
    let _gate = rbitcoin_consensus::script_pool::steal_test_gate();
    let (_dir, q) = rbitcoin_query::testutil::tiny_query_labeled("filter-left-chain");
    q.set_block_filter_index(true).unwrap();
    rbitcoin_query::prepare_live_indexes(&q).unwrap();
    let params = rbitcoin_consensus::ChainParams::regtest();
    let ms = rbitcoin_consensus::Milestone::height(u32::MAX);
    let genesis = rbitcoin_consensus::genesis_block(&params);
    rbitcoin_query::accept_and_connect_block(
        &q,
        &params,
        rbitcoin_primitives::Height(0),
        &genesis,
        ms,
    )
    .unwrap();
    let b1 =
        rbitcoin_consensus::mine_empty_regtest(genesis.block_hash(), genesis.header.time + 600, 1);
    rbitcoin_query::accept_and_connect_block(&q, &params, rbitcoin_primitives::Height(1), &b1, ms)
        .unwrap();
    let hash = b1.block_hash().to_byte_array();
    rbitcoin_query::chain_view::forget_height_of_hash_on_call(2);
    let err = q.basic_filter_for_hash_within(&hash, 0).unwrap_err();
    assert!(
        err.to_string()
            .contains(rbitcoin_query::block_filter::FILTER_STALE_TOO_DEEP),
        "{err}"
    );
}

#[test]
fn by_hash_drops_a_row_the_index_does_not_name() {
    let _gate = rbitcoin_consensus::script_pool::steal_test_gate();
    let (_dir, q) = rbitcoin_query::testutil::tiny_query_labeled("filter-row-bind");
    q.set_block_filter_index(true).unwrap();
    rbitcoin_query::prepare_live_indexes(&q).unwrap();
    let params = rbitcoin_consensus::ChainParams::regtest();
    let ms = rbitcoin_consensus::Milestone::height(u32::MAX);
    let genesis = rbitcoin_consensus::genesis_block(&params);
    rbitcoin_query::accept_and_connect_block(
        &q,
        &params,
        rbitcoin_primitives::Height(0),
        &genesis,
        ms,
    )
    .unwrap();
    let b1 =
        rbitcoin_consensus::mine_empty_regtest(genesis.block_hash(), genesis.header.time + 600, 1);
    rbitcoin_query::accept_and_connect_block(&q, &params, rbitcoin_primitives::Height(1), &b1, ms)
        .unwrap();
    let hash = b1.block_hash().to_byte_array();
    let good = q.basic_filter_for_hash(&hash).unwrap().unwrap().0;
    let sentinel = vec![0xcd; 8];
    rbitcoin_query::block_filter::lie_next_basic_filter_body(sentinel.clone());
    let got = q.basic_filter_for_hash(&hash).unwrap().unwrap().0;
    assert_ne!(
        got, sentinel,
        "a body the idx does not name must not be returned"
    );
    assert_eq!(got, good);
}
