//! Index cells. Each side's index shares its cell with that side's private
//! cache of the other index, so the owner touches exactly one line on the
//! fast path. The alignment decides whether the two cells can share a line.
//!
//! Apple silicon reports `hw.cachelinesize = 128`; with [`Pad64`] the
//! producer and consumer cells sit 64 bytes apart, inside one 128-byte line.

use std::cell::UnsafeCell;
use std::sync::atomic::AtomicUsize;

pub trait IndexCell: Default + Send + Sync + 'static {
    const ALIGN: usize;
    fn index(&self) -> &AtomicUsize;
    /// Owner-private cache of the other side's index.
    fn cache(&self) -> *mut usize;
}

macro_rules! cell {
    ($name:ident, $align:literal) => {
        #[repr(C, align($align))]
        #[derive(Default)]
        pub struct $name {
            index: AtomicUsize,
            cache: UnsafeCell<usize>,
        }

        // The cache is only ever touched by the owning side.
        unsafe impl Sync for $name {}

        impl IndexCell for $name {
            const ALIGN: usize = $align;
            #[inline(always)]
            fn index(&self) -> &AtomicUsize {
                &self.index
            }
            #[inline(always)]
            fn cache(&self) -> *mut usize {
                self.cache.get()
            }
        }
    };
}

cell!(Pad64, 64);
cell!(Pad128, 128);

#[cfg(test)]
mod tests {
    use super::*;
    use std::mem::{align_of, size_of};

    #[test]
    fn cell_layout() {
        assert_eq!(align_of::<Pad64>(), 64);
        assert_eq!(size_of::<Pad64>(), 64);
        assert_eq!(align_of::<Pad128>(), 128);
        assert_eq!(size_of::<Pad128>(), 128);
    }
}
