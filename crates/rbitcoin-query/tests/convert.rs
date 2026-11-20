use bitcoin::absolute::LockTime;
use bitcoin::block::Header;
use bitcoin::block::Version;
use bitcoin::hashes::Hash;
use bitcoin::transaction::Version as TxVersion;
use bitcoin::Transaction;
use bitcoin::{
    Amount, BlockHash, CompactTarget, OutPoint, ScriptBuf, Sequence, TxIn, TxMerkleNode, TxOut,
    Witness,
};
use rbitcoin_primitives::Fk;
use rbitcoin_query::convert::*;

#[test]
fn header_to_record_stores_caller_hash() {
    let header = Header {
        version: Version::ONE,
        prev_blockhash: BlockHash::from_byte_array([0; 32]),
        merkle_root: TxMerkleNode::from_byte_array([0; 32]),
        time: 1,
        bits: CompactTarget::from_consensus(0x207f_ffff),
        nonce: 0,
    };
    let computed = header.block_hash().to_byte_array();
    let rec = header_to_record(Fk(3), &header, computed);
    assert_eq!(rec.hash, computed);
    assert_eq!(rec.prev_fk, Fk(3));
    assert_eq!(rec.timestamp, 1);
    let distinct = [0xab; 32];
    assert_ne!(distinct, computed);
    let rec2 = header_to_record(Fk::NULL, &header, distinct);
    assert_eq!(rec2.hash, distinct);
}

/// Write encode must bind each stamped spend edge to the wire prevout it
/// claims to spend — a stale/mismatched edge is Corrupt, not encoded.
#[test]
fn input_encode_rejects_edge_wire_prevout_mismatch() {
    let tx = Transaction {
        version: TxVersion::TWO,
        lock_time: LockTime::ZERO,
        input: vec![TxIn {
            previous_output: OutPoint {
                txid: bitcoin::Txid::from_byte_array([1; 32]),
                vout: 3,
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
    let edge = |prev_txid: [u8; 32], vout: u32| rbitcoin_query::SpendEdge {
        prev_txid,
        vout,
        spend_fk: Fk(9),
        create_fk: Fk(5),
        vin: 0,
    };
    let ok = rbitcoin_query::input_records_from_wire(&tx, Fk(9), &[edge([1u8; 32], 3)]).unwrap();
    assert_eq!(ok[0].prev_txid, [1u8; 32]);
    assert_eq!(ok[0].prev_index(), 3);
    for bad in [edge([2u8; 32], 3), edge([1u8; 32], 4)] {
        let err = rbitcoin_query::input_records_from_wire(&tx, Fk(9), &[bad])
            .expect_err("mismatched edge must not encode");
        assert!(
            format!("{err}").contains("edge/wire prevout"),
            "unexpected error: {err}"
        );
    }
}
