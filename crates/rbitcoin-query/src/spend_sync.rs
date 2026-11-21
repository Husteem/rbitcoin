//! Background device barrier for spend replay stems.
//!
//! Confirm records a snapshot height and does not `sync_data`. This thread
//! does, about every ten minutes, and publishes only the height it read
//! before the barrier.

use std::sync::{Arc, Condvar, Mutex};
use std::thread::JoinHandle;
use std::time::{Duration, Instant};

use rbitcoin_store::SPEND_DURABLE_INTERVAL_MS;

/// Park after a failed checkpoint while the edge cap still wants another one.
/// A full ten-minute park would leave confirm blocked on that cap.
const SPEND_FAIL_BACKOFF_MS: u64 = 1000;
/// How long a repeatedly failing checkpoint waits between WARN lines.
const SPEND_FAIL_WARN_GAP_MS: u64 = 60_000;

/// What one checkpoint failure should log.
#[doc(hidden)]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum CheckpointFailAction {
    /// First failure since the last success.
    First,
    /// The 60s reminder while it keeps failing.
    Again,
    /// Inside the gap; no line.
    Quiet,
}

/// Rate-limit the checkpoint failure WARN without hiding that it is still
/// failing. A recovery line is emitted once after any failure.
#[doc(hidden)]
#[derive(Clone, Copy, Debug, Default)]
pub struct CheckpointFailLog {
    failures: u32,
    next_warn_ms: u64,
}

impl CheckpointFailLog {
    pub fn on_failure(&mut self, now_ms: u64) -> CheckpointFailAction {
        self.failures = self.failures.saturating_add(1);
        if self.failures == 1 {
            self.next_warn_ms = now_ms.saturating_add(SPEND_FAIL_WARN_GAP_MS);
            return CheckpointFailAction::First;
        }
        if now_ms >= self.next_warn_ms {
            self.next_warn_ms = now_ms.saturating_add(SPEND_FAIL_WARN_GAP_MS);
            return CheckpointFailAction::Again;
        }
        CheckpointFailAction::Quiet
    }

    /// Consecutive failures this success clears, if any.
    pub fn on_success(&mut self) -> Option<u32> {
        let failures = self.failures;
        self.failures = 0;
        self.next_warn_ms = 0;
        (failures > 0).then_some(failures)
    }
}

use crate::Query;

pub struct SpendSync {
    pub wake: Arc<(Mutex<bool>, Condvar)>,
    pub join: Option<JoinHandle<()>>,
}

impl SpendSync {
    pub fn spawn(query: Arc<Query>) -> Self {
        let wake = Arc::new((Mutex::new(false), Condvar::new()));
        query.store().bind_spend_sync_wake(Arc::clone(&wake));
        let flag = Arc::clone(&wake);
        let join = std::thread::Builder::new()
            .name("rbtc-spend-sync".into())
            .spawn(move || run(query, flag))
            .expect("spawn spend sync");
        Self {
            wake,
            join: Some(join),
        }
    }

    /// One checkpoint at the current snapshot, then join.
    pub fn shutdown(mut self) {
        self.stop_and_join();
    }

    #[doc(hidden)]
    pub fn stop_and_join(&mut self) {
        {
            let (lock, cv) = &*self.wake;
            let mut stop = lock.lock().unwrap();
            *stop = true;
            cv.notify_one();
        }
        if let Some(join) = self.join.take() {
            let _ = join.join();
        }
    }
}

impl Drop for SpendSync {
    fn drop(&mut self) {
        self.stop_and_join();
    }
}

#[doc(hidden)]
pub fn run(query: Arc<Query>, wake: Arc<(Mutex<bool>, Condvar)>) {
    let (lock, cv) = &*wake;
    let started = Instant::now();
    let mut fail_log = CheckpointFailLog::default();
    let mut since_ok = Instant::now();
    loop {
        let stop = {
            let guard = lock.lock().unwrap();
            if *guard {
                true
            } else if query.store().spend_checkpoint_early() {
                false
            } else {
                let elapsed = since_ok.elapsed().as_millis() as u64;
                let wait = SPEND_DURABLE_INTERVAL_MS.saturating_sub(elapsed);
                if wait == 0 {
                    false
                } else {
                    let (guard, _) = cv.wait_timeout(guard, Duration::from_millis(wait)).unwrap();
                    *guard
                }
            }
        };
        let elapsed = since_ok.elapsed().as_millis() as u64;
        let due = query.store().spend_checkpoint_early() || elapsed >= SPEND_DURABLE_INTERVAL_MS;
        if !stop && !due {
            continue;
        }
        let published = match checkpoint(&query) {
            Ok(()) => {
                if let Some(failures) = fail_log.on_success() {
                    rbitcoin_log::info!(
                        "store: spend checkpoint recovered after {failures} failure(s)"
                    );
                }
                true
            }
            Err(e) => {
                let now_ms = started.elapsed().as_millis() as u64;
                match fail_log.on_failure(now_ms) {
                    CheckpointFailAction::First => {
                        rbitcoin_log::warn!("store: spend checkpoint: {e}");
                    }
                    CheckpointFailAction::Again => {
                        rbitcoin_log::warn!(
                            "store: spend checkpoint still failing ({failures} in a row): {e}",
                            failures = fail_log.failures
                        );
                    }
                    CheckpointFailAction::Quiet => {}
                }
                false
            }
        };
        query.store().relax_spend_pace();
        if published {
            since_ok = Instant::now();
        } else if !stop {
            // A failed publish stays due. While the edge cap is armed, wait
            // one second. Otherwise wait the full interval, even when this
            // attempt started with a fresh timer.
            let park = if query.store().spend_checkpoint_early() {
                SPEND_FAIL_BACKOFF_MS
            } else {
                SPEND_DURABLE_INTERVAL_MS
            };
            let guard = lock.lock().unwrap();
            if !*guard {
                let _ = cv.wait_timeout(guard, Duration::from_millis(park)).unwrap();
            }
        }
        if stop {
            break;
        }
    }
}

#[doc(hidden)]
pub fn checkpoint(query: &Query) -> Result<(), rbitcoin_store::StoreError> {
    let started = Instant::now();
    let result = query.store().checkpoint_observed_spend();
    if result.is_ok() {
        let notes = query.store().txs.spend_delta_stats().notes_bytes;
        query
            .confirm_stats()
            .note_spend_checkpoint(started.elapsed().as_millis() as u64, notes);
    }
    result
}
