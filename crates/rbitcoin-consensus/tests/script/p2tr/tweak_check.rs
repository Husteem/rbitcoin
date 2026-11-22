use bitcoin::consensus::Encodable;
use bitcoin::hashes::Hash;
use bitcoin::secp256k1::{Scalar, Secp256k1, SecretKey};
use bitcoin::taproot::{TapLeafHash, TapNodeHash, TapTweakHash};
use rbitcoin_consensus::error::ConsensusError;
use rbitcoin_consensus::script::p2tr::verify_control_commitment;

#[test]
fn control_commitment_uses_the_same_tweak_check() {
    let secp = Secp256k1::new();
    let sk = SecretKey::from_slice(&[4u8; 32]).unwrap();
    let (internal, _) = sk.x_only_public_key(&secp);
    let script = bitcoin::ScriptBuf::from_bytes(vec![0x51]);
    let mut eng = TapLeafHash::engine();
    0xc0u8
        .consensus_encode(&mut eng)
        .expect("hash engines do not error");
    script
        .consensus_encode(&mut eng)
        .expect("hash engines do not error");
    let tapleaf = TapLeafHash::from_engine(eng);
    let curr = TapNodeHash::from_byte_array(tapleaf.to_byte_array());
    let tweak = TapTweakHash::from_key_and_tweak(internal, Some(curr));
    let scalar = Scalar::from_be_bytes(tweak.to_byte_array()).unwrap();
    let (output, parity) = internal.add_tweak(&secp, &scalar).unwrap();
    let mut control = vec![0xc0 | parity.to_u8()];
    control.extend_from_slice(&internal.serialize());
    let (leaf, _) =
        verify_control_commitment(&control, &output.serialize(), script.as_script()).unwrap();
    assert_eq!(leaf, 0xc0);
    let err =
        verify_control_commitment(&control, &internal.serialize(), script.as_script()).unwrap_err();
    match err {
        ConsensusError::Script(msg) => {
            assert!(msg.contains("WITNESS_PROGRAM_MISMATCH"), "{msg}");
        }
        other => panic!("{other}"),
    }
}
