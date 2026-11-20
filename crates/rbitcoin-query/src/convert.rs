use bitcoin::block::Header;
use bitcoin::hashes::Hash;
use rbitcoin_primitives::Fk;
use rbitcoin_store::HeaderRecord;

/// Header row from wire fields. `hash` is the caller-computed `header.block_hash()`.
pub fn header_to_record(prev_fk: Fk, header: &Header, hash: [u8; 32]) -> HeaderRecord {
    HeaderRecord {
        prev_fk,
        version: header.version.to_consensus(),
        timestamp: header.time,
        bits: header.bits.to_consensus(),
        nonce: header.nonce,
        merkle_root: header.merkle_root.to_byte_array(),
        hash,
        size: 0,
        weight: 0,
    }
}
