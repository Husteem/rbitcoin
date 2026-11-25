//! Tokio **reactor** threads must not wait on std locks, store IO, or scripts.
//!
//! Tokio names both multi-thread workers and `spawn_blocking` threads
//! `tokio-rt-worker`. Blocking-pool work must enter [`BlockingRegion`].

use std::cell::{Cell, RefCell};
use std::future::Future;
use std::panic::{catch_unwind, resume_unwind, AssertUnwindSafe};
#[cfg(debug_assertions)]
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{mpsc, Arc, OnceLock};
use std::thread;
use tokio::sync::{OwnedSemaphorePermit, Semaphore};

tokio::task_local! {
    static API_STORE_BUDGET: Arc<Semaphore>;
    /// [`rbitcoin_query::Query::heavy_sh_join`] for [`armed_blocking`].
    static API_HEAVY_TURN: Arc<Semaphore>;
    #[cfg(debug_assertions)]
    static STORE_BUDGET_WAITERS: Arc<AtomicUsize>;
}

thread_local! {
    static BLOCKING_DEPTH: Cell<u32> = const { Cell::new(0) };
    static HELD_STORE_PERMIT: RefCell<Option<OwnedSemaphorePermit>> = const { RefCell::new(None) };
}

/// Guard: this OS thread may take std locks / store IO (blocking pool or tests).
pub struct BlockingRegion(());

impl BlockingRegion {
    pub fn enter() -> Self {
        BLOCKING_DEPTH.with(|d| d.set(d.get().saturating_add(1)));
        Self(())
    }
}

impl Drop for BlockingRegion {
    fn drop(&mut self) {
        BLOCKING_DEPTH.with(|d| d.set(d.get().saturating_sub(1)));
    }
}

pub fn on_tokio_worker() -> bool {
    thread::current()
        .name()
        .is_some_and(|n| n.starts_with("tokio-rt-worker"))
}

pub fn in_blocking_region() -> bool {
    BLOCKING_DEPTH.with(|d| d.get() > 0)
}

const OFF_POOL_THREAD: &str = "rbtc-off-pool";

thread_local! {
    static ON_OFF_POOL: Cell<bool> = const { Cell::new(false) };
}

fn on_off_pool_thread() -> bool {
    ON_OFF_POOL.with(|c| c.get())
}

fn reject_off_pool_reentry() {
    if on_off_pool_thread() {
        panic!("off_pool re-entered from rbtc-off-pool");
    }
}

fn off_pool_tx() -> mpsc::Sender<Box<dyn FnOnce() + Send>> {
    static TX: OnceLock<mpsc::Sender<Box<dyn FnOnce() + Send>>> = OnceLock::new();
    TX.get_or_init(|| {
        let (tx, rx) = mpsc::channel::<Box<dyn FnOnce() + Send>>();
        thread::Builder::new()
            .name(OFF_POOL_THREAD.into())
            .spawn(move || {
                while let Ok(job) = rx.recv() {
                    job();
                }
            })
            .expect("rbtc-off-pool");
        tx
    })
    .clone()
}

/// Queue `f` on the one `rbtc-off-pool` thread without waiting.
///
/// Cleanup that must leave a Tokio worker (a `Drop`) uses this. The job must
/// not call [`off_pool`], same as any other job on that thread.
pub(crate) fn queue_off_pool(f: impl FnOnce() + Send + 'static) {
    let job: Box<dyn FnOnce() + Send> = Box::new(f);
    let _ = off_pool_tx().send(job);
}

/// Run `f` on the one `rbtc-off-pool` thread and await it.
///
/// Not the Tokio blocking pool: a parked pool must still finish a connect or
/// a mempool admit. The thread starts on the first call and runs one job at
/// a time. Callers await between jobs, so a job must not call [`off_pool`]
/// again. Dropping the awaiter does not cancel `f`. Script verify stays on
/// the caller via `run_cpu`, outside `f`.
pub async fn off_pool<T: Send + 'static>(f: impl FnOnce() -> T + Send + 'static) -> T {
    reject_off_pool_reentry();
    let (tx, rx) = tokio::sync::oneshot::channel();
    let job = Box::new(move || {
        // The flag lives on this thread. The next job sets it again, and
        // nothing here reads it between jobs.
        ON_OFF_POOL.with(|c| c.set(true));
        let result = catch_unwind(AssertUnwindSafe(f));
        let _ = tx.send(result);
    });
    off_pool_tx().send(job).expect("rbtc-off-pool");
    match rx.await.expect("rbtc-off-pool dropped") {
        Ok(value) => value,
        Err(payload) => resume_unwind(payload),
    }
}

