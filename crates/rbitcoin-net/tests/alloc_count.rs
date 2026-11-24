//! The largest single allocation the calling thread asked for, so a test
//! can pin that a path does not copy a large table. A realloc counts as an
//! ask of its new size.

use std::alloc::{GlobalAlloc, Layout, System};
use std::cell::Cell;

pub struct Counting;

thread_local! {
    static LARGEST: Cell<usize> = const { Cell::new(0) };
}

fn note(bytes: usize) {
    // A thread being torn down has no slot left; its asks are not a test's.
    let _ = LARGEST.try_with(|l| l.set(l.get().max(bytes)));
}

// SAFETY: every call forwards to `System` with the caller's layout and
// pointer unchanged; the counter is a const-initialized thread-local
// `Cell<usize>`, which needs no allocation and has no destructor.
unsafe impl GlobalAlloc for Counting {
    unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
        note(layout.size());
        // SAFETY: the caller upholds `GlobalAlloc::alloc`'s contract.
        unsafe { System.alloc(layout) }
    }

    unsafe fn alloc_zeroed(&self, layout: Layout) -> *mut u8 {
        note(layout.size());
        // SAFETY: the caller upholds `GlobalAlloc::alloc_zeroed`'s contract.
        unsafe { System.alloc_zeroed(layout) }
    }

    unsafe fn realloc(&self, ptr: *mut u8, layout: Layout, new_size: usize) -> *mut u8 {
        note(new_size);
        // SAFETY: the caller upholds `GlobalAlloc::realloc`'s contract.
        unsafe { System.realloc(ptr, layout, new_size) }
    }

    unsafe fn dealloc(&self, ptr: *mut u8, layout: Layout) {
        // SAFETY: the caller upholds `GlobalAlloc::dealloc`'s contract.
        unsafe { System.dealloc(ptr, layout) }
    }
}

/// The largest allocation this thread asked for since the last call.
pub fn take_largest() -> usize {
    LARGEST.with(|l| l.replace(0))
}
