use bitcoin::consensus::Encodable;
use bitcoin::hashes::Hash;
use bitcoin::secp256k1::{Parity, Scalar, Secp256k1, SecretKey};
use bitcoin::taproot::{TapLeafHash, TapNodeHash, TapTweakHash};
use rbitcoin_consensus::error::ConsensusError;
use rbitcoin_consensus::script::p2tr::{output_key_matches_tweak, verify_control_commitment};

/// secp256k1 order n.
const ORDER: [u8; 32] = [
    0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFE,
    0xBA, 0xAE, 0xDC, 0xE6, 0xAF, 0x48, 0xA0, 0x3B, 0xBF, 0xD2, 0x5E, 0x8C, 0xD0, 0x36, 0x41, 0x41,
];

fn sub32(a: [u8; 32], b: [u8; 32]) -> [u8; 32] {
    let mut out = [0u8; 32];
    let mut borrow = 0i16;
    for i in (0..32).rev() {
        let v = i16::from(a[i]) - i16::from(b[i]) - borrow;
        if v < 0 {
            out[i] = (v + 256) as u8;
            borrow = 1;
        } else {
            out[i] = v as u8;
            borrow = 0;
        }
    }
    assert_eq!(borrow, 0, "underflow");
    out
}

#[test]
fn tweak_at_or_above_order_is_not_a_commitment() {
    let secp = Secp256k1::new();
    let sk = SecretKey::from_slice(&[3u8; 32]).unwrap();
    let (internal, _) = sk.x_only_public_key(&secp);
    // n reduced to 0 is P + 0*G = P. That must still fail.
    assert!(!output_key_matches_tweak(
        &internal,
        &internal,
        Parity::Even,
        &ORDER
    ));
    // 2^256-1 reduced mod n is a non-zero residue. Q = P + rG must still fail.
    let above = [0xffu8; 32];
    let residue = sub32(above, ORDER);
    let scalar = Scalar::from_be_bytes(residue).unwrap();
    let (q, parity) = internal.add_tweak(&secp, &scalar).unwrap();
    assert!(!output_key_matches_tweak(&internal, &q, parity, &above));
    let mut max = ORDER;
    max[31] = 0x40;
    let scalar = Scalar::from_be_bytes(max).unwrap();
    let (output, parity) = internal.add_tweak(&secp, &scalar).unwrap();
    assert!(output_key_matches_tweak(&internal, &output, parity, &max));
}

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
