//! Allocation accounting.
//!
//! Install [`CountingAlloc`] as the global allocator in a benchmark or test
//! binary, then bracket a hot region with [`thread_allocations`] to prove it
//! performs zero heap allocations:
//!
//! ```ignore
//! #[global_allocator]
//! static A: titan_platform::alloc_count::CountingAlloc = titan_platform::alloc_count::CountingAlloc;
//! ```

use std::alloc::{GlobalAlloc, Layout, System};
use std::cell::Cell;
use std::sync::atomic::{AtomicU64, Ordering};

pub struct CountingAlloc;

static GLOBAL_ALLOCS: AtomicU64 = AtomicU64::new(0);
static GLOBAL_BYTES: AtomicU64 = AtomicU64::new(0);

thread_local! {
    // const-initialised, no destructor: no allocation on first access.
    static THREAD_ALLOCS: Cell<u64> = const { Cell::new(0) };
}

#[inline(always)]
fn note(bytes: usize) {
    GLOBAL_ALLOCS.fetch_add(1, Ordering::Relaxed);
    GLOBAL_BYTES.fetch_add(bytes as u64, Ordering::Relaxed);
    let _ = THREAD_ALLOCS.try_with(|c| c.set(c.get() + 1));
}

unsafe impl GlobalAlloc for CountingAlloc {
    unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
        note(layout.size());
        System.alloc(layout)
    }

    unsafe fn alloc_zeroed(&self, layout: Layout) -> *mut u8 {
        note(layout.size());
        System.alloc_zeroed(layout)
    }

    unsafe fn realloc(&self, ptr: *mut u8, layout: Layout, new_size: usize) -> *mut u8 {
        note(new_size);
        System.realloc(ptr, layout, new_size)
    }

    unsafe fn dealloc(&self, ptr: *mut u8, layout: Layout) {
        System.dealloc(ptr, layout)
    }
}

/// Allocations (including reallocations) made by the calling thread so far.
pub fn thread_allocations() -> u64 {
    THREAD_ALLOCS.try_with(|c| c.get()).unwrap_or(0)
}

/// Allocations made by the whole process so far.
pub fn global_allocations() -> u64 {
    GLOBAL_ALLOCS.load(Ordering::Relaxed)
}

/// Bytes requested by the whole process so far.
pub fn global_bytes() -> u64 {
    GLOBAL_BYTES.load(Ordering::Relaxed)
}
