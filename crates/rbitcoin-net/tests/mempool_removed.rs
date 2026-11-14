use super::tx_relay::{copy_maturity_pad, pad_cbs, spend_true, tmp};
use bitcoin::{OutPoint, ScriptBuf, Transaction, Txid};
use rbitcoin_mempool::{RemovalReason, RemovedRow};
use rbitcoin_net::tx_relay::MempoolHub;
use std::collections::BTreeSet;
use std::sync::Arc;

fn spend(cb: Txid, fee: u64) -> Transaction {
    spend_true(cb, fee, ScriptBuf::from_bytes(vec![0x51]))
}

fn child_of(parent: &Transaction, fee: u64) -> Transaction {
    let mut tx = spend(parent.compute_txid(), 0);
    tx.output[0].value = parent.output[0].value - bitcoin::Amount::from_sat(fee);
    tx
}

fn sequence(hub: &MempoolHub) -> u64 {
    hub.lock_read().mempool_sequence()
}

fn reasons(hub: &MempoolHub) -> Vec<(Txid, RemovalReason)> {
    hub.lock_read()
        .removed_rows()
        .rows
        .into_iter()
        .map(|r| (r.txid, r.reason))
        .collect()
}

#[test]
fn hub_removals_carry_their_reason_and_sequence() {
    let (_store, q, cbs) = pad_cbs(12);
    let dir = tmp();
    let hub = MempoolHub::open(&dir, q).unwrap();
    hub.set_relay_enabled(true);
    hub.note_mock_now(10_000);

    let first = spend(cbs[0], 1_000);
    let bump = spend(cbs[0], 50_000);
    hub.accept_tx(&first).expect("first");
    hub.accept_tx(&bump).expect("replacement");
    let loser = spend(cbs[1], 1_000);
    let loser_child = child_of(&loser, 1_000);
    hub.accept_tx(&loser).expect("loser");
    hub.accept_tx(&loser_child).expect("loser child");
    let mined_at = sequence(&hub);
    hub.remove_for_block_spent(&[bump.compute_txid()], &[OutPoint::new(cbs[1], 0)]);

    let stale = spend(cbs[3], 1_000);
    hub.accept_tx(&stale).expect("stale");
    hub.set_expiry_hours(1);
    hub.note_mock_now(10_000 + 3_600 + 5);
    let expired_at = sequence(&hub);
    assert_eq!(hub.expire_stale(), 1);

    let low = spend(cbs[4], 1_000);
    let high = spend(cbs[5], 50_000);
    hub.accept_tx(&low).expect("low");
    hub.accept_tx(&high).expect("high");
    hub.set_max_weight(high.weight().to_wu());
    hub.trim_over_budget();

    let got = reasons(&hub);
    assert_eq!(got.len(), 6, "{got:?}");
    assert_eq!(
        got[0],
        (
            first.compute_txid(),
            RemovalReason::Replaced {
                by: bump.compute_txid()
            }
        )
    );
    assert_eq!(got[1], (bump.compute_txid(), RemovalReason::Block));
    assert_eq!(
        got[2..4].iter().copied().collect::<BTreeSet<_>>(),
        BTreeSet::from([
            (loser.compute_txid(), RemovalReason::Conflict),
            (loser_child.compute_txid(), RemovalReason::Conflict),
        ]),
        "a block conflict takes its descendant with the same reason"
    );
    assert_eq!(got[4], (stale.compute_txid(), RemovalReason::Expiry));
    assert_eq!(got[5], (low.compute_txid(), RemovalReason::SizeLimit));
    let rows = hub.lock_read().removed_rows().rows;
    assert_eq!(rows[1].seq, mined_at, "the mined tx took the next number");
    assert_eq!(rows[4].seq, expired_at);
    assert_eq!(hub.lock_read().removed_rows().oldest, 1);
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn reorg_evict_keeps_reorg_for_the_tree() {
    let (store, q, cbs) = copy_maturity_pad(3);
    let dir = tmp();
    let hub = MempoolHub::open(&dir, Arc::clone(&q)).unwrap();
    hub.set_relay_enabled(true);
    let parent = spend(cbs[0], 1_000);
    let child = child_of(&parent, 1_000);
    hub.accept_tx(&parent).expect("parent");
    hub.accept_tx(&child).expect("child");
    while q.tip_height().map(|h| h.0).unwrap_or(0) > 0 {
        q.disconnect_tip().unwrap();
    }
    hub.evict_after_reorg();
    let got: BTreeSet<_> = reasons(&hub).into_iter().collect();
    assert_eq!(
        got,
        BTreeSet::from([
            (parent.compute_txid(), RemovalReason::Reorg),
            (child.compute_txid(), RemovalReason::Reorg),
        ])
    );
    assert_eq!(sequence(&hub), 5);
    let _ = std::fs::remove_dir_all(&dir);
    let _ = std::fs::remove_dir_all(&store);
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

/// What a client does with two listings: each txid in the first and not in
/// the second has exactly one row numbered between their sequences.
#[test]
fn two_listings_and_the_rows_between_them_explain_every_disappearance() {
    let (_store, q, cbs) = pad_cbs(12);
    let dir = tmp();
    let hub = MempoolHub::open(&dir, q).unwrap();
    hub.set_relay_enabled(true);
    hub.note_mock_now(10_000);
    let mut listings = vec![listing(&hub)];

    let a = spend(cbs[0], 1_000);
    let b = spend(cbs[1], 1_000);
    let b_child = child_of(&b, 1_000);
    let c = spend(cbs[2], 1_000);
    for tx in [&a, &b, &b_child, &c] {
        hub.accept_tx(tx).expect("admit");
        listings.push(listing(&hub));
    }
    let a_bump = spend(cbs[0], 50_000);
    hub.accept_tx(&a_bump).expect("replacement");
    listings.push(listing(&hub));
    hub.remove_for_block_spent(&[b.compute_txid()], &[OutPoint::new(cbs[2], 0)]);
    listings.push(listing(&hub));
    hub.note_mock_now(10_000 + 3_605);
    let d = spend(cbs[3], 1_000);
    let e = spend(cbs[4], 50_000);
    hub.accept_tx(&d).expect("d");
    hub.accept_tx(&e).expect("e");
    listings.push(listing(&hub));
    hub.set_expiry_hours(1);
    hub.expire_stale();
    listings.push(listing(&hub));
    hub.set_max_weight(e.weight().to_wu());
    hub.trim_over_budget();
    listings.push(listing(&hub));

    let last = listings.last().unwrap();
    assert_eq!(last.0, BTreeSet::from([e.compute_txid()]));
    let want = [
        (
            a.compute_txid(),
            RemovalReason::Replaced {
                by: a_bump.compute_txid(),
            },
        ),
        (b.compute_txid(), RemovalReason::Block),
        (c.compute_txid(), RemovalReason::Conflict),
        (a_bump.compute_txid(), RemovalReason::Expiry),
        (b_child.compute_txid(), RemovalReason::Expiry),
        (d.compute_txid(), RemovalReason::SizeLimit),
    ];
    let removed = hub.lock_read().removed_rows();
    assert_eq!(removed.oldest, 1);
    let mut checked = 0;
    for (i, (first, s1)) in listings.iter().enumerate() {
        for (second, s2) in &listings[i..] {
            for gone in first.difference(second) {
                let rows: Vec<&RemovedRow> = removed
                    .rows
                    .iter()
                    .filter(|r| r.txid == *gone && *s1 <= r.seq && r.seq < *s2)
                    .collect();
                assert_eq!(rows.len(), 1, "{gone} between {s1} and {s2}: {rows:?}");
                let reason = want.iter().find(|(t, _)| t == gone).map(|(_, r)| *r);
                assert_eq!(Some(rows[0].reason), reason, "{gone}");
                checked += 1;
            }
            if first == second {
                assert_eq!(s1, s2, "same members, same sequence here");
            }
        }
    }
    assert!(checked >= want.len());
    let _ = std::fs::remove_dir_all(&dir);
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
