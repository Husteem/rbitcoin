//! The store-read budget the API surfaces share: each enabled surface is
//! sure of its even split, may borrow idle permits up to one short of the
//! budget, and a freed permit goes first to a surface below its split.

use rbitcoin_query::ApiSurface::{Electrum, Rest, Rpc};
use rbitcoin_query::{ApiSurface, Query, StorePermit};
use std::sync::Arc;
use std::time::Duration;

fn budget(
    label: &str,
    size: usize,
    on: &[ApiSurface],
) -> (rbitcoin_query::testutil::TempDir, Arc<Query>) {
    let (dir, q) = rbitcoin_query::testutil::tiny_query_labeled(label);
    q.set_api_surfaces(on);
    q.set_api_read_budget(size);
    (dir, Arc::new(q))
}

/// A permit for `surface` within a short wait, or `None` when it waits.
async fn take(q: &Query, surface: ApiSurface) -> Option<StorePermit> {
    tokio::time::timeout(Duration::from_millis(100), q.acquire_api_read(surface))
        .await
        .ok()
}

/// A waiter for `surface` on its own task.
fn wait_for(q: &Arc<Query>, surface: ApiSurface) -> tokio::task::JoinHandle<StorePermit> {
    let q = Arc::clone(q);
    tokio::spawn(async move { q.acquire_api_read(surface).await })
}

/// One enabled surface may hold the whole budget.
#[tokio::test]
async fn a_sole_surface_holds_the_whole_budget() {
    let (_dir, q) = budget("budget-sole", 5, &[Electrum]);
    let mut held = Vec::new();
    for n in 0..5 {
        held.push(
            take(&q, Electrum)
                .await
                .unwrap_or_else(|| panic!("permit {n}")),
        );
    }
    assert!(take(&q, Electrum).await.is_none(), "the budget is spent");
}

/// With another surface idle, a busy surface borrows past its split, up to
/// one permit short of the budget.
#[tokio::test]
async fn a_busy_surface_borrows_idle_permits_up_to_its_ceiling() {
    let (_dir, q) = budget("budget-borrow", 4, &[Rpc, Electrum]);
    let mut held = Vec::new();
    for n in 0..3 {
        held.push(
            take(&q, Electrum)
                .await
                .unwrap_or_else(|| panic!("permit {n}")),
        );
    }
    assert!(
        take(&q, Electrum).await.is_none(),
        "one permit stays for the other surface"
    );
    assert!(take(&q, Rpc).await.is_some(), "RPC still reads at once");
}

/// A freed permit goes to a surface below its split before a surface that
/// would borrow, whichever waited first.
#[tokio::test]
async fn a_freed_permit_goes_first_to_a_surface_below_its_split() {
    let (_dir, q) = budget("budget-priority", 6, &[Rpc, Rest, Electrum]);
    let mut electrum = Vec::new();
    for n in 0..4 {
        electrum.push(
            take(&q, Electrum)
                .await
                .unwrap_or_else(|| panic!("permit {n}")),
        );
    }
    let _rest = [take(&q, Rest).await.unwrap(), take(&q, Rest).await.unwrap()];
    // The budget is spent. REST would borrow and asks first; RPC holds
    // nothing of its split and asks second.
    let mut rest = wait_for(&q, Rest);
    tokio::time::sleep(Duration::from_millis(50)).await;
    let mut rpc = wait_for(&q, Rpc);
    tokio::time::sleep(Duration::from_millis(50)).await;
    assert!(!rest.is_finished() && !rpc.is_finished(), "both wait");

    drop(electrum.pop());
    let first = tokio::time::timeout(Duration::from_secs(5), &mut rpc)
        .await
        .expect("RPC gets the freed permit")
        .unwrap();
    assert!(!rest.is_finished(), "REST still waits");
    drop(electrum.pop());
    tokio::time::timeout(Duration::from_secs(5), &mut rest)
        .await
        .expect("REST gets the next one")
        .unwrap();
    drop(first);
}

/// A wait dropped while queued leaves the queue, and a permit granted to a
/// wait that is dropped before it reads it goes back: a client that
/// disconnects while waiting holds nothing.
#[tokio::test]
async fn a_dropped_wait_holds_no_permit() {
    use rbitcoin_query::ApiReadBudget;

    let budget = ApiReadBudget::new(1, &[Rpc, Electrum]);
    let held = budget.try_acquire(Rpc).expect("the one permit");

    // Dropped while queued.
    let queued = {
        let budget = Arc::clone(&budget);
        tokio::spawn(async move { budget.acquire(Electrum).await })
    };
    tokio::time::sleep(Duration::from_millis(50)).await;
    assert_eq!(budget.waiting(Electrum), 1);
    queued.abort();
    let _ = queued.await;
    assert_eq!(
        budget.waiting(Electrum),
        0,
        "the dropped wait left the queue"
    );

    // Granted, then dropped before it is polled again.
    let mut wait = Box::pin(budget.acquire(Electrum));
    tokio::select! {
        biased;
        _ = &mut wait => panic!("the budget is spent"),
        () = std::future::ready(()) => {}
    }
    assert_eq!(budget.waiting(Electrum), 1);
    drop(held);
    assert_eq!(
        budget.held(Electrum),
        1,
        "the freed permit went to the wait"
    );
    drop(wait);
    assert_eq!(budget.available(), 1, "the unread grant came back");
    assert_eq!(budget.held(Rpc) + budget.held(Electrum), 0);
}

/// The limits per config: an even split with its remainder first, at least
/// one each; a ceiling one short of the budget beside another surface.
#[test]
fn the_budget_is_split_across_the_enabled_surfaces() {
    use rbitcoin_query::ApiReadBudget;
    use rbitcoin_query::ApiSurface::Esplora;

    let all = ApiReadBudget::new(7, &[Rpc, Rest, Esplora, Electrum]);
    let mins: Vec<usize> = [Rpc, Rest, Esplora, Electrum].map(|s| all.min(s)).to_vec();
    assert_eq!(mins, [2, 2, 2, 1]);
    assert_eq!(all.ceil(Electrum), 6);
    let sole = ApiReadBudget::new(6, &[Electrum]);
    assert_eq!((sole.min(Electrum), sole.ceil(Electrum)), (6, 6));
    assert_eq!(sole.min(Rpc), 0, "a surface that is off is sure of nothing");
    let hdd = ApiReadBudget::new(1, &[Rpc, Electrum]);
    assert_eq!((hdd.min(Rpc), hdd.ceil(Rpc)), (1, 1));
    let two = ApiReadBudget::new(2, &[Rpc, Electrum]);
    assert_eq!((two.min(Rpc), two.ceil(Rpc)), (1, 1), "nothing to borrow");
}
