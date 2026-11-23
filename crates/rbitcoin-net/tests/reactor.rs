use rbitcoin_net::reactor::*;

#[cfg(test)]
mod off_pool_reentry {
    use super::*;
    use std::future::Future;
    use std::panic::{catch_unwind, AssertUnwindSafe};
    use std::task::Context;

    #[tokio::test(flavor = "multi_thread", worker_threads = 2)]
    async fn a_job_cannot_call_off_pool() {
        off_pool(|| {
            let mut fut = Box::pin(off_pool(|| ()));
            let waker = std::task::Waker::noop();
            let mut cx = Context::from_waker(waker);
            let panicked = catch_unwind(AssertUnwindSafe(|| {
                let _ = fut.as_mut().poll(&mut cx);
            }));
            assert!(panicked.is_err(), "nested off_pool must panic");
        })
        .await;
    }
}

#[cfg(test)]
mod store_permit_drop {
    use super::*;
    use std::sync::Arc;
    use std::time::Duration;
    use tokio::sync::Semaphore;

    #[tokio::test]
    async fn dropping_the_guard_returns_the_permit() {
        let sem = Arc::new(Semaphore::new(1));
        with_store_budget(sem, async {
            let first = acquire_armed_store_budget().await;
            let guard = hold_store_permit(first);
            drop(guard);
            let again =
                tokio::time::timeout(Duration::from_millis(200), acquire_armed_store_budget())
                    .await;
            assert!(
                again.is_ok(),
                "the dropped guard must release the store permit"
            );
        })
        .await;
    }
}

#[cfg(test)]
mod off_pool_lane {
    use super::*;
    use std::collections::HashSet;
    use std::sync::atomic::{AtomicUsize, Ordering};
    use std::sync::{Arc, Condvar, Mutex};
    use std::thread::ThreadId;
    use std::time::{Duration, Instant};

    /// Eight callers blocked inside `off_pool` still share one OS thread, and
    /// no two jobs are inside at once.
    ///
    /// The jobs record the thread they ran on and its name. A process-wide
    /// count of the `rbtc-off-pool` comm is not the witness: an unnamed thread
    /// that a lane job starts inherits that comm, and the other tests in this
    /// binary run beside this one.
    #[tokio::test(flavor = "multi_thread", worker_threads = 2)]
    async fn one_thread_runs_every_off_pool_job() {
        let arrived = Arc::new(AtomicUsize::new(0));
        let inside = Arc::new(AtomicUsize::new(0));
        let most_inside = Arc::new(AtomicUsize::new(0));
        let ran_on = Arc::new(Mutex::new(Vec::<(ThreadId, Option<String>)>::new()));
        let gate = Arc::new((Mutex::new(false), Condvar::new()));
        let mut tasks = Vec::new();
        for _ in 0..8 {
            let arrived = Arc::clone(&arrived);
            let inside = Arc::clone(&inside);
            let most_inside = Arc::clone(&most_inside);
            let ran_on = Arc::clone(&ran_on);
            let gate = Arc::clone(&gate);
            tasks.push(tokio::spawn(async move {
                arrived.fetch_add(1, Ordering::Release);
                off_pool(move || {
                    let now_inside = inside.fetch_add(1, Ordering::AcqRel) + 1;
                    most_inside.fetch_max(now_inside, Ordering::AcqRel);
                    let me = std::thread::current();
                    ran_on
                        .lock()
                        .expect("ran_on")
                        .push((me.id(), me.name().map(str::to_owned)));
                    let (lock, cv) = &*gate;
                    let mut go = lock.lock().expect("gate");
                    while !*go {
                        go = cv.wait(go).expect("gate");
                    }
                    inside.fetch_sub(1, Ordering::AcqRel);
                })
                .await;
            }));
        }
        let deadline = Instant::now() + Duration::from_secs(2);
        while arrived.load(Ordering::Acquire) < 8 || ran_on.lock().expect("ran_on").is_empty() {
            assert!(
                Instant::now() < deadline,
                "eight off-pool callers did not reach the lane"
            );
            tokio::task::yield_now().await;
        }
        {
            let (lock, cv) = &*gate;
            *lock.lock().expect("gate") = true;
            cv.notify_all();
        }
        for task in tasks {
            task.await.expect("off_pool task");
        }
        let ran_on = ran_on.lock().expect("ran_on").clone();
        assert_eq!(ran_on.len(), 8, "every off-pool job ran");
        let name = ran_on[0].1.as_deref();
        assert_eq!(name, Some("rbtc-off-pool"), "lane thread name: {name:?}");
        let threads: HashSet<ThreadId> = ran_on.into_iter().map(|(id, _)| id).collect();
        assert_eq!(
            threads.len(),
            1,
            "off_pool jobs ran on {} OS threads",
            threads.len()
        );
        let most_inside = most_inside.load(Ordering::Acquire);
        assert_eq!(
            most_inside, 1,
            "{most_inside} off_pool jobs were inside at once"
        );
    }
}
