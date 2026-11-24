//! What the calling thread asks the allocator for: the largest single ask,
//! and how many asks had one watched size, so a test can pin that a path
//! does not copy a large table (a copy asks for the table's exact layout).
//! A realloc counts as an ask of its new size.

use std::alloc::{GlobalAlloc, Layout, System};
use std::cell::Cell;

pub struct Counting;

thread_local! {
    static LARGEST: Cell<usize> = const { Cell::new(0) };
    static WATCH: Cell<(usize, usize)> = const { Cell::new((0, 0)) };
}

fn note(bytes: usize) {
    // A thread being torn down has no slot left; its asks are not a test's.
    let _ = LARGEST.try_with(|l| l.set(l.get().max(bytes)));
    let _ = WATCH.try_with(|w| {
        let (size, hits) = w.get();
        if size != 0 && bytes == size {
            w.set((size, hits + 1));
        }
    });
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

/// Count this thread's asks of exactly `bytes` from now on.
pub fn watch(bytes: usize) {
    WATCH.with(|w| w.set((bytes, 0)));
}

/// Asks of the watched size since [`watch`] or the last call.
pub fn take_hits() -> usize {
    WATCH.with(|w| {
        let (size, hits) = w.get();
        w.set((size, 0));
        hits
    })
}
