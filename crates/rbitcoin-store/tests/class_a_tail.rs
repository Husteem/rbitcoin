//! A synced Class A tail block torn to zeroes on the next append.
use rbitcoin_primitives::Fk;
use rbitcoin_store::store::Store;
use rbitcoin_store::tx_table::{InputRecord, OutputRecord, TxRecord};
use std::io::{Seek, SeekFrom, Write};
use std::path::Path;

fn tmp() -> std::path::PathBuf {
    let p = rbitcoin_store::testutil::fresh_path("class-a-tail");
    std::fs::create_dir_all(&p).unwrap();
    p
}

fn coinbase(txid: [u8; 32]) -> (TxRecord, Vec<InputRecord>, Vec<OutputRecord>) {
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

/// Zero the 4 KiB block that holds the file's published end.
fn zero_published_tail_block(path: &Path) {
    let mut file = std::fs::OpenOptions::new()
        .read(true)
        .write(true)
        .open(path)
        .unwrap();
    let mut header = [0u8; 16];
    std::io::Read::read_exact(&mut file, &mut header).unwrap();
    let logical = u64::from_le_bytes(header[8..16].try_into().unwrap());
    let block = logical & !4095;
    assert!(
        block >= 4096,
        "tail block {block} includes the file header (logical {logical})"
    );
    file.seek(SeekFrom::Start(block)).unwrap();
    file.write_all(&[0u8; 4096]).unwrap();
    file.sync_all().unwrap();
}

/// 128 creates fill `txid.body` to 4 KiB + 32, so create 128 sits in the
/// partial tail. A later append rewrites that block. Zeroing it is the
/// power cut. Open must still return the synced txid.
#[test]
fn torn_synced_tail_keeps_the_named_txid() {
    let dir = tmp();
    let named = 128u64;
    let txid = [0x80u8; 32];
    {
        let s = Store::create_tiny(&dir).unwrap();
        let batch: Vec<_> = (1..=named)
            .map(|i| {
                let mut id = [i as u8; 32];
                if i == named {
                    id = txid;
                }
                coinbase(id)
            })
            .collect();
        s.put_tx_full_batch_indexed(&batch, false).unwrap();
        s.txs.sync_class_a_named(named).unwrap();
        assert_eq!(s.txs.txids.get(Fk(named)).unwrap(), txid);
        let logical = s.txs.txids.logical_len();
        assert_ne!(
            logical % 4096,
            0,
            "txid.body ends on a block boundary, so the next append does not rewrite a partial tail"
        );
        s.put_tx_full_batch_indexed(&[coinbase([0x81u8; 32])], false)
            .unwrap();
    }
    zero_published_tail_block(&dir.join("txid.body"));
    let s = Store::open_tiny(&dir).unwrap();
    let got = s.txs.txids.get(Fk(named)).unwrap();
    assert_eq!(got, txid, "named txid became {got:?}");
    assert_eq!(s.get_tx(Fk(named)).unwrap().txid, txid);
    let _ = std::fs::remove_dir_all(&dir);
}
