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
