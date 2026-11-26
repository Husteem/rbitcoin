//! The store-read budget the API surfaces share (`--store-read-budget`).
//!
//! Each surface the node serves is sure of an even split of the budget (its
//! minimum) and may borrow idle permits up to its ceiling: the whole budget
//! when it is the only surface, otherwise one permit short of it. A freed
//! permit goes first to a waiter whose surface holds less than its minimum,
//! then to one under its ceiling, round-robin across surfaces each time. A
//! read that is running is never taken back, so a surface below its minimum
//! waits at most until the next permit frees.

use std::collections::VecDeque;
use std::sync::{Arc, Mutex, MutexGuard};
use tokio::sync::oneshot;

/// An API surface with its own minimum and ceiling of
/// [`crate::Query::api_read_budget`].
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ApiSurface {
    /// JSON-RPC, including long-polls and templates.
    Rpc = 0,
    /// `--rest`.
    Rest = 1,
    /// Esplora and BlindBit, one listener.
    Esplora = 2,
    Electrum = 3,
}

const SURFACES: usize = 4;

/// Every surface, in the order the even split hands out its remainder.
pub const ALL_API_SURFACES: [ApiSurface; SURFACES] = [
    ApiSurface::Rpc,
    ApiSurface::Rest,
    ApiSurface::Esplora,
    ApiSurface::Electrum,
];

/// The process store-read budget, split across the surfaces it was built
/// for. Callers take a [`StorePermit`] on the async task and move it into
/// the blocking read.
pub struct ApiReadBudget {
    size: usize,
    min: [usize; SURFACES],
    ceil: [usize; SURFACES],
    state: Mutex<State>,
}

struct State {
    free: usize,
    held: [usize; SURFACES],
    queues: [VecDeque<Waiter>; SURFACES],
    next_id: u64,
    /// The surface a freed permit looks at first; it moves past each grant.
    turn: usize,
}

struct Waiter {
    id: u64,
    tx: oneshot::Sender<StorePermit>,
}

/// One permit of an [`ApiReadBudget`], counted against its surface.
/// Dropping it hands the permit to the next waiter.
pub struct StorePermit {
    budget: Arc<ApiReadBudget>,
    surface: ApiSurface,
}

impl Drop for StorePermit {
    fn drop(&mut self) {
        ApiReadBudget::release(&self.budget, self.surface);
    }
}

impl ApiReadBudget {
    /// `size` permits split across `enabled`. The enabled surfaces split it
    /// evenly, the remainder one permit each to the first of them in
    /// [`ALL_API_SURFACES`] order, at least one each. A surface that is off
    /// has no minimum. A surface's ceiling is `size` when no other surface is
    /// on, otherwise `size - 1`, never below its minimum and at least one.
    /// `0` waits forever.
    pub fn new(size: usize, enabled: &[ApiSurface]) -> Arc<Self> {
        let on: Vec<usize> = ALL_API_SURFACES
            .into_iter()
            .filter(|s| enabled.contains(s))
            .map(|s| s as usize)
            .collect();
        let one = size.min(1);
        let mut min = [0; SURFACES];
        if let Some(n) = std::num::NonZeroUsize::new(on.len()) {
            for (i, &s) in on.iter().enumerate() {
                min[s] = (size / n + usize::from(i < size % n)).max(one);
            }
        }
        let ceil = std::array::from_fn(|s| {
            let others = on.iter().any(|&t| t != s);
            let cap = if others { size.saturating_sub(1) } else { size };
            cap.max(min[s]).max(one)
        });
        Arc::new(Self {
            size,
            min,
            ceil,
            state: Mutex::new(State {
                free: size,
                held: [0; SURFACES],
                queues: Default::default(),
                next_id: 0,
                turn: 0,
            }),
        })
    }

    /// The permit count this budget was built with, held or not.
    pub fn size(&self) -> usize {
        self.size
    }

    /// Permits `surface` is sure of.
    pub fn min(&self, surface: ApiSurface) -> usize {
        self.min[surface as usize]
    }

    /// Permits `surface` may hold at once, borrowing.
    pub fn ceil(&self, surface: ApiSurface) -> usize {
        self.ceil[surface as usize]
    }

    /// Permits no one holds.
    pub fn available(&self) -> usize {
        self.lock().free
    }

