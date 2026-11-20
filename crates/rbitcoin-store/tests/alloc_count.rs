//! What the calling thread asks the allocator for, so a test can pin how
//! much heap a store path cycles and how many times it asks. A realloc
//! counts as an ask of its new size: a buffer regrown by doubling counts
//! every size it passed through.

use std::alloc::{GlobalAlloc, Layout, System};
use std::cell::Cell;

pub struct Counting;

/// Bytes and calls one thread asked for.
#[derive(Clone, Copy, Debug, Default)]
pub struct Asked {
    pub bytes: u64,
    pub calls: u64,
}

thread_local! {
    static ASKED: Cell<Asked> = const { Cell::new(Asked { bytes: 0, calls: 0 }) };
}

fn count(bytes: usize) {
    // A thread being torn down has no slot left; its asks are not a test's.
    let _ = ASKED.try_with(|a| {
        let mut v = a.get();
        v.bytes = v.bytes.saturating_add(bytes as u64);
        v.calls += 1;
        a.set(v);
    });
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

/// Run `f` and return what it returned with what this thread asked the
/// allocator for meanwhile. Other threads' asks are not counted.
pub fn asked_by<R>(f: impl FnOnce() -> R) -> (R, Asked) {
    let before = ASKED.with(Cell::get);
    let out = f();
    let after = ASKED.with(Cell::get);
    let asked = Asked {
        bytes: after.bytes - before.bytes,
        calls: after.calls - before.calls,
    };
    (out, asked)
}
