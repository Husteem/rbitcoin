use rbitcoin_primitives::{Fk, TableKind};
use rbitcoin_store::error::StoreError;
use rbitcoin_store::file::TableFile;
use rbitcoin_store::scripthash::{
    pop_free_head, read_alloc_header, write_alloc_header, AllocState,
};
use rbitcoin_store::scripthash_layout::{slab_bytes, SH_MAX_CLASS, SH_PREFIX_PAGE};
use rbitcoin_store::scripthash_pages::SH_FLAG_BIT;
use rbitcoin_store::scripthash_slabs::*;

fn encode_stream(fks: &[u64]) -> Result<Vec<u8>, StoreError> {
    let mut out = vec![0u8; fks.len().saturating_mul(10).max(1)];
    let n = encode_fk_delta_stream_into(&mut out, fks)?;
    out.truncate(n);
    Ok(out)
}

fn decode_stream(buf: &[u8], n: usize) -> Result<Vec<Fk>, StoreError> {
    let mut out = Vec::new();
    decode_fk_delta_stream_into(buf, n, &mut out)?;
    Ok(out)
}

fn encode_slab(fks: &[u64]) -> Result<Vec<u8>, StoreError> {
    let mut out = vec![0u8; 2 + fks.len().saturating_mul(10).max(1)];
    let n = encode_slab_payload_into(&mut out, fks)?;
    out.truncate(n);
    Ok(out)
}

fn decode_slab(buf: &[u8]) -> Result<Vec<Fk>, StoreError> {
    let mut out = Vec::new();
    decode_slab_payload_into(buf, &mut out)?;
    Ok(out)
}

