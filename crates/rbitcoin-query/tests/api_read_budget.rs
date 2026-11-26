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

/// Wait until `n` callers of `surface` are queued.
async fn queued(budget: &rbitcoin_query::ApiReadBudget, surface: ApiSurface, n: usize) {
    tokio::time::timeout(Duration::from_secs(5), async {
        while budget.waiting(surface) != n {
            tokio::task::yield_now().await;
        }
    })
    .await
    .unwrap_or_else(|_| panic!("{n} {surface:?} waiters queue"));
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
    let budget = q.api_read_budget();
    let mut rest = wait_for(&q, Rest);
    queued(&budget, Rest, 1).await;
    let mut rpc = wait_for(&q, Rpc);
    queued(&budget, Rpc, 1).await;
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
    self::queued(&budget, Electrum, 1).await;
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

/// Freed permits that only borrowers want go round the surfaces in turn,
/// and a surface's waiters are served in the order they asked.
#[tokio::test]
async fn borrowers_take_freed_permits_in_turn() {
    let (_dir, q) = budget("budget-turn", 6, &[Rpc, Rest, Electrum]);
    let budget = q.api_read_budget();
    let _rpc = [take(&q, Rpc).await.unwrap(), take(&q, Rpc).await.unwrap()];
    let _rest = [take(&q, Rest).await.unwrap(), take(&q, Rest).await.unwrap()];
    let mut electrum = vec![
        take(&q, Electrum).await.unwrap(),
        take(&q, Electrum).await.unwrap(),
    ];
    // Every surface holds its split. Two RPC waiters ask, then two REST.
    let mut waits = Vec::new();
    for (s, n) in [(Rpc, 1), (Rpc, 2), (Rest, 1), (Rest, 2)] {
        waits.push(wait_for(&q, s));
        queued(&budget, s, n).await;
    }
    assert_eq!((budget.waiting(Rpc), budget.waiting(Rest)), (2, 2));

    drop(electrum.pop());
    let _first = tokio::time::timeout(Duration::from_secs(5), &mut waits[0])
        .await
        .expect("the first RPC waiter is served first")
        .unwrap();
    drop(electrum.pop());
    let _second = tokio::time::timeout(Duration::from_secs(5), &mut waits[2])
        .await
        .expect("then REST, not the second RPC waiter")
        .unwrap();
    assert!(!waits[1].is_finished(), "the second RPC waiter still waits");
    assert_eq!((budget.held(Rpc), budget.held(Rest)), (3, 3));
    for w in waits {
        w.abort();
    }
}

/// With one permit every surface on is below its minimum whenever it holds
/// nothing, so under steady demand from one surface the permit still goes
/// round the surfaces that wait.
#[tokio::test]
async fn one_permit_goes_round_the_waiting_surfaces() {
    let (_dir, q) = budget("budget-one", 1, &[Rpc, Electrum]);
    let budget = q.api_read_budget();
    let held = take(&q, Rpc).await.expect("the one permit");
    let mut rpc = wait_for(&q, Rpc);
    queued(&budget, Rpc, 1).await;
    let mut electrum = wait_for(&q, Electrum);
    queued(&budget, Electrum, 1).await;
    let mut more_rpc = wait_for(&q, Rpc);
    queued(&budget, Rpc, 2).await;

    drop(held);
    let next = tokio::time::timeout(Duration::from_secs(5), &mut rpc)
        .await
        .expect("RPC's first waiter gets the permit")
        .unwrap();
    drop(next);
    let next = tokio::time::timeout(Duration::from_secs(5), &mut electrum)
        .await
        .expect("then Electrum, though RPC still waits")
        .unwrap();
    assert!(!more_rpc.is_finished());
    drop(next);
    let _last = tokio::time::timeout(Duration::from_secs(5), &mut more_rpc)
        .await
        .expect("then RPC again")
        .unwrap();
}
