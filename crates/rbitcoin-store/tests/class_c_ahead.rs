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
    file.write_all(&[0u8; 4096]).unwrap();
    file.seek(SeekFrom::Start(4096)).unwrap();
    file.write_all(&[0u8; 4096]).unwrap();
    file.sync_all().unwrap();
    let mut check = [1u8; 4096];
    file.seek(SeekFrom::Start(0)).unwrap();
    file.read_exact(&mut check).unwrap();
    assert_eq!(check, [0u8; 4096]);
}

#[test]
fn open_drops_header_txs_sidecars_from_different_flushes() {
    let dir = tmp();
    let count_side = dir.join("header_txs_count.body.ahead");
    let first_side = dir.join("header_txs_first.body.ahead");
    let saved_count = {
        let t = rbitcoin_store::HeaderTxsTable::create(&dir).unwrap();
        let write = rbitcoin_store::ClassCWrite::for_test();
        t.put_range(&write, Fk(1), Fk(1), 1).unwrap();
        t.flush().unwrap();
        t.put_range(&write, Fk(1), Fk(5), 3).unwrap();
        rbitcoin_store::chain::fail_next_header_txs_flush_after_ahead(
            &dir.join("header_txs_first.body"),
        );
        assert!(t.flush().is_err());
        let saved = std::fs::read(&count_side).unwrap();
        t.put_range(&write, Fk(1), Fk(8), 4).unwrap();
        rbitcoin_store::chain::fail_next_header_txs_flush_after_ahead(
            &dir.join("header_txs_first.body"),
        );
        assert!(t.flush().is_err());
        saved
    };
    std::fs::write(&count_side, &saved_count).unwrap();
    let t = rbitcoin_store::HeaderTxsTable::open(&dir).unwrap();
    assert_eq!(
        (t.first.get(0).unwrap(), t.count.get(0).unwrap()),
        (1, 1),
        "open restored first from one flush and count from another"
    );
    assert!(!first_side.exists());
    assert!(!count_side.exists());
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn open_restores_the_pair_when_a_retry_rewrites_one_sidecar() {
    let dir = tmp();
    let body = dir.join("header_txs_first.body");
    {
        let t = rbitcoin_store::HeaderTxsTable::create(&dir).unwrap();
        let write = rbitcoin_store::ClassCWrite::for_test();
        t.put_range(&write, Fk(1), Fk(1), 1).unwrap();
        t.flush().unwrap();
        t.put_range(&write, Fk(1), Fk(5), 3).unwrap();
        rbitcoin_store::chain::fail_next_header_txs_flush_after_first(&body);
        assert!(t.flush().is_err());
        rbitcoin_store::chain::fail_next_header_txs_flush_after_ahead(&body);
        assert!(t.flush().is_err());
    }
    let t = rbitcoin_store::HeaderTxsTable::open(&dir).unwrap();
    assert_eq!(
        (t.first.get(0).unwrap(), t.count.get(0).unwrap()),
        (5, 3),
        "open kept first 5 beside count 1"
    );
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn header_txs_flush_dirsyncs_the_pair_once() {
    let dir = tmp();
    let t = rbitcoin_store::HeaderTxsTable::create(&dir).unwrap();
    let write = rbitcoin_store::ClassCWrite::for_test();
    t.put_range(&write, Fk(1), Fk(1), 1).unwrap();
    rbitcoin_store::file::arm_fsync_dir_trace();
    t.flush().unwrap();
    let dirs = rbitcoin_store::file::take_fsynced_dirs();
    #[cfg(unix)]
    assert_eq!(
        dirs.len(),
        2,
        "the pair should sync the directory once after the sidecars and once after the unlinks, got {dirs:?}"
    );
    // Windows `fsync_parent_dir` is a no-op, so the trace stays empty.
    #[cfg(not(unix))]
    drop(dirs);
    let _ = std::fs::remove_dir_all(&dir);
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
        rbitcoin_store::chain::fail_next_header_txs_flush_after_ahead(
            &dir.join("header_txs_first.body"),
        );
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
    assert_eq!(
        s.header_txs
            .get_range(Fk(1))
            .unwrap()
            .map(|(f, n)| (f.0, n)),
        Some((1, 1)),
        "the page-zero range was not restored"
    );
    assert!(!dir.join("header_txs_first.body.ahead").exists());
    assert!(!dir.join("header_txs_count.body.ahead").exists());
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn leftover_ahead_pages_survive_a_later_write() {
    let dir = tmp();
    {
        let s = Store::create_tiny(&dir).unwrap();
        let write = rbitcoin_store::ClassCWrite::for_test();
        let mut parent_hash = [0u8; 32];
        let mut prev = Fk::NULL;
        for h in 0..1100u32 {
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
            if h == 599 {
                s.header_txs.flush().unwrap();
            }
        }
        rbitcoin_store::chain::fail_next_header_txs_flush_after_payload(
            &dir.join("header_txs_first.body"),
        );
        assert!(
            s.header_txs.flush().is_err(),
            "the failpoint must leave the sidecar after a successful payload"
        );
        assert!(dir.join("header_txs_first.body.ahead").is_file());

        let (rec, ins, outs) = coinbase(1100);
        let tx = s
            .put_tx_full_batch_indexed(&[(rec, ins, outs)], true)
            .unwrap()[0];
        let txid = s.txs.body_txid(tx).unwrap();
        let header = hdr(prev, parent_hash, 1100, txid);
        let hfk = s.put_header(&header).unwrap();
        s.header_txs.put_range(&write, hfk, tx, 1).unwrap();
        rbitcoin_store::chain::fail_next_header_txs_flush_after_ahead(
            &dir.join("header_txs_first.body"),
        );
        assert!(
            s.header_txs.flush().is_err(),
            "the later write must replace the sidecar"
        );
    }

    let first = dir.join("header_txs_first.body");
    let mut file = std::fs::OpenOptions::new()
        .read(true)
        .write(true)
        .open(&first)
        .unwrap();
    file.seek(SeekFrom::Start(4096)).unwrap();
    file.write_all(&[0u8; 4096]).unwrap();
    file.sync_all().unwrap();
    drop(file);

    let s = Store::open_tiny(&dir).unwrap();
    assert!(
        s.header_txs.has_body(Fk(600)).unwrap(),
        "the committed range on the first page was lost"
    );
    let _ = std::fs::remove_dir_all(&dir);
}

/// Every `confirmed[]` flush rewrites the length in page 0 and the block that
/// holds the previous tip. A tear of either must come back from the image.
#[test]
fn torn_confirmed_page_zero_restores_from_ahead_image() {
    use rbitcoin_primitives::Height;
    use rbitcoin_store::chain::ConfirmedTable;
    let dir = rbitcoin_store::testutil::TempDir::labeled("confirmed-ahead").unwrap();
    let path = dir.path.join("confirmed.body");
    {
        let t = ConfirmedTable::create(&dir.path).unwrap();
        let write = rbitcoin_store::ClassCWrite::for_test();
        for h in 0..600u32 {
            t.set(&write, Height(h), Fk(u64::from(h) + 1)).unwrap();
        }
        t.flush().unwrap();
        // Height 600 lands in the block that holds heights 510..=599.
        t.set(&write, Height(600), Fk(601)).unwrap();
        rbitcoin_store::chain::fail_next_header_txs_flush_after_ahead(&path);
        assert!(
            t.flush().is_err(),
            "the failpoint must stop before the payload flush"
        );
        assert!(dir.path.join("confirmed.body.ahead").is_file());
    }

    zero_header_and_block(&path);
    let t = ConfirmedTable::open(&dir.path).unwrap();
    assert_eq!(t.tip_height(), Some(Height(600)));
    assert_eq!(t.get(Height(0)).unwrap(), Some(Fk(1)), "page zero was lost");
    assert_eq!(
        t.get(Height(599)).unwrap(),
        Some(Fk(600)),
        "the committed previous tip was lost"
    );
    assert_eq!(t.get(Height(600)).unwrap(), Some(Fk(601)));
    assert!(!dir.path.join("confirmed.body.ahead").exists());
}

#[test]
fn one_header_txs_failpoint_does_not_clear_another_path() {
    let a = rbitcoin_store::testutil::TempDir::labeled("ahead-a").unwrap();
    let b = rbitcoin_store::testutil::TempDir::labeled("ahead-b").unwrap();
    let ta = rbitcoin_store::HeaderTxsTable::create(&a.path).unwrap();
    let tb = rbitcoin_store::HeaderTxsTable::create(&b.path).unwrap();
    let pa = a.path.join("header_txs_first.body");
    let pb = b.path.join("header_txs_first.body");
    rbitcoin_store::chain::fail_next_header_txs_flush_after_ahead(&pa);
    rbitcoin_store::chain::fail_next_header_txs_flush_after_ahead(&pb);
    assert!(tb.flush().is_err(), "path b was armed");
    assert!(
        ta.flush().is_err(),
        "arming path b cleared the failpoint for path a"
    );
    rbitcoin_store::chain::fail_next_header_txs_flush_after_payload(&pa);
    rbitcoin_store::chain::fail_next_header_txs_flush_after_payload(&pb);
    assert!(tb.flush().is_err(), "path b payload failpoint was armed");
    assert!(
        ta.flush().is_err(),
        "arming path b cleared the payload failpoint for path a"
    );
}