#[test]
fn alloc_header_rejects_a_free_head_past_bump() {
    let dir = rbitcoin_store::testutil::fresh_path("sh-alloc-range");
    std::fs::create_dir_all(&dir).unwrap();
    let path = dir.join("scripthash.body");
    let file = TableFile::create(&path, TableKind::ScriptHash).unwrap();
    file.set_logical_len(SH_PREFIX_PAGE as u64).unwrap();
    let mut state = AllocState {
        live_count: 0,
        bump: SH_PREFIX_PAGE as u64,
        free_head: [0u64; SH_MAX_CLASS as usize + 1],
    };
    state.free_head[0] = state.bump.saturating_add(8);
    write_alloc_header(&file, &state).unwrap();
    match read_alloc_header(&file) {
        Err(StoreError::Corrupt(_)) => {}
        Ok((_, version)) => {
            panic!("out-of-range free head must be Corrupt, read version {version}")
        }
        Err(other) => panic!("out-of-range free head must be Corrupt, got {other}"),
    }
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn slab_class_picks_smallest_fit_and_slack() {
    assert_eq!(slab_class_for_n_fks(0), None);
    assert_eq!(slab_class_for_n_fks(1), None);
    assert_eq!(slab_class_for_n_fks(2), Some(0));
    assert_eq!(slab_class_for_n_fks(3), Some(0));
    assert_eq!(slab_class_for_n_fks(4), Some(0));
    assert_eq!(slab_class_for_n_fks(5), Some(1));
    assert_eq!(slab_class_for_n_fks(8), Some(1));
    assert_eq!(slab_class_for_n_fks(9), Some(2));
    assert_eq!(slab_class_for_n_fks(256), Some(6));
    assert_eq!(slab_class_for_n_fks(257), None);
    assert_eq!(slab_class_for_n_fks_with_slack(4), Some(1));
    assert_eq!(slab_class_for_n_fks_with_slack(256), Some(6));
    let five: Vec<u64> = (1..=5u64).collect();
    let packed = encode_slab(&five).unwrap();
    assert_eq!(slab_class_for_packed_len(packed.len()), Some(0));
    let two = [1u64, 2];
    let packed2 = encode_slab(&two).unwrap();
    assert_eq!(slab_bytes(0), 16);
    assert_eq!(slab_class_for_packed_len(packed2.len()), Some(0));
    assert!(
        packed2.len() as u64 <= slab_bytes(0),
        "2-fk ULEB must fit class 0 16 B (len={})",
        packed2.len()
    );
    let three_hundred: Vec<u64> = (1..=300u64).collect();
    let packed = encode_slab(&three_hundred).unwrap();
    let class = slab_class_for_packed_len(packed.len()).expect("300 tight deltas fit a slab");
    assert!(class <= 5, "class={class} packed={}", packed.len());
    assert_eq!(slab_class_for_packed_len(0), None);
    assert_eq!(slab_class_for_packed_len(2049), None);
}

#[test]
fn fk_delta_stream_roundtrip_and_packed_len() {
    let cases: &[&[u64]] = &[
        &[],
        &[1],
        &[3, 4, 5, 7],
        &[1_000_000_000, 1_000_000_003, 1_000_000_010],
        &[10, 11, 12, 13, 14, 15, 16, 20],
    ];
    for raw in cases {
        let fks: Vec<Fk> = raw.iter().copied().map(Fk).collect();
        let stream = encode_stream(raw).unwrap();
        assert!(
            stream.len() <= 8 * fks.len() || fks.is_empty(),
            "packed length {} > 8×n={}",
            stream.len(),
            fks.len()
        );
        let got = decode_stream(&stream, fks.len()).unwrap();
        assert_eq!(got, fks);
        let mut into = Vec::new();
        decode_fk_delta_stream_into(&stream, fks.len(), &mut into).unwrap();
        assert_eq!(into, got);
        let slab = encode_slab(raw).unwrap();
        assert_eq!(decode_slab(&slab).unwrap(), fks);
        let mut slab_got = Vec::new();
        decode_slab_payload_into(&slab, &mut slab_got).unwrap();
        assert_eq!(slab_got, fks);
        assert_eq!(slab.len(), 2 + stream.len());
        let mut stream_into = vec![0u8; raw.len().saturating_mul(10).max(1)];
        let sn = encode_fk_delta_stream_into(&mut stream_into, raw).unwrap();
        assert_eq!(&stream_into[..sn], stream.as_slice());
        let mut slab_into = vec![0u8; 2 + raw.len().saturating_mul(10).max(1)];
        let pn = encode_slab_payload_into(&mut slab_into, raw).unwrap();
        assert_eq!(&slab_into[..pn], slab.as_slice());
    }
    assert!(encode_stream(&[5, 5]).is_err());
    assert!(encode_stream(&[0]).is_err());
    assert!(encode_stream(&[SH_FLAG_BIT | 1]).is_err());
    assert!(encode_stream(&[1, SH_FLAG_BIT | 2]).is_err());
    assert!(decode_stream(&[0x00], 1).is_err());
    let mut flagged = Vec::new();
    rbitcoin_primitives::write_uleb128(&mut flagged, SH_FLAG_BIT);
    assert!(decode_stream(&flagged, 1).is_err());
    assert!(decode_stream(&[0x01, 0x00], 2).is_err());
    assert!(decode_slab(&[0]).is_err());
    assert!(decode_stream(&[], 0).unwrap().is_empty());
    let mut keep = vec![Fk(99)];
    decode_fk_delta_stream_into(&[], 0, &mut keep).unwrap();
    assert_eq!(keep, vec![Fk(99)]);
    assert!(decode_slab_payload_into(&[0], &mut Vec::new()).is_err());
}

#[test]
fn popped_free_link_out_of_range_is_corrupt() {
    let dir = rbitcoin_store::testutil::fresh_path("sh-free-link");
    std::fs::create_dir_all(&dir).unwrap();
    let path = dir.join("scripthash.body");
    let file = TableFile::create(&path, TableKind::ScriptHash).unwrap();
    file.set_logical_len(8192).unwrap();
    let mut state = AllocState {
        live_count: 0,
        bump: 8192,
        free_head: [0u64; SH_MAX_CLASS as usize + 1],
    };
    state.free_head[0] = 4096;
    write_alloc_header(&file, &state).unwrap();
    file.write_at(4096, &8200u64.to_le_bytes()).unwrap();
    let mut heads = state.free_head;
    match pop_free_head(&file, &mut heads, 0) {
        Err(StoreError::Corrupt(_)) => {}
        Ok(_) => panic!("an out-of-range next link must be Corrupt"),
        Err(e) => panic!("an out-of-range next link must be Corrupt, got {e}"),
    }
    let _ = std::fs::remove_dir_all(&dir);
}
