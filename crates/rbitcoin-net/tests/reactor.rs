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

#[cfg(all(test, target_os = "linux"))]
mod off_pool_lane {
    use super::*;
    use std::sync::atomic::{AtomicUsize, Ordering};
    use std::sync::{Arc, Condvar, Mutex};
    use std::time::{Duration, Instant};

    fn off_pool_threads() -> usize {
        let mut n = 0usize;
        for entry in std::fs::read_dir("/proc/self/task").expect("task dir") {
            let path = entry.expect("task").path().join("comm");
            let Ok(comm) = std::fs::read_to_string(&path) else {
                continue;
            };
            if comm.trim() == "rbtc-off-pool" {
                n += 1;
            }
        }
        n
    }

    /// Eight callers blocked inside `off_pool` still share one OS thread.
    #[tokio::test(flavor = "multi_thread", worker_threads = 2)]
    async fn one_thread_runs_every_off_pool_job() {
        off_pool(|| ()).await;
        let (stop_tx, stop_rx) = std::sync::mpsc::channel::<()>();
        let extra = std::thread::Builder::new()
            .name("rbtc-off-pool".into())
            .spawn(move || {
                let _ = stop_rx.recv();
            })
            .expect("extra off-pool thread");
        let deadline = Instant::now() + Duration::from_secs(2);
        let mut named = off_pool_threads();
        while named < 2 {
            assert!(
                Instant::now() < deadline,
                "named rbtc-off-pool threads stayed at {named}"
            );
            tokio::task::yield_now().await;
            named = off_pool_threads();
        }
        drop(stop_tx);
        extra.join().expect("extra off-pool thread");
        let started = Arc::new(AtomicUsize::new(0));
        let arrived = Arc::new(AtomicUsize::new(0));
        let gate = Arc::new((Mutex::new(false), Condvar::new()));
        let mut tasks = Vec::new();
        for _ in 0..8 {
            let started = Arc::clone(&started);
            let arrived = Arc::clone(&arrived);
            let gate = Arc::clone(&gate);
            tasks.push(tokio::spawn(async move {
                arrived.fetch_add(1, Ordering::Release);
                off_pool(move || {
                    started.fetch_add(1, Ordering::Release);
                    let (lock, cv) = &*gate;
                    let mut go = lock.lock().expect("gate");
                    while !*go {
                        go = cv.wait(go).expect("gate");
                    }
                })
                .await;
            }));
        }
        let deadline = Instant::now() + Duration::from_secs(2);
        while arrived.load(Ordering::Acquire) < 8 {
            assert!(Instant::now() < deadline, "off-pool callers did not start");
            tokio::task::yield_now().await;
        }
        for _ in 0..64 {
            tokio::task::yield_now().await;
        }
        let threads = off_pool_threads();
        let running = started.load(Ordering::Acquire);
        {
            let (lock, cv) = &*gate;
            *lock.lock().expect("gate") = true;
            cv.notify_all();
        }
        for task in tasks {
            task.await.expect("off_pool task");
        }
        assert_eq!(
            threads, 1,
            "rbtc-off-pool threads: {threads}, jobs inside: {running}"
        );
        assert!(
            running < 8,
            "each job took its own thread ({running} inside at once)"
        );
    }
}