    /// Permits `surface` holds.
    pub fn held(&self, surface: ApiSurface) -> usize {
        self.lock().held[surface as usize]
    }

    /// Callers of `surface` waiting for a permit.
    pub fn waiting(&self, surface: ApiSurface) -> usize {
        self.lock().queues[surface as usize].len()
    }

    /// A permit for `surface` now, or `None` when it would wait.
    pub fn try_acquire(self: &Arc<Self>, surface: ApiSurface) -> Option<StorePermit> {
        let mut st = self.lock();
        if !self.grantable(&st, surface) {
            return None;
        }
        Self::count(&mut st, surface as usize);
        drop(st);
        Some(self.permit(surface))
    }

    /// A permit for `surface`, waited for on this task. Dropping the wait
    /// leaves the queue; a permit granted to a dropped wait goes on to the
    /// next waiter.
    pub async fn acquire(self: &Arc<Self>, surface: ApiSurface) -> StorePermit {
        let (id, rx) = {
            let mut st = self.lock();
            if self.grantable(&st, surface) {
                Self::count(&mut st, surface as usize);
                drop(st);
                return self.permit(surface);
            }
            let (tx, rx) = oneshot::channel();
            let id = st.next_id;
            st.next_id += 1;
            st.queues[surface as usize].push_back(Waiter { id, tx });
            (id, rx)
        };
        let mut queued = Queued {
            budget: self,
            surface,
            id,
            answered: false,
        };
        let permit = rx.await.expect("a queued waiter is answered");
        queued.answered = true;
        permit
    }

    fn lock(&self) -> MutexGuard<'_, State> {
        self.state.lock().unwrap_or_else(|e| e.into_inner())
    }

    fn grantable(&self, st: &State, surface: ApiSurface) -> bool {
        let s = surface as usize;
        st.free > 0 && st.held[s] < self.ceil[s] && st.queues[s].is_empty()
    }

    fn count(st: &mut State, s: usize) {
        st.free -= 1;
        st.held[s] += 1;
    }

    fn permit(self: &Arc<Self>, surface: ApiSurface) -> StorePermit {
        StorePermit {
            budget: Arc::clone(self),
            surface,
        }
    }

    fn release(this: &Arc<Self>, surface: ApiSurface) {
        let grants = {
            let mut st = this.lock();
            st.held[surface as usize] -= 1;
            st.free += 1;
            this.dispatch(&mut st)
        };
        // Sent with the lock released: a permit whose waiter is gone comes
        // back through this function.
        for (tx, s) in grants {
            let _ = tx.send(this.permit(s));
        }
    }

    /// Grants for the waiters the free permits now reach, counted already.
    fn dispatch(&self, st: &mut State) -> Vec<(oneshot::Sender<StorePermit>, ApiSurface)> {
        let mut grants = Vec::new();
        while st.free > 0 {
            let Some(s) = self.next_waiter(st) else {
                break;
            };
            let Some(w) = st.queues[s].pop_front() else {
                break;
            };
            if w.tx.is_closed() {
                continue;
            }
            Self::count(st, s);
            st.turn = (s + 1) % SURFACES;
            grants.push((w.tx, ALL_API_SURFACES[s]));
        }
        grants
    }

    /// The surface whose front waiter the next free permit goes to: one
    /// below its minimum, else one under its ceiling, from `turn` on.
    fn next_waiter(&self, st: &State) -> Option<usize> {
        let order = || (0..SURFACES).map(|i| (st.turn + i) % SURFACES);
        let waits = |s: usize| !st.queues[s].is_empty();
        order()
            .find(|&s| waits(s) && st.held[s] < self.min[s])
            .or_else(|| order().find(|&s| waits(s) && st.held[s] < self.ceil[s]))
    }
}

/// A queued wait. Dropped before its permit arrives, it leaves the queue.
struct Queued<'a> {
    budget: &'a Arc<ApiReadBudget>,
    surface: ApiSurface,
    id: u64,
    answered: bool,
}

impl Drop for Queued<'_> {
    fn drop(&mut self) {
        if self.answered {
            return;
        }
        let mut st = self.budget.lock();
        st.queues[self.surface as usize].retain(|w| w.id != self.id);
    }
}
