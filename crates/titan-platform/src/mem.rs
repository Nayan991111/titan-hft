//! Page-aligned, pre-faulted, locked memory regions.
//!
//! Hot-path structures (rings, order book arenas) live in a [`PinnedRegion`]:
//! 1. mapped with `mmap`, so the start is aligned to the VM page size
//!    (16 KiB on Apple silicon macOS);
//! 2. every page is written once before use, so no zero-fill fault can occur
//!    inside a measured region;
//! 3. `mlock`ed so the pages cannot be compressed or paged out.
//!
//! Residency is verifiable with [`PinnedRegion::resident_pages`] and fault
//! freedom with [`crate::rusage::Usage`].

use std::io;
use std::ptr::{self, NonNull};

pub fn page_size() -> usize {
    unsafe { libc::sysconf(libc::_SC_PAGESIZE) as usize }
}

pub struct PinnedRegion {
    ptr: NonNull<u8>,
    len: usize,
    page: usize,
    lock_error: Option<i32>,
}

unsafe impl Send for PinnedRegion {}
unsafe impl Sync for PinnedRegion {}

impl PinnedRegion {
    /// Map at least `min_len` bytes, rounded up to whole pages, pre-fault and
    /// try to lock them. A failed `mlock` is recorded, not fatal; check
    /// [`PinnedRegion::is_locked`] before treating the region as pinned.
    pub fn new(min_len: usize) -> io::Result<PinnedRegion> {
        let page = page_size();
        let len = min_len.max(1).div_ceil(page) * page;
        let raw = unsafe {
            libc::mmap(
                ptr::null_mut(),
                len,
                libc::PROT_READ | libc::PROT_WRITE,
                libc::MAP_PRIVATE | libc::MAP_ANON,
                -1,
                0,
            )
        };
        if raw == libc::MAP_FAILED {
            return Err(io::Error::last_os_error());
        }
        let ptr = NonNull::new(raw as *mut u8).expect("mmap returned null");
        for off in (0..len).step_by(page) {
            unsafe { ptr::write_volatile(ptr.as_ptr().add(off), 0u8) };
        }
        let lock_error = if unsafe { libc::mlock(raw, len) } == 0 {
            None
        } else {
            Some(io::Error::last_os_error().raw_os_error().unwrap_or(-1))
        };
        Ok(PinnedRegion { ptr, len, page, lock_error })
    }

    pub fn as_ptr(&self) -> *mut u8 {
        self.ptr.as_ptr()
    }

    pub fn len(&self) -> usize {
        self.len
    }

    pub fn is_empty(&self) -> bool {
        self.len == 0
    }

    pub fn page_size(&self) -> usize {
        self.page
    }

    pub fn pages(&self) -> usize {
        self.len / self.page
    }

    pub fn is_locked(&self) -> bool {
        self.lock_error.is_none()
    }

    /// errno from `mlock`, if locking failed.
    pub fn lock_error(&self) -> Option<i32> {
        self.lock_error
    }

    /// Number of pages currently resident, from `mincore`.
    pub fn resident_pages(&self) -> io::Result<usize> {
        let mut vec = vec![0u8; self.pages()];
        let rc = unsafe {
            libc::mincore(self.ptr.as_ptr() as *mut _, self.len, vec.as_mut_ptr() as *mut _)
        };
        if rc != 0 {
            return Err(io::Error::last_os_error());
        }
        Ok(vec.iter().filter(|&&b| b & 1 != 0).count())
    }

    /// The region as a byte slice.
    pub fn as_mut_slice(&mut self) -> &mut [u8] {
        unsafe { std::slice::from_raw_parts_mut(self.ptr.as_ptr(), self.len) }
    }
}

impl Drop for PinnedRegion {
    fn drop(&mut self) {
        unsafe {
            if self.lock_error.is_none() {
                libc::munlock(self.ptr.as_ptr() as *const _, self.len);
            }
            libc::munmap(self.ptr.as_ptr() as *mut _, self.len);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::rusage::Usage;

    #[test]
    fn region_is_page_aligned_and_rounded() {
        let r = PinnedRegion::new(1).unwrap();
        assert_eq!(r.as_ptr() as usize % r.page_size(), 0);
        assert_eq!(r.len(), r.page_size());
    }

    #[test]
    fn all_pages_resident_after_prefault() {
        let r = PinnedRegion::new(8 << 20).unwrap();
        assert_eq!(r.resident_pages().unwrap(), r.pages());
    }

    #[test]
    fn writes_after_prefault_take_no_faults() {
        let mut r = PinnedRegion::new(8 << 20).unwrap();
        let page = r.page_size();
        let before = Usage::now();
        let s = r.as_mut_slice();
        for off in (0..s.len()).step_by(page) {
            unsafe { ptr::write_volatile(&mut s[off], 1) };
        }
        let d = Usage::now().since(&before);
        assert_eq!(d.minor_faults, 0, "page faults inside a pre-faulted region");
    }
}
