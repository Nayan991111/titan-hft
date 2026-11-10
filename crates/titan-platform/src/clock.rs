//! Generic timer access.
//!
//! On arm64 the virtual counter `CNTVCT_EL0` is readable from user space. On
//! Apple silicon it runs at 24 MHz (41.67 ns per tick), so it is used for
//! wall-clock spans and frequency calibration, never for single events shorter
//! than a tick. Cycle-accurate counts come from [`crate::kperf`].

#[cfg(target_arch = "aarch64")]
use core::arch::asm;

/// Read the virtual counter. The leading `isb` keeps earlier instructions from
/// being reordered past the read.
#[cfg(target_arch = "aarch64")]
#[inline(always)]
pub fn counter() -> u64 {
    let v: u64;
    unsafe {
        asm!("isb", "mrs {v}, cntvct_el0", v = out(reg) v, options(nostack, preserves_flags));
    }
    v
}

/// Counter frequency in Hz as reported by `CNTFRQ_EL0`.
#[cfg(target_arch = "aarch64")]
#[inline(always)]
pub fn counter_frequency() -> u64 {
    let v: u64;
    unsafe {
        asm!("mrs {v}, cntfrq_el0", v = out(reg) v, options(nostack, nomem, preserves_flags));
    }
    v
}

#[cfg(not(target_arch = "aarch64"))]
fn origin() -> std::time::Instant {
    use std::sync::OnceLock;
    static ORIGIN: OnceLock<std::time::Instant> = OnceLock::new();
    *ORIGIN.get_or_init(std::time::Instant::now)
}

/// Portable fallback: nanoseconds since first use.
#[cfg(not(target_arch = "aarch64"))]
#[inline(always)]
pub fn counter() -> u64 {
    origin().elapsed().as_nanos() as u64
}

#[cfg(not(target_arch = "aarch64"))]
#[inline(always)]
pub fn counter_frequency() -> u64 {
    1_000_000_000
}

/// Convert a counter delta to nanoseconds.
#[inline]
pub fn ticks_to_ns(ticks: u64, freq_hz: u64) -> f64 {
    ticks as f64 * 1e9 / freq_hz as f64
}

/// `mach_timebase_info` numerator and denominator (ns = ticks * numer / denom).
#[cfg(target_os = "macos")]
pub fn mach_timebase() -> (u32, u32) {
    #[repr(C)]
    struct Timebase {
        numer: u32,
        denom: u32,
    }
    extern "C" {
        fn mach_timebase_info(info: *mut Timebase) -> i32;
    }
    let mut tb = Timebase { numer: 0, denom: 0 };
    unsafe { mach_timebase_info(&mut tb) };
    (tb.numer, tb.denom)
}

#[cfg(not(target_os = "macos"))]
pub fn mach_timebase() -> (u32, u32) {
    (1, 1)
}

/// Distribution of back-to-back counter deltas over `pairs` samples.
/// Index k holds the count of deltas equal to k ticks; the last slot is "k or more".
pub fn back_to_back_histogram(pairs: usize) -> [u64; 8] {
    let mut h = [0u64; 8];
    for _ in 0..pairs {
        let a = counter();
        let b = counter();
        let d = b.wrapping_sub(a) as usize;
        h[d.min(7)] += 1;
    }
    h
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn counter_is_monotonic() {
        let mut prev = counter();
        for _ in 0..100_000 {
            let now = counter();
            assert!(now >= prev);
            prev = now;
        }
    }

    #[test]
    fn frequency_is_plausible() {
        let f = counter_frequency();
        assert!(f >= 1_000_000, "counter frequency {f} Hz is implausibly low");
    }
}
