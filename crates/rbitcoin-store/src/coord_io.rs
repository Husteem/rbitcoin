//! Debug flag set while the IBD coordinator holds its shared state lock.
//!
//! Release builds keep the type and report the flag as never set.

#[cfg(debug_assertions)]
use std::cell::Cell;

#[cfg(debug_assertions)]
thread_local! {
    static COORD_LOCK_HELD: Cell<bool> = const { Cell::new(false) };
}

/// Set while the IBD coordinator task holds `shared`.
#[doc(hidden)]
pub struct CoordSharedLock;

impl CoordSharedLock {
    #[doc(hidden)]
    pub fn enter() -> Self {
        #[cfg(debug_assertions)]
        COORD_LOCK_HELD.with(|held| held.set(true));
        Self
    }
}

impl Drop for CoordSharedLock {
    fn drop(&mut self) {
        #[cfg(debug_assertions)]
        COORD_LOCK_HELD.with(|held| held.set(false));
    }
}

#[doc(hidden)]
#[inline]
pub fn coord_shared_lock_held() -> bool {
    #[cfg(debug_assertions)]
    {
        COORD_LOCK_HELD.with(|held| held.get())
    }
    #[cfg(not(debug_assertions))]
    {
        false
    }
}
