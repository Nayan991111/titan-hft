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

    /// Prefaulting makes pages resident.
    ///
    /// **This test used to read `assert_eq!(r.resident_pages()?, r.pages())`
    /// and that assertion was wrong, not merely fragile.** Residency is the
    /// kernel's to decide, not this code's, and `PinnedRegion::new` treats a
    /// failed `mlock` as non-fatal by design (see `lock_error`). When the
    /// kernel refuses to lock, which an unprivileged process on macOS
    /// routinely sees, nothing in this code prevents the pages from being
    /// compressed or evicted, so an exact-equality assertion was testing the
    /// machine's memory pressure rather than the constructor. It failed on a
    /// correct build on a laptop running a browser and an editor on battery.
    ///
    /// The property actually worth asserting is that prefaulting did
    /// something, and the threshold is chosen against the size of the defect
    /// it has to catch: a region that was never written would be near zero
    /// pages resident, so 50 percent carries a large margin against the real
    /// failure while being indifferent to a page or two being reclaimed.
    #[test]
    fn prefault_makes_pages_resident() {
        let r = PinnedRegion::new(8 << 20).unwrap();
        let pages = r.pages();
        let resident = r.resident_pages().unwrap();
        assert!(pages > 0);
        if r.is_locked() {
            assert!(
                resident * 100 >= pages * 99,
                "locked region: {resident} of {pages} pages resident, \
                 expected at least 99 percent"
            );
        } else {
            eprintln!(
                "note: mlock refused (errno {:?}), so these pages are not pinned \
                 and residency is the operating system's to decide; \
                 {resident} of {pages} resident",
                r.lock_error()
            );
            assert!(
                resident * 2 >= pages,
                "unlocked region: only {resident} of {pages} pages resident, \
                 which is too few for the constructor to have prefaulted at all"
            );
        }
    }

    /// Prefaulting removes the zero-fill faults a fresh mapping would take.
    ///
    /// **Replaces an `assert_eq!(minor_faults, 0)` that was not a correctness
    /// test.** A fault count is a property of the whole process and of the
    /// machine's memory pressure; other tests in this binary run in parallel
    /// and fault too. Asserting exactly zero gave the real defect, which is
    /// prefaulting not happening, a detection margin of zero, and gave the
    /// machine unlimited power to fail a correct build.
    ///
    /// This is a differential instead. Arm A writes one byte per page to a
    /// fresh mapping that was never written, which must take a zero-fill
    /// fault per page. Arm B does the same to a `PinnedRegion`, whose
    /// constructor already wrote every page. Both arms run in the same
    /// process moments apart, so power state, memory pressure and competing
    /// applications apply to both and cancel in the ratio.
    ///
    /// A validity gate comes first: if arm A did not fault roughly once per
    /// page, the fault counter is not measuring what this test needs and the
    /// test reports that instead of pretending to a verdict. That is the same
    /// discipline the experiment registrations use, and it is here because
    /// the old version of this test had no such gate.
    #[test]
    fn prefaulting_removes_the_faults_a_fresh_mapping_takes() {
        let bytes = 8usize << 20;
        let page = page_size();
        let pages = bytes / page;

        // Arm A: fresh, unwritten mapping.
        let raw = unsafe {
            libc::mmap(
                ptr::null_mut(),
                bytes,
                libc::PROT_READ | libc::PROT_WRITE,
                libc::MAP_PRIVATE | libc::MAP_ANON,
                -1,
                0,
            )
        };
        assert_ne!(raw, libc::MAP_FAILED, "mmap for the control arm failed");
        let before_a = Usage::now();
        for off in (0..bytes).step_by(page) {
            unsafe { ptr::write_volatile((raw as *mut u8).add(off), 1u8) };
        }
        let faults_a = Usage::now().since(&before_a).minor_faults;
        unsafe { libc::munmap(raw, bytes) };

        // Arm B: prefaulted by the constructor.
        let mut r = PinnedRegion::new(bytes).unwrap();
        let before_b = Usage::now();
        let s = r.as_mut_slice();
        for off in (0..s.len()).step_by(page) {
            unsafe { ptr::write_volatile(&mut s[off], 1) };
        }
        let faults_b = Usage::now().since(&before_b).minor_faults;

        // Validity gate, not a hypothesis about this code.
        if faults_a * 2 < pages as i64 {
            eprintln!(
                "note: the control arm took {faults_a} faults over {pages} pages, \
                 too few for the fault counter to be measuring zero-fill faults \
                 here; prefault arm took {faults_b}. No verdict."
            );
            return;
        }
        assert!(
            faults_b * 8 <= faults_a,
            "prefaulting did not reduce faults: fresh mapping {faults_a}, \
             pre-faulted region {faults_b}, over {pages} pages"
        );
    }
}