/// Run `fut` with the API store-read budget armed. Prepare takes one permit
/// and drops it before script verify. The commit coin re-read takes one
/// again. A task that never calls this does not take the budget (P2P decode,
/// and admits that are not an API read).
pub async fn with_store_budget<R>(budget: Arc<Semaphore>, fut: impl Future<Output = R>) -> R {
    API_STORE_BUDGET.scope(budget, fut).await
}

/// Run `fut` with the node's heavy scripthash turn armed for
/// [`armed_blocking`].
pub async fn with_heavy_turn<R>(turn: Arc<Semaphore>, fut: impl Future<Output = R>) -> R {
    API_HEAVY_TURN.scope(turn, fut).await
}

/// Same task as [`with_store_budget`]. [`acquire_armed_store_budget`] counts a
/// caller that found the budget empty. Debug builds only: tests wait on it.
#[cfg(debug_assertions)]
#[doc(hidden)]
pub async fn with_store_budget_waiters<R>(
    waiters: Arc<AtomicUsize>,
    fut: impl Future<Output = R>,
) -> R {
    STORE_BUDGET_WAITERS.scope(waiters, fut).await
}

/// One caller parked on an empty store budget, counted while it waits.
/// Debug builds only: tests wait on the count.
#[cfg(debug_assertions)]
#[doc(hidden)]
pub struct StoreBudgetWait(Arc<AtomicUsize>);

#[cfg(debug_assertions)]
impl StoreBudgetWait {
    pub fn enter(counter: Arc<AtomicUsize>) -> Self {
        counter.fetch_add(1, Ordering::SeqCst);
        Self(counter)
    }
}

#[cfg(debug_assertions)]
impl Drop for StoreBudgetWait {
    fn drop(&mut self) {
        self.0.fetch_sub(1, Ordering::SeqCst);
    }
}

/// Whether `e` carries a [`tokio::task::JoinError`]: a task this side spawned
/// panicked or was cancelled. Socket errors a client can cause are not.
pub fn is_join_error(e: &std::io::Error) -> bool {
    e.get_ref()
        .is_some_and(|inner| inner.is::<tokio::task::JoinError>())
}

/// The child's `Ok` value. A panicked child resumes that payload. Cancel
/// still aborts the caller.
pub fn finish_join<T>(joined: Result<T, tokio::task::JoinError>, cancel: &'static str) -> T {
    match joined {
        Ok(value) => value,
        Err(err) => match err.try_into_panic() {
            Ok(payload) => std::panic::resume_unwind(payload),
            Err(err) => panic!("{cancel}: {err}"),
        },
    }
}

/// `tokio::spawn` does not keep the task-local budget or heavy turn. Copy
/// them into the child so a detached admit still waits for a store permit.
pub fn spawn_with_store_budget<T: Send + 'static>(
    fut: impl Future<Output = T> + Send + 'static,
) -> tokio::task::JoinHandle<T> {
    let budget = API_STORE_BUDGET.try_with(Arc::clone).ok();
    let turn = API_HEAVY_TURN.try_with(Arc::clone).ok();
    tokio::spawn(async move {
        let fut = async move {
            match turn {
                Some(turn) => with_heavy_turn(turn, fut).await,
                None => fut.await,
            }
        };
        match budget {
            Some(budget) => with_store_budget(budget, fut).await,
            None => fut.await,
        }
    })
}

