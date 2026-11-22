use bitcoin::script::Script;
use bitcoin::sighash::SighashCache;
use bitcoin::Transaction;
use rbitcoin_consensus::block::ScriptCheckJob;
use rbitcoin_consensus::script::p2pkh::*;

use bitcoin::absolute::LockTime;
use bitcoin::script::ScriptBuf;
use bitcoin::{Amount, OutPoint, Sequence, TxIn, TxOut, Witness};

#[test]
fn parse_two_pushes_errors() {
    assert!(parse_two_pushes(Script::from_bytes(&[0x51])).is_err()); // OP_1
    assert!(parse_two_pushes(Script::from_bytes(&[0x00])).is_err()); // OP_0 unexpected
    assert!(parse_two_pushes(Script::from_bytes(&[0x01, 0xaa])).is_err()); // len
    assert!(parse_two_pushes(Script::from_bytes(&[0x05, 0x01])).is_err()); // decode
    let mut big = vec![0x4d, 0x09, 0x02]; // PUSHDATA2 521
    big.extend([0u8; 521]);
    big.extend([0x01, 0xbb]);
    assert!(parse_two_pushes(Script::from_bytes(&big)).is_err()); // push size
    let ok = parse_two_pushes(Script::from_bytes(&[0x01, 0xaa, 0x01, 0xbb])).unwrap();
    assert_eq!(ok, (vec![0xaa], vec![0xbb]));
    assert!(parse_two_pushes(Script::from_bytes(&[0x01, 0xaa, 0x01, 0xbb, 0x01, 0xcc])).is_err());
    // 520 is MAX_SCRIPT_ELEMENT_SIZE. The next byte is the over-size push above.
    let mut at_cap = vec![0x4d, 0x08, 0x02];
    at_cap.extend([0x11u8; 520]);
    at_cap.extend([0x01, 0xbb]);
    let capped = parse_two_pushes(Script::from_bytes(&at_cap)).unwrap();
    assert_eq!(capped.0.len(), 520);
    assert_eq!(capped.1, vec![0xbb]);
}

#[test]
fn verify_pubkey_hash_mismatch() {
    let mut spk = vec![0x76, 0xa9, 0x14];
    spk.extend([0u8; 20]);
    spk.extend([0x88, 0xac]);
    let mut ss = vec![0x01, 0x30, 0x21];
    ss.extend([0x02; 33]); // fake compressed pubkey
    let tx = Transaction {
        version: bitcoin::transaction::Version::ONE,
        lock_time: LockTime::ZERO,
        input: vec![TxIn {
            previous_output: OutPoint::null(),
            script_sig: ScriptBuf::from_bytes(ss),
            sequence: Sequence::MAX,
            witness: Witness::new(),
        }],
        output: vec![TxOut {
            value: Amount::from_sat(1),
            script_pubkey: ScriptBuf::from_bytes(vec![0x51]),
        }],
    };
    let job = {
        let txid = [0u8; 32];
        let prevouts = vec![TxOut {
            value: Amount::from_sat(10),
            script_pubkey: ScriptBuf::from_bytes(spk),
        }];
        let tx = tx.clone();
        let flags = rbitcoin_consensus::block::ScriptVerifyFlags {
            bip65_active: true,
            bip112_active: true,
            bip66_active: true,
            bip16_active: true,
            taproot_active: true,
            minimal_if: false,
            nullfail: false,
            low_s: false,
            strictenc: false,
            null_dummy: false,
            minimal_data: false,
            witness_pubkeytype: false,
            witness_active: true,
            discourage_upgradable_witness: false,
            const_scriptcode: false,
            cleanstack: false,
        };
        ScriptCheckJob::with_txid(txid, prevouts, tx, flags)
    };
    let mut cache = SighashCache::new(&*job.tx);
    let err = {
        let i = job.inputs().next().unwrap();
        let spk = job.prevout_script(i).expect("prevout");
        let spk: &[u8; 25] = spk.try_into().expect("p2pkh template");
        verify(&job, i, &job.tx, &mut cache, spk)
    }
    .unwrap_err();
    assert!(format!("{err}").contains("p2pkh"));
}

mod sighash_script_code_tests {
    use super::*;
    use bitcoin::hashes::Hash;
    use bitcoin::secp256k1::{Secp256k1, SecretKey};
    use bitcoin::PublicKey;

    #[test]
    fn keyhash_collision_script_code_is_the_deleted_template() {
        let secp = Secp256k1::new();
        let sk = SecretKey::from_slice(&[9u8; 32]).unwrap();
        let pk = PublicKey::new(sk.public_key(&secp));
        let spk = ScriptBuf::new_p2pkh(&bitcoin::PubkeyHash::hash(&pk.to_bytes()));
        let keyhash = bitcoin::PubkeyHash::hash(&pk.to_bytes());
        let raw: &[u8; 25] = spk.as_bytes().try_into().unwrap();
        let code = sighash_script_code(raw, keyhash.as_byte_array());
        // OP_DUP OP_HASH160 OP_EQUALVERIFY OP_CHECKSIG. The 20-byte push is gone.
        // Borrowed: the fast path must not allocate a scriptCode for this hit.
        assert!(matches!(code, std::borrow::Cow::Borrowed(_)));
        assert_eq!(code.as_ref(), &[0x76, 0xa9, 0x88, 0xac]);
        assert_ne!(code.as_ref(), spk.as_bytes());
    }

    #[test]
    fn other_signatures_hash_the_template_in_place() {
        let secp = Secp256k1::new();
        let sk = SecretKey::from_slice(&[9u8; 32]).unwrap();
        let pk = PublicKey::new(sk.public_key(&secp));
        let spk = ScriptBuf::new_p2pkh(&bitcoin::PubkeyHash::hash(&pk.to_bytes()));
        let raw = spk.as_bytes();
        let typed: &[u8; 25] = raw.try_into().unwrap();
        let ordinary = [0x30u8; 71];
        let same = sighash_script_code(typed, &ordinary);
        assert!(matches!(same, std::borrow::Cow::Borrowed(_)));
        assert!(std::ptr::eq(same.as_ref(), raw));
        let mut miss = [0u8; 20];
        miss[0] = 0x30;
        let untouched = sighash_script_code(typed, &miss);
        assert!(std::ptr::eq(untouched.as_ref(), raw));
    }
}
