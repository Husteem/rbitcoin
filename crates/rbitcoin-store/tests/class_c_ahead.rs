//! A header-txs append rewrites a synced partial block. The write-ahead image
//! must survive a tear between the sidecar and the payload flush.
use rbitcoin_primitives::Fk;
use rbitcoin_store::header_table::block_header_hash;
use rbitcoin_store::header_table::HeaderRecord;
use rbitcoin_store::store::Store;
use rbitcoin_store::tx_table::{InputRecord, OutputRecord, TxRecord};
use std::io::{Read, Seek, SeekFrom, Write};
use std::path::{Path, PathBuf};

fn tmp() -> PathBuf {
    let p = rbitcoin_store::testutil::fresh_path("class-c-ahead");
    std::fs::create_dir_all(&p).unwrap();
    p
}

fn coinbase(h: u32) -> (TxRecord, Vec<InputRecord>, Vec<OutputRecord>) {
    let mut txid = [0u8; 32];
    txid[..4].copy_from_slice(&h.to_le_bytes());
    txid[31] = 0xD9;
    (
        TxRecord {
            txid,
            version: 1,
            locktime: 0,
            input_count: 1,
            output_count: 1,
        },
        vec![InputRecord::coinbase(u32::MAX, vec![0x01], vec![])],
        vec![OutputRecord::unspent(50, vec![0x51])],
    )
}

fn hdr(prev: Fk, parent_hash: [u8; 32], h: u32, merkle: [u8; 32]) -> HeaderRecord {
    let version = 1i32;
    let timestamp = h + 1;
    let bits = 0x207f_ffff;
    let hash = if prev.is_null() {
        block_header_hash(version, &[0u8; 32], &merkle, timestamp, bits, h)
    } else {
        block_header_hash(version, &parent_hash, &merkle, timestamp, bits, h)
    };
    HeaderRecord {
        prev_fk: prev,
        version,
        timestamp,
        bits,
        nonce: h,
        merkle_root: merkle,
        hash,
        size: 0,
        weight: 0,
    }
}

/// Zero the first bytes and the 4 KiB block that holds the 512th range
/// onwards. The write-ahead image is the only copy left.
fn zero_header_and_block(path: &Path) {
    let mut file = std::fs::OpenOptions::new()
        .read(true)
        .write(true)
        .open(path)
        .unwrap();
    file.seek(SeekFrom::Start(0)).unwrap();
    file.write_all(&[0u8; 16]).unwrap();
    file.seek(SeekFrom::Start(4096)).unwrap();
    file.write_all(&[0u8; 4096]).unwrap();
    file.sync_all().unwrap();
    let mut check = [1u8; 16];
    file.seek(SeekFrom::Start(0)).unwrap();
    file.read_exact(&mut check).unwrap();
    assert_eq!(check, [0u8; 16]);
}

#[test]
fn torn_header_txs_block_restores_from_ahead_image() {
    let dir = tmp();
    let (h600, h601);
    {
        let s = Store::create_tiny(&dir).unwrap();
        let write = rbitcoin_store::ClassCWrite::for_test();
        let mut parent_hash = [0u8; 32];
        let mut prev = Fk::NULL;
        let mut last = None;
        for h in 0..600u32 {
            let (rec, ins, outs) = coinbase(h);
            let tx = s
                .put_tx_full_batch_indexed(&[(rec, ins, outs)], true)
                .unwrap()[0];
            let txid = s.txs.body_txid(tx).unwrap();
            let header = hdr(prev, parent_hash, h, txid);
            parent_hash = header.hash;
            let hfk = s.put_header(&header).unwrap();
            s.header_txs.put_range(&write, hfk, tx, 1).unwrap();
            prev = hfk;
            last = Some(hfk);
        }
        s.header_txs.flush().unwrap();
        h600 = last.unwrap();
        assert!(s.header_txs.has_body(h600).unwrap());

        // The 601st range starts in the block that holds ranges 510..=599.
        let (rec, ins, outs) = coinbase(600);
        let tx = s
            .put_tx_full_batch_indexed(&[(rec, ins, outs)], true)
            .unwrap()[0];
        let txid = s.txs.body_txid(tx).unwrap();
        let header = hdr(prev, parent_hash, 600, txid);
        let hfk = s.put_header(&header).unwrap();
        s.header_txs.put_range(&write, hfk, tx, 1).unwrap();
        h601 = hfk;
        rbitcoin_store::chain::fail_next_header_txs_flush_after_ahead();
        assert!(
            s.header_txs.flush().is_err(),
            "the failpoint must stop before the payload flush"
        );
        assert!(dir.join("header_txs_first.body.ahead").is_file());
        assert!(dir.join("header_txs_count.body.ahead").is_file());
    }

    zero_header_and_block(&dir.join("header_txs_first.body"));
    zero_header_and_block(&dir.join("header_txs_count.body"));
    let s = Store::open_tiny(&dir).unwrap();
    assert!(
        s.header_txs.has_body(h600).unwrap(),
        "the committed 600th range was lost"
    );
    assert!(
        s.header_txs.has_body(h601).unwrap(),
        "the write-ahead 601st range was lost"
    );
    assert_eq!(
        s.header_txs.get_range(h601).unwrap().map(|(f, n)| (f.0, n)),
        Some((601, 1))
    );
    assert!(!dir.join("header_txs_first.body.ahead").exists());
    assert!(!dir.join("header_txs_count.body.ahead").exists());
    let _ = std::fs::remove_dir_all(&dir);
}
