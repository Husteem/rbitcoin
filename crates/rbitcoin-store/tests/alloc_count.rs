//! Bytes the calling thread asks the allocator for, so a test can pin how
//! much heap a store path cycles. A realloc counts its new size: a buffer
//! regrown by doubling counts every size it passed through.

use std::alloc::{GlobalAlloc, Layout, System};
use std::cell::Cell;

pub struct Counting;

thread_local! {
    static ASKED: Cell<u64> = const { Cell::new(0) };
}

fn count(bytes: usize) {
    // A thread being torn down has no slot left; its asks are not a test's.
    let _ = ASKED.try_with(|a| a.set(a.get().saturating_add(bytes as u64)));
}

// SAFETY: every call forwards to `System` with the caller's layout and
// pointer unchanged; the counter is a const-initialized thread-local
// `Cell<u64>`, which needs no allocation and has no destructor.
unsafe impl GlobalAlloc for Counting {
    unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
        count(layout.size());
        // SAFETY: the caller upholds `GlobalAlloc::alloc`'s contract.
        unsafe { System.alloc(layout) }
    }

    unsafe fn alloc_zeroed(&self, layout: Layout) -> *mut u8 {
        count(layout.size());
        // SAFETY: the caller upholds `GlobalAlloc::alloc_zeroed`'s contract.
        unsafe { System.alloc_zeroed(layout) }
    }

    unsafe fn realloc(&self, ptr: *mut u8, layout: Layout, new_size: usize) -> *mut u8 {
        count(new_size);
        // SAFETY: the caller upholds `GlobalAlloc::realloc`'s contract.
        unsafe { System.realloc(ptr, layout, new_size) }
    }

    unsafe fn dealloc(&self, ptr: *mut u8, layout: Layout) {
        // SAFETY: the caller upholds `GlobalAlloc::dealloc`'s contract.
        unsafe { System.dealloc(ptr, layout) }
    }
}

/// Run `f` and return what it returned with the bytes this thread asked
/// the allocator for meanwhile. Other threads' asks are not counted.
pub fn asked_by<R>(f: impl FnOnce() -> R) -> (R, u64) {
    let before = ASKED.with(Cell::get);
    let out = f();
    (out, ASKED.with(Cell::get) - before)
}
