use rbitcoin_primitives::Fk;
use rbitcoin_store::int_map::*;
use std::hash::Hasher;

fn hash_u64(key: u64) -> u64 {
    let mut h = U64IdentityHasher::default();
    h.write_u64(key);
    h.finish()
}

const LOW_57: u64 = (1 << 57) - 1;

/// hashbrown places a key by the low bits and filters a probe group by a
/// 7-bit tag from the top bits. Every id here is below 2^57, so a raw-key
/// hash gave all of them tag 0 and each probe compared every full slot.
#[test]
fn u64_identity_hasher_keeps_the_key_low_and_spreads_the_probe_tag() {
    let mut tags = std::collections::HashSet::new();
    for key in 1..=4096u64 {
        let h = hash_u64(key);
        assert_eq!(h & LOW_57, key, "bucket placement is the key");
        tags.insert(h >> 57);
    }
    assert!(tags.len() >= 120, "{} distinct tags of 128", tags.len());

    let mut h3 = U64IdentityHasher::default();
    h3.write_u32(0xabcd_u32);
    assert_eq!(h3.finish() & LOW_57, 0xabcd_u64);
}

#[test]
fn u64_identity_hasher_map_roundtrips_pack_scale() {
    // Byte-path mix (Hash trait fallback for non-u64 keys).
    let mut h2 = U64IdentityHasher::default();
    h2.write(&[0x01, 0x02, 0x03, 0x04]);
    assert_ne!(h2.finish(), 0);

    let n = 8_000u64;
    let mut m: U64Map<u32> = U64Map::with_capacity_and_hasher(n as usize, Default::default());
    for i in 1..=n {
        m.insert(i, (i % 1_000_000) as u32);
    }
    assert_eq!(m.len(), n as usize);
    for i in 1..=n {
        assert_eq!(m.get(&i).copied(), Some((i % 1_000_000) as u32));
    }
    assert_eq!(m.get(&0), None);
    assert_eq!(m.get(&(n + 1)), None);

    let mut s: U64Set = U64Set::with_capacity_and_hasher(n as usize, Default::default());
    for i in 1..=n {
        s.insert(i);
    }
    assert_eq!(s.len(), n as usize);
    assert!(s.contains(&1));
    assert!(!s.contains(&(n + 1)));

    let mut fm: FkMap<u8> = FkMap::default();
    fm.insert(Fk(42), 7);
    assert_eq!(fm.get(&Fk(42)).copied(), Some(7));

    let mut fs: FkSet = FkSet::default();
    assert!(fs.insert(Fk(1)));
    assert!(!fs.insert(Fk(1)));
    assert!(fs.contains(&Fk(1)));
}