/// Run `f` on the blocking pool, one store permit from `permit` per run.
///
/// A heavy scripthash join in `f` that finds `turn` taken refuses at once
/// ([`rbitcoin_query::scripthash::heavy_turn_job`]). That run's thread and
/// permit go back, the turn is awaited on this task, and `f` runs once more
/// holding it, taking the turn before its store permit. The rerun is handed
/// the refused run's value, so state moved through `f` comes back. With no
/// `turn`, a refused run's value is the answer.
pub async fn blocking_heavy_turn<T, F, P, Fut>(
    turn: Option<&Arc<Semaphore>>,
    mut permit: P,
    mut f: F,
) -> Result<T, tokio::task::JoinError>
where
    T: Send + 'static,
    F: FnMut(Option<T>) -> T + Send + 'static,
    P: FnMut() -> Fut,
    Fut: Future<Output = Option<OwnedSemaphorePermit>>,
{
    let mut prev = None;
    let mut grant = None;
    loop {
        let store = permit().await;
        let held = grant.take();
        let (back, value, refused) = tokio::task::spawn_blocking(move || {
            let _store = store;
            let _g = BlockingRegion::enter();
            let (value, refused) = rbitcoin_query::scripthash::heavy_turn_job(held, || f(prev));
            (f, value, refused)
        })
        .await?;
        let Some(turn) = turn.filter(|_| refused) else {
            return Ok(value);
        };
        f = back;
        prev = Some(value);
        grant = Some(Arc::clone(turn).acquire_owned().await.expect("heavy turn"));
    }
}

/// [`blocking_heavy_turn`] with this task's armed store budget and heavy
/// turn ([`with_store_budget`], [`with_heavy_turn`]).
pub async fn armed_blocking<T: Send + 'static>(
    f: impl FnMut(Option<T>) -> T + Send + 'static,
) -> Result<T, tokio::task::JoinError> {
    let turn = API_HEAVY_TURN.try_with(Arc::clone).ok();
    blocking_heavy_turn(turn.as_ref(), acquire_armed_store_budget, f).await
}

/// One permit of the armed budget, or `None` when this task has no budget.
/// The wait is async. The caller moves the permit into the read so a dropped
/// request does not release it while the read still runs.
pub async fn acquire_armed_store_budget() -> Option<OwnedSemaphorePermit> {
    let Ok(sem) = API_STORE_BUDGET.try_with(Arc::clone) else {
        return None;
    };
    match Arc::clone(&sem).try_acquire_owned() {
        Ok(permit) => Some(permit),
        Err(_) => {
            #[cfg(debug_assertions)]
            let _waiting = STORE_BUDGET_WAITERS
                .try_with(Arc::clone)
                .ok()
                .map(StoreBudgetWait::enter);
            Some(sem.acquire_owned().await.expect("store budget"))
        }
    }
}

/// Park `permit` on this thread. [`release_store_permit`] drops it before
/// later CPU on the same call. The guard drops whatever is still held.
#[doc(hidden)]
pub struct StorePermitGuard;

#[doc(hidden)]
pub fn hold_store_permit(permit: Option<OwnedSemaphorePermit>) -> StorePermitGuard {
    HELD_STORE_PERMIT.with(|slot| {
        *slot.borrow_mut() = permit;
    });
    StorePermitGuard
}

/// Drop this thread's store-read permit. A thread that never held one is unchanged.
#[doc(hidden)]
pub fn release_store_permit() {
    HELD_STORE_PERMIT.with(|slot| {
        slot.borrow_mut().take();
    });
}

impl Drop for StorePermitGuard {
    fn drop(&mut self) {
        release_store_permit();
    }
}

/// [`off_pool`] that holds one armed store-read permit for `f` only.
pub async fn off_pool_budgeted<T: Send + 'static>(f: impl FnOnce() -> T + Send + 'static) -> T {
    let permit = acquire_armed_store_budget().await;
    off_pool(move || {
        let _permit = permit;
        f()
    })
    .await
}

pub fn assert_not_reactor(what: &'static str) {
    assert!(
        !on_tokio_worker() || in_blocking_region(),
        "{what} on tokio-rt-worker"
    );
}

pub use rbitcoin_primitives::assert_not_script_worker;
