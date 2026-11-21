//! Identity-hasher maps for dense sequential integer keys (create_fk, abs, heights).
//!
//! **Intended keys:** Class A append ids, body abs offsets, block heights — dense
//! sequential `u64` / `u32` / [`Fk`]. Those land on consecutive hashbrown buckets
//! under a power-of-two mask → even occupancy. **Avoid** for keys that share low
//! bits (aligned pointers) or for cryptographic hashes (`[u8; 32]` / txid).
//!
//! hashbrown filters each probe group by a 7-bit tag taken from the top 7 bits
//! of the hash. These keys are far below 2^57, so the raw key gives every entry
//! the same tag and a probe compares every full slot of its group.
//! [`U64IdentityHasher::finish`] keeps the key in the low bits (same bucket) and
//! puts a multiplied tag in the top 7.
//!
//! Keys a power-of-two stride apart (8-byte `spent.body` slots) share low
//! bits, so identity placement would use one bucket per stride. Those use
//! [`StrideHasher`] ([`SlotMap`], [`SlotSet`]).

use rbitcoin_primitives::Fk;
use std::collections::{HashMap, HashSet};
use std::hash::{BuildHasherDefault, Hasher};

/// Bits above this are hashbrown's probe tag.
const TAG_SHIFT: u32 = 57;
const TAG_BITS: u64 = !((1u64 << TAG_SHIFT) - 1);
/// 2^64 / φ: a multiply by it moves every key bit into the high half.
const FIB_MUL: u64 = 0x9E37_79B9_7F4A_7C15;

/// Identity hasher: the low 57 bits of `finish()` are the key (no SipHash mix);
/// the top 7 carry a Fibonacci-multiplied tag.
#[derive(Default, Clone, Copy)]
pub struct U64IdentityHasher(u64);

impl Hasher for U64IdentityHasher {
    #[inline]
    fn write(&mut self, bytes: &[u8]) {
        for &b in bytes {
            self.0 = self
                .0
                .wrapping_mul(0x1000_0000_01b3)
                .wrapping_add(u64::from(b));
        }
    }
    #[inline]
    fn write_u64(&mut self, i: u64) {
        self.0 = i;
    }
    #[inline]
    fn write_u32(&mut self, i: u32) {
        self.0 = u64::from(i);
    }
    #[inline]
    fn finish(&self) -> u64 {
        self.0 ^ (self.0.wrapping_mul(FIB_MUL) & TAG_BITS)
    }
}

/// `HashMap` with [`U64IdentityHasher`] for dense `u64` keys.
pub type U64Map<V> = HashMap<u64, V, BuildHasherDefault<U64IdentityHasher>>;

/// `HashSet` with [`U64IdentityHasher`] for dense `u64` keys.
pub type U64Set = HashSet<u64, BuildHasherDefault<U64IdentityHasher>>;

/// `HashMap` with [`U64IdentityHasher`] for dense `u32` keys (heights, etc.).
pub type U32Map<V> = HashMap<u32, V, BuildHasherDefault<U64IdentityHasher>>;

/// `HashMap` with [`U64IdentityHasher`] for [`Fk`] keys (single-field u64).
pub type FkMap<V> = HashMap<Fk, V, BuildHasherDefault<U64IdentityHasher>>;

/// `HashSet` with [`U64IdentityHasher`] for [`Fk`] keys.
pub type FkSet = HashSet<Fk, BuildHasherDefault<U64IdentityHasher>>;

/// One multiply and a fold, for keys a power-of-two stride apart. The bucket
/// comes from the low bits, which a product alone takes only from the key's
/// low bits, so the product's high half folds down. The top 7 bits (the
/// probe tag) are the product's.
#[derive(Default, Clone, Copy)]
pub struct StrideHasher(u64);

impl Hasher for StrideHasher {
    #[inline]
    fn write(&mut self, bytes: &[u8]) {
        for &b in bytes {
            self.write_u64((self.0 << 8) | u64::from(b));
        }
    }

    #[inline]
    fn write_u64(&mut self, i: u64) {
        let h = i.wrapping_mul(FIB_MUL);
        self.0 = h ^ (h >> 32);
    }

    #[inline]
    fn finish(&self) -> u64 {
        self.0
    }
}

/// `HashMap` with [`StrideHasher`] for strided `u64` keys (slot offsets).
pub type SlotMap<V> = HashMap<u64, V, BuildHasherDefault<StrideHasher>>;

/// `HashSet` with [`StrideHasher`] for strided `u64` keys (slot offsets).
pub type SlotSet = HashSet<u64, BuildHasherDefault<StrideHasher>>;
