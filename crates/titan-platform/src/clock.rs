//! Generic timer access.
//!
//! On arm64 the virtual counter `CNTVCT_EL0` is readable from user space.
//! Its unit is given by `CNTFRQ_EL0`, but its resolution can be much coarser:
//! from Armv8.6 the reported frequency is fixed at 1 GHz while an
//! implementation may advance the counter in steps larger than one. Always
//! measure the step with [`resolution`] instead of assuming 1 / frequency.
//! The counter is used for wall-clock spans and frequency calibration, never
//! for single events shorter than one step. Cycle counts come from
//! [`crate::kperf`].

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

/// Observed counter resolution from back-to-back reads.
#[derive(Clone, Copy, Debug)]
pub struct Resolution {
    /// Smallest non-zero delta seen, in counter units.
    pub min_step: u64,
    /// Most frequent non-zero delta, in counter units.
    pub modal_step: u64,
    /// Mean delta over all pairs, in counter units (an estimate of read cost).
    pub mean_delta: f64,
    /// Fraction of pairs with a zero delta.
    pub zero_fraction: f64,
    /// Pairs whose delta exceeded 8 times the modal step (interruptions).
    pub outliers: u64,
    pub pairs: u64,
}

/// Measure the counter's effective resolution with `pairs` back-to-back reads.
pub fn resolution(pairs: usize) -> Resolution {
    let mut deltas: Vec<u64> = Vec::with_capacity(pairs);
    for _ in 0..pairs {
        let a = counter();
        let b = counter();
        deltas.push(b.wrapping_sub(a));
    }
    let zeros = deltas.iter().filter(|&&d| d == 0).count();
    let mut nonzero: Vec<u64> = deltas.iter().copied().filter(|&d| d != 0).collect();
    nonzero.sort_unstable();
    let min_step = nonzero.first().copied().unwrap_or(0);
    let mut modal_step = 0;
    let mut best = 0usize;
    let mut i = 0;
    while i < nonzero.len() {
        let mut j = i;
        while j < nonzero.len() && nonzero[j] == nonzero[i] {
            j += 1;
        }
        if j - i > best {
            best = j - i;
            modal_step = nonzero[i];
        }
        i = j;
    }
    let sum: u128 = deltas.iter().map(|&d| d as u128).sum();
    let outliers = if modal_step == 0 { 0 } else { deltas.iter().filter(|&&d| d > 8 * modal_step).count() as u64 };
    Resolution {
        min_step,
        modal_step,
        mean_delta: sum as f64 / pairs.max(1) as f64,
        zero_fraction: zeros as f64 / pairs.max(1) as f64,
        outliers,
        pairs: pairs as u64,
    }
}

/// Counts of each distinct non-zero back-to-back delta, most frequent first.
pub fn nonzero_delta_counts(pairs: usize) -> Vec<(u64, u64)> {
    let mut deltas: Vec<u64> = Vec::with_capacity(pairs);
    for _ in 0..pairs {
        let a = counter();
        let b = counter();
        let d = b.wrapping_sub(a);
        if d != 0 {
            deltas.push(d);
        }
    }
    deltas.sort_unstable();
    let mut out: Vec<(u64, u64)> = Vec::new();
    for d in deltas {
        match out.last_mut() {
            Some((v, c)) if *v == d => *c += 1,
            _ => out.push((d, 1)),
        }
    }
    out.sort_by(|a, b| b.1.cmp(&a.1).then(a.0.cmp(&b.0)));
    out
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
    fn resolution_is_consistent() {
        let r = resolution(100_000);
        assert_eq!(r.pairs, 100_000);
        assert!(r.min_step <= r.modal_step || r.modal_step == 0);
        assert!((0.0..=1.0).contains(&r.zero_fraction));
    }

    #[test]
    fn frequency_is_plausible() {
        let f = counter_frequency();
        assert!(f >= 1_000_000, "counter frequency {f} Hz is implausibly low");
    }
}
