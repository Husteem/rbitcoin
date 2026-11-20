use super::tx_relay::{pad_cbs, spend_true, tmp};
use bitcoin::{ScriptBuf, Transaction, Txid};
use rbitcoin_mempool::RemovedRow;
use rbitcoin_net::tx_relay::MempoolHub;
use std::collections::BTreeSet;
use std::sync::Arc;

fn spend(cb: Txid, fee: u64) -> Transaction {
    spend_true(cb, fee, ScriptBuf::from_bytes(vec![0x51]))
}

fn sequence(hub: &MempoolHub) -> u64 {
    hub.lock_read().mempool_sequence()
}

/// A rollback takes back what the node staged. It numbers its changes and
/// writes no row, and a rolled-back replacement replaced nothing.
#[test]
fn package_and_one_parent_rollbacks_write_no_row() {
    let (_store, q, cbs) = pad_cbs(12);
    let dir = tmp();
    let hub = MempoolHub::open(&dir, q).unwrap();
    hub.set_relay_enabled(true);

    let lone = spend(cbs[0], 1_000);
    hub.accept_tx(&lone).expect("lone");
    let at = sequence(&hub);
    hub.rollback_1p1c_parent(&lone.compute_txid(), &[]);
    assert!(!hub.contains(&lone.compute_txid()));
    assert_eq!(sequence(&hub), at + 1);

    let victim = spend(cbs[1], 1_000);
    hub.accept_tx(&victim).expect("victim");
    let bump = hub.accept_tx(&spend(cbs[1], 50_000)).expect("replacement");
    let at = sequence(&hub);
    hub.rollback_package_accepted(std::slice::from_ref(&bump));
    assert!(hub.contains(&victim.compute_txid()), "the victim is back");
    assert!(!hub.contains(&bump.txid));
    assert_eq!(sequence(&hub), at + 2);
    assert_eq!(
        hub.lock_read().removed_rows().rows,
        Vec::<RemovedRow>::new()
    );
    let _ = std::fs::remove_dir_all(&dir);
}

fn listing(hub: &MempoolHub) -> (BTreeSet<Txid>, u64) {
    let (txids, seq) = hub.live_txids_and_sequence();
    (txids.into_iter().collect(), seq)
}

/// The txids and the sequence come from one read of the pool. With adds
/// only, a listing at sequence `s` holds exactly `s - 1` transactions.
#[test]
fn a_listing_under_concurrent_admits_matches_its_sequence() {
    let (_store, q, cbs) = pad_cbs(12);
    let dir = tmp();
    let hub = Arc::new(MempoolHub::open(&dir, q).unwrap());
    hub.set_relay_enabled(true);
    // The watcher lists once before any admit starts, so it does not depend
    // on which thread the scheduler runs first.
    let (listed, first_listing) = std::sync::mpsc::channel();
    let snaps = std::thread::scope(|s| {
        let watcher = s.spawn(|| {
            let mut snaps = vec![listing(&hub)];
            listed
                .send(())
                .expect("the test waits for the first listing");
            while hub.live_count() < cbs.len() {
                snaps.push(listing(&hub));
                std::thread::yield_now();
            }
            snaps
        });
        first_listing.recv().expect("the watcher listed");
        for chunk in cbs.chunks(3) {
            let hub = Arc::clone(&hub);
            s.spawn(move || {
                for cb in chunk {
                    hub.accept_tx(&spend(*cb, 1_000)).expect("admit");
                }
            });
        }
        watcher.join().unwrap()
    });
    for (txids, seq) in &snaps {
        assert_eq!(txids.len() as u64, seq - 1, "listing at {seq}: {txids:?}");
    }
    assert_eq!(listing(&hub).1, cbs.len() as u64 + 1);
    let _ = std::fs::remove_dir_all(&dir);
}
