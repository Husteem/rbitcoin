use rbitcoin_primitives::Fk;
use rbitcoin_store::error::StoreError;
use rbitcoin_store::testutil::TempDir;
use rbitcoin_store::txstat::*;

fn tiny(fee: u64, base: u32, wit: u32) -> TxStatRow {
    TxStatRow {
        fee_sat: fee,
        base,
        wit_extra: wit,
    }
}

#[test]
fn txstat_blob_longer_than_the_file_is_corrupt() {
    let err = checked_blob_len(1 << 30, 32).expect_err("huge len");
    match err {
        StoreError::Corrupt(msg) => assert!(msg.contains("txstat"), "{msg}"),
        other => panic!("{other:?}"),
    }
    assert_eq!(checked_blob_len(4, 32).unwrap(), 4);
}

#[test]
fn torn_txstat_page_restores_from_ahead_image() {
    let dir = TempDir::labeled("txstat-ahead").unwrap();
    let path = dir.join("txstat.body");
    let rows: Vec<TxStatRow> = (1..=601u64).map(|i| tiny(i, 100, 10)).collect();
    {
        let t = TxStat::create(&dir).unwrap();
        t.append_batch(0, &rows).unwrap();
        fail_next_txstat_flush_after_ahead(&path);
        assert!(
            t.write_block_rows(Fk(1), 510, &rows[509..601]).is_err(),
            "the failpoint must stop before the cell pwrite"
        );
        assert!(dir.join("txstat.body.ahead").is_file());
    }
    let mut file = std::fs::OpenOptions::new()
        .read(true)
        .write(true)
        .open(&path)
        .unwrap();
    use std::io::{Seek, SeekFrom, Write};
    file.seek(SeekFrom::Start(0)).unwrap();
    file.write_all(&[0u8; 4096]).unwrap();
    file.seek(SeekFrom::Start(4096)).unwrap();
    file.write_all(&[0u8; 4096]).unwrap();
    file.sync_all().unwrap();
    drop(file);

    let t = TxStat::open(&dir).unwrap();
    assert_eq!(t.count(), 601);
    assert_eq!(
        t.get_cell(Fk(509)).unwrap(),
        pack_cell(&rows[508]).unwrap().0
    );
    assert_eq!(
        t.get_cell(Fk(601)).unwrap(),
        pack_cell(&rows[600]).unwrap().0
    );
    assert!(!dir.join("txstat.body.ahead").exists());
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn leftover_ahead_pages_survive_a_later_write() {
    let dir = TempDir::labeled("txstat-ahead-merge").unwrap();
    let path = dir.join("txstat.body");
    let rows: Vec<TxStatRow> = (1..=601u64).map(|i| tiny(i, 100, 10)).collect();
    {
        let t = TxStat::create(&dir).unwrap();
        t.append_batch(0, &rows).unwrap();
        fail_next_txstat_flush_after_ahead(&path);
        assert!(
            t.write_block_rows(Fk(1), 510, &rows[509..601]).is_err(),
            "the failpoint must stop before the cell pwrite"
        );
        fail_next_txstat_flush_after_ahead(&path);
        assert!(
            t.write_row(Fk(1), &tiny(9, 9, 9)).is_err(),
            "the later write must replace the sidecar"
        );
    }
    let mut file = std::fs::OpenOptions::new()
        .read(true)
        .write(true)
        .open(&path)
        .unwrap();
    use std::io::{Seek, SeekFrom, Write};
    file.seek(SeekFrom::Start(4096)).unwrap();
    file.write_all(&[0u8; 4096]).unwrap();
    file.sync_all().unwrap();
    drop(file);

    let t = TxStat::open(&dir).unwrap();
    assert_eq!(t.count(), 601);
    assert_eq!(
        t.get_cell(Fk(510)).unwrap(),
        pack_cell(&rows[509]).unwrap().0,
        "the committed cell on the first page was lost"
    );
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn append_does_not_replay_a_stale_ahead_over_new_tail() {
    let dir = TempDir::labeled("txstat-stale-ahead").unwrap();
    let path = dir.join("txstat.body");
    let first: Vec<TxStatRow> = (1..=601u64).map(|i| tiny(i, 100, 10)).collect();
    let extra: Vec<TxStatRow> = (602..=700u64).map(|i| tiny(i, 100, 10)).collect();
    {
        let t = TxStat::create(&dir).unwrap();
        t.append_batch(0, &first).unwrap();
        fail_next_txstat_flush_after_ahead(&path);
        assert!(t.write_row(Fk(1), &tiny(9, 9, 9)).is_err());
        t.append_batch(601, &extra).unwrap();
        assert_eq!(t.count(), 700);
    }
    let t = TxStat::open(&dir).unwrap();
    assert_eq!(t.count(), 700);
    assert_eq!(
        t.get_cell(Fk(700)).unwrap(),
        pack_cell(&extra[98]).unwrap().0,
        "restore replayed a stale image over the appended tail"
    );
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn create_empty_then_extend_zeros() {
    let dir = TempDir::labeled("txstat-extend").unwrap();
    let t = TxStat::create(&dir).unwrap();
    assert_eq!(t.count(), 0);
    t.extend_or_truncate_to(2).unwrap();
    assert_eq!(t.count(), 2);
    assert_eq!(t.get_cell(Fk(1)).unwrap(), [0u8; 8]);
    assert_eq!(t.get_cell(Fk(2)).unwrap(), [0u8; 8]);
    t.truncate_to_count(1).unwrap();
    assert_eq!(t.count(), 1);
    assert!(t.get_cell(Fk(2)).is_err());
}

#[test]
fn uleb_typical_fits_in_cell() {
    let row = tiny(1000, 110, 112);
    let (cell, tail) = pack_cell(&row).unwrap();
    assert!(tail.is_none());
    assert_eq!(parse_cell(cell).unwrap(), CellParse::Complete(row));
    assert_eq!(row.size(), 222);
    assert_eq!(row.weight(), 4 * 110 + 112);
}

#[test]
fn former_n_in_byte_no_longer_forces_a_tail() {
    let row = tiny(50_000, 200, 200_000);
    let (cell, tail) = pack_cell(&row).unwrap();
    assert!(tail.is_none(), "three fields of this row fit in 8 B");
    assert_eq!(parse_cell(cell).unwrap(), CellParse::Complete(row));
}

#[test]
fn overflow_is_remaining_bytes_only() {
    let row = tiny(u64::from(u32::MAX), 4_000_000, 4_000_000);
    let stream = encode_stream(&row).unwrap();
    assert!(stream.len() > 8, "wide fee and sizes must miss 8 B");
    let (cell, tail) = pack_cell(&row).unwrap();
    let tail = tail.expect("tail");
    assert_eq!(tail.as_slice(), &stream[8..]);
    assert_eq!(parse_cell(cell).unwrap(), CellParse::NeedTail);
    assert_eq!(parse_with_tail(cell, &tail).unwrap(), row);
}

#[test]
fn header_blob_roundtrip() {
    let dir = TempDir::labeled("txstat-ovf").unwrap();
    let t = TxStat::create(&dir).unwrap();
    let small = tiny(1000, 110, 0);
    let fat = tiny(u64::from(u32::MAX), 4_000_000, 4_000_000);
    t.append_batch(0, &[small, fat]).unwrap();
    t.write_block_rows(Fk(1), 1, &[small, fat]).unwrap();
    let blob = t.header_blob(Fk(1)).unwrap();
    assert!(!blob.is_empty());
    let got = t.get_range(1, 2, Some(&blob)).unwrap();
    assert_eq!(got[0], Some(small));
    assert_eq!(got[1], Some(fat));
    assert_eq!(t.get_row_merged(Fk(2), 1, &blob).unwrap(), Some(fat));
}

#[test]
fn leftover_txfixed_body_unlinked_on_open() {
    let dir = TempDir::labeled("txstat-leftover").unwrap();
    std::fs::write(dir.join("txfixed.body"), b"junk").unwrap();
    let _t = TxStat::open(&dir).unwrap();
    assert!(!dir.join("txfixed.body").exists());
    assert!(dir.join("txstat.body").is_file());
}

#[test]
fn range_reads_the_cell_and_header_blob_reads_the_slot() {
    let dir = TempDir::labeled("txstat-slot").unwrap();
    let t = TxStat::create(&dir).unwrap();
    let small = tiny(1000, 110, 0);
    let fat = tiny(u64::from(u32::MAX), 4_000_000, 4_000_000);
    t.append_batch(0, &[small, fat]).unwrap();
    assert_eq!(t.get_range(1, 1, None).unwrap(), vec![Some(small)]);
    let err = match t.get_range(1, 4, None) {
        Ok(_) => panic!("range accepted a fk past the count"),
        Err(err) => err,
    };
    assert!(matches!(err, StoreError::NotFound), "{err:?}");

    assert!(t.header_blob(Fk(1)).unwrap().is_empty());
    t.write_block_rows(Fk(1), 1, &[small, fat]).unwrap();
    let blob = t.header_blob(Fk(1)).unwrap();
    assert!(!blob.is_empty());
    let got = t.get_range(1, 2, Some(&blob)).unwrap();
    assert_eq!(got[0], Some(small));
    assert_eq!(got[1], Some(fat));
    assert!(t.header_blob(Fk(2)).unwrap().is_empty());
}

/// Readers stamp unstamped blocks (Esplora block routes, `getblock`), so
/// `put_header_blob` runs on several threads at once. Each header must read
/// back its own blob. Equal-length blobs make a shared offset silent: the
/// slot still decodes, with another block's tails.
#[test]
fn concurrent_header_blobs_keep_their_own_bytes() {
    const THREADS: u64 = 8;
    const PER: u64 = 400;
    let dir = TempDir::labeled("txstat-ovf-race").unwrap();
    let t = std::sync::Arc::new(TxStat::create(&dir).unwrap());
    let blob_for = |fk: u64| {
        let tail = fk.to_le_bytes().repeat(4);
        encode_ovf_blob(&[(1, tail)]).unwrap()
    };
    let start = std::sync::Arc::new(std::sync::Barrier::new(THREADS as usize));
    let handles: Vec<_> = (0..THREADS)
        .map(|i| {
            let t = std::sync::Arc::clone(&t);
            let start = std::sync::Arc::clone(&start);
            std::thread::spawn(move || {
                start.wait();
                for k in 0..PER {
                    let fk = 1 + k * THREADS + i;
                    t.put_header_blob(Fk(fk), &blob_for(fk)).unwrap();
                }
            })
        })
        .collect();
    for h in handles {
        h.join().unwrap();
    }
    let total: u64 = (1..=THREADS * PER)
        .map(|fk| blob_for(fk).len() as u64)
        .sum();
    assert_eq!(
        t.ovf.data_len(),
        total,
        "appends neither overlap nor leave gaps"
    );
    let wrong: Vec<u64> = (1..=THREADS * PER)
        .filter(|&fk| t.header_blob(Fk(fk)).unwrap() != blob_for(fk))
        .collect();
    assert!(wrong.is_empty(), "headers read another blob: {wrong:?}");
}

fn ahead_body(logical: u64) -> Vec<u8> {
    let mut h = vec![0u8; 20];
    h[0..4].copy_from_slice(b"TTA1");
    h[4..6].copy_from_slice(&1u16.to_le_bytes());
    h[8..12].copy_from_slice(&1u32.to_le_bytes());
    h[12..20].copy_from_slice(&logical.to_le_bytes());
    h
}

fn ahead_page(off: u64) -> Vec<u8> {
    use bitcoin_hashes::{sha256, Hash, HashEngine};
    let mut image = vec![0u8; 4096];
    image[0] = 9;
    let mut out = Vec::new();
    out.extend_from_slice(&off.to_le_bytes());
    let mut eng = sha256::HashEngine::default();
    eng.input(&off.to_le_bytes());
    eng.input(&image);
    out.extend_from_slice(&sha256::Hash::from_engine(eng).to_byte_array());
    out.extend_from_slice(&image);
    out
}

#[test]
fn txstat_open_rejects_every_bad_ahead_shape() {
    let dir = TempDir::labeled("txstat-ahead-bad").unwrap();
    let side = dir.join("txstat.body.ahead");
    let t = TxStat::create(&dir).unwrap();
    drop(t);

    let mut zero = ahead_body(4096);
    zero[8..12].copy_from_slice(&0u32.to_le_bytes());
    let mut bad_magic = ahead_body(4096);
    bad_magic[0] ^= 1;
    let mut bad_ver = ahead_body(4096);
    bad_ver[4] = 2;
    let mut short = ahead_body(4096);
    short.extend_from_slice(&[0u8; 10]);
    let mut misaligned = ahead_body(4096);
    let mut bad_sum = ahead_page(4096);
    bad_sum[8] ^= 1;
    let mut trailing = ahead_body(4096);
    trailing.extend_from_slice(&ahead_page(4096));
    trailing.push(0);

    let mut huge = ahead_body(4096);
    huge[8..12].copy_from_slice(&u32::MAX.to_le_bytes());
    std::fs::write(&side, huge).unwrap();
    assert!(TxStat::open(&dir).is_err());

    let body_path = dir.join("txstat.body");
    let before = std::fs::read(&body_path).unwrap();
    let mut prefix_then_trailing = ahead_body(4096);
    prefix_then_trailing.extend_from_slice(&ahead_page(0));
    prefix_then_trailing.push(0);
    std::fs::write(&side, prefix_then_trailing).unwrap();
    assert!(TxStat::open(&dir).is_err());
    assert_eq!(
        std::fs::read(&body_path).unwrap(),
        before,
        "a malformed journal must not apply its prefix"
    );

    for (case, raw) in [
        zero,
        bad_magic,
        bad_ver,
        short,
        {
            misaligned.extend_from_slice(&ahead_page(4096));
            misaligned[20..28].copy_from_slice(&1u64.to_le_bytes());
            misaligned
        },
        {
            let mut bad_off = ahead_body(4096);
            bad_off.extend_from_slice(&ahead_page(4097));
            bad_off
        },
        {
            let mut b = ahead_body(8192);
            b.extend_from_slice(&bad_sum);
            b
        },
        trailing,
    ]
    .into_iter()
    .enumerate()
    {
        std::fs::write(&side, raw).unwrap();
        assert!(TxStat::open(&dir).is_err(), "case {case} was accepted");
    }
    let _ = std::fs::remove_dir_all(dir.path());
}

#[test]
fn append_batch_relies_on_the_class_a_tail_guard() {
    let dir = TempDir::labeled("txstat-append-no-ahead").unwrap();
    let t = TxStat::create(&dir).unwrap();
    t.append_batch(0, &[tiny(1, 2, 3)]).unwrap();
    assert_eq!(t.count(), 1);
    assert!(
        !dir.join("txstat.body.ahead").exists(),
        "an append-only batch must not copy the appended range"
    );
    let _ = std::fs::remove_dir_all(dir.path());
}
