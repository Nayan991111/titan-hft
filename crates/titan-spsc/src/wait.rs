//! What a blocked side executes while waiting for the other side's index to
//! change from a value it has already seen.
//!
//! | strategy | arm64 code per iteration |
//! |---|---|
//! | [`Spin`] | reload only |
//! | [`SpinHint`] | `core::hint::spin_loop()`, which Rust lowers to `isb` |
//! | [`Yield`] | `yield` hint |
//! | [`Wfe`] | arm the exclusive monitor on the index line with `ldxr`, then `wfe` until another core writes that line |

use std::sync::atomic::AtomicUsize;
#[cfg(not(target_arch = "aarch64"))]
use std::sync::atomic::Ordering;

pub trait Wait: Send + Sync + 'static {
    const NAME: &'static str;
    /// Called after observing `watch == seen`. May return spuriously; the
    /// caller always re-checks with a proper acquire load.
    fn wait(watch: &AtomicUsize, seen: usize);
}

pub struct Spin;
pub struct SpinHint;
pub struct Yield;
pub struct Wfe;

impl Wait for Spin {
    const NAME: &'static str = "spin";
    #[inline(always)]
    fn wait(_watch: &AtomicUsize, _seen: usize) {}
}

impl Wait for SpinHint {
    const NAME: &'static str = "isb";
    #[inline(always)]
    fn wait(_watch: &AtomicUsize, _seen: usize) {
        std::hint::spin_loop();
    }
}

impl Wait for Yield {
    const NAME: &'static str = "yield";
    #[inline(always)]
    fn wait(_watch: &AtomicUsize, _seen: usize) {
        #[cfg(target_arch = "aarch64")]
        unsafe {
            core::arch::asm!("yield", options(nomem, nostack, preserves_flags));
        }
        #[cfg(not(target_arch = "aarch64"))]
        std::hint::spin_loop();
    }
}

impl Wait for Wfe {
    const NAME: &'static str = "wfe";
    #[inline(always)]
    fn wait(watch: &AtomicUsize, seen: usize) {
        #[cfg(target_arch = "aarch64")]
        unsafe {
            // Same sequence as the Linux arm64 __cmpwait: clear the local
            // event, arm the monitor with ldxr, and sleep only if the value
            // is still unchanged. A write to the line by another core clears
            // the monitor and generates the wake-up event.
            core::arch::asm!(
                "sevl",
                "wfe",
                "ldxr {t}, [{p}]",
                "eor {t}, {t}, {v}",
                "cbnz {t}, 2f",
                "wfe",
                "2:",
                p = in(reg) watch.as_ptr(),
                v = in(reg) seen,
                t = out(reg) _,
                options(nostack),
            );
        }
        #[cfg(not(target_arch = "aarch64"))]
        {
            let _ = (watch.load(Ordering::Relaxed), seen);
            std::hint::spin_loop();
        }
    }
}
