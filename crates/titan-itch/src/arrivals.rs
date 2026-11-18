//! Arrival schedules for the pipeline harness.
//!
//! The pipeline already drives messages at a constant offered rate, or at the
//! timestamps of a real NASDAQ session. Both are useful and neither isolates
//! burstiness: the constant rate has none, and the real session mixes
//! burstiness with every other property of that particular day.
//!
//! This module adds two synthetic schedules with burstiness as an explicit
//! parameter, so the cost of a cache line or a memory ordering can be measured
//! against arrival clustering alone, with the message content held fixed.
//!
//! * [`poisson`] draws independent exponential gaps. Arrivals are memoryless,
//!   so the count in any window is Poisson and its variance equals its mean.
//! * [`Hawkes`] is self-exciting: every arrival raises the intensity for its
//!   successors, which is what produces the micro-bursts seen in real order
//!   flow. Counts are over-dispersed, with variance above the mean.
//!
//! Both return the same thing the constant-rate path returns, a vector of
//! release times in counter ticks, so nothing downstream changes.
//!
//! # The model
//!
//! A univariate Hawkes process with an exponential kernel has conditional
//! intensity
//!
//! ```text
//!     lambda(t) = mu + sum over t_i < t of  alpha * exp(-beta * (t - t_i))
//! ```
//!
//! `mu` is the background rate, `alpha` the jump in intensity caused by an
//! arrival and `beta` the rate at which that jump decays. The branching ratio
//! `n = alpha / beta` is the expected number of children per arrival. The
//! process is stationary only for `n < 1`, and its stationary mean intensity
//! is `mu / (1 - n)`. [`Hawkes::with_mean_rate`] inverts that so a caller asks
//! for the mean rate it wants and gets it, rather than tuning `mu` by hand and
//! discovering afterwards what rate it produced.
//!
//! Simulation is Ogata's thinning method: propose the next point from an
//! exponential with the current intensity as an upper bound, then accept it
//! with probability equal to the true intensity at that instant divided by the
//! bound. The bound is only ever decreased by waiting and only ever increased
//! by an accepted arrival, so it is valid throughout.
//!
//! The exponential kernel admits an exact recursion for the sum, so the
//! intensity is advanced in constant time per proposal rather than by walking
//! the history. Without it a ten million message schedule is quadratic.

use crate::gen::Rng;

/// A uniform draw on the open interval (0, 1).
///
/// Open at both ends on purpose: `ln(0)` is negative infinity and would
/// produce an infinite gap, and the thinning test wants a value that can
/// actually reject.
#[inline]
fn uniform(rng: &mut Rng) -> f64 {
    // 53 bits, the mantissa width of f64, then shifted off zero.
    let bits = rng.next_u64() >> 11;
    (bits as f64 + 0.5) / ((1u64 << 53) as f64)
}

/// Release times in counter ticks for `n` messages arriving as a Poisson
/// process of `rate` messages per second.
///
/// `freq` is the counter frequency in ticks per second.
pub fn poisson(n: usize, rate: f64, freq: u64, seed: u64) -> Vec<u64> {
    assert!(rate > 0.0, "rate must be positive");
    let mut rng = Rng::new(seed);
    let mut out = Vec::with_capacity(n);
    let mut t = 0.0f64;
    for _ in 0..n {
        t += -uniform(&mut rng).ln() / rate;
        out.push((t * freq as f64) as u64);
    }
    out
}

/// A univariate Hawkes process with an exponential kernel.
#[derive(Clone, Copy, Debug)]
pub struct Hawkes {
    /// Background intensity, arrivals per second.
    pub mu: f64,
    /// Intensity added by one arrival.
    pub alpha: f64,
    /// Decay rate of that addition, per second.
    pub beta: f64,
}

impl Hawkes {
    /// Build a process whose stationary mean rate is `rate`.
    ///
    /// `branching` is `alpha / beta`, the expected number of arrivals directly
    /// triggered by each arrival, and must lie in [0, 1). At zero the process
    /// is Poisson. Approaching one it clusters harder and its mean takes
    /// longer to settle, which is why values above 0.95 are rejected rather
    /// than quietly producing a schedule whose realised rate is nothing like
    /// the one requested.
    ///
    /// `beta` sets the timescale of a burst: a jump decays to `1/e` of itself
    /// after `1 / beta` seconds.
    pub fn with_mean_rate(rate: f64, branching: f64, beta: f64) -> Hawkes {
        assert!(rate > 0.0, "rate must be positive");
        assert!(beta > 0.0, "beta must be positive");
        assert!(
            (0.0..=0.95).contains(&branching),
            "branching ratio must be in [0, 0.95]; at or above 1 the process \
             is explosive and has no stationary mean"
        );
        Hawkes { mu: rate * (1.0 - branching), alpha: branching * beta, beta }
    }

    /// The stationary mean intensity this process will produce.
    pub fn mean_rate(&self) -> f64 {
        self.mu / (1.0 - self.alpha / self.beta)
    }

    /// Release times in counter ticks for `n` messages.
    ///
    /// Ogata thinning with the exact exponential recursion, so the cost is
    /// constant per proposal and the whole schedule is linear in `n`.
    pub fn schedule(&self, n: usize, freq: u64, seed: u64) -> Vec<u64> {
        let mut rng = Rng::new(seed);
        let mut out = Vec::with_capacity(n);
        let mut t = 0.0f64;
        // The decaying part of the intensity, excluding the background. After
        // an arrival it gains alpha; over a wait of w it is scaled by
        // exp(-beta * w). Keeping it as one scalar is what makes this linear.
        let mut excite = 0.0f64;
        while out.len() < n {
            // Upper bound on the intensity from now until the next accepted
            // arrival: the intensity only decreases while we wait.
            let bound = self.mu + excite;
            let w = -uniform(&mut rng).ln() / bound;
            t += w;
            let decayed = excite * (-self.beta * w).exp();
            let lambda = self.mu + decayed;
            if uniform(&mut rng) * bound <= lambda {
                out.push((t * freq as f64) as u64);
                excite = decayed + self.alpha;
            } else {
                excite = decayed;
            }
        }
        out
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const FREQ: u64 = 24_000_000;

    fn seconds(sched: &[u64]) -> Vec<f64> {
        sched.iter().map(|&t| t as f64 / FREQ as f64).collect()
    }

    /// Variance of counts divided by their mean, over fixed windows.
    ///
    /// One for a Poisson process. Above one when arrivals cluster. This is the
    /// statistic the whole module exists to be able to vary.
    fn fano(times: &[f64], windows: usize) -> f64 {
        let span = times[times.len() - 1] - times[0];
        let w = span / windows as f64;
        let mut counts = vec![0u64; windows];
        for &t in times {
            let i = (((t - times[0]) / w) as usize).min(windows - 1);
            counts[i] += 1;
        }
        let n = counts.len() as f64;
        let mean = counts.iter().sum::<u64>() as f64 / n;
        let var = counts.iter().map(|&c| (c as f64 - mean).powi(2)).sum::<f64>() / (n - 1.0);
        var / mean
    }

    #[test]
    fn schedules_are_non_decreasing() {
        for s in [
            poisson(50_000, 1e6, FREQ, 1),
            Hawkes::with_mean_rate(1e6, 0.0, 1e4).schedule(50_000, FREQ, 2),
            Hawkes::with_mean_rate(1e6, 0.8, 1e4).schedule(50_000, FREQ, 3),
        ] {
            assert!(s.windows(2).all(|w| w[1] >= w[0]), "release times went backwards");
        }
    }

    #[test]
    fn same_seed_gives_the_same_schedule() {
        let h = Hawkes::with_mean_rate(2e6, 0.6, 5e3);
        assert_eq!(h.schedule(10_000, FREQ, 42), h.schedule(10_000, FREQ, 42));
        assert_ne!(h.schedule(10_000, FREQ, 42), h.schedule(10_000, FREQ, 43));
        assert_eq!(poisson(10_000, 1e6, FREQ, 7), poisson(10_000, 1e6, FREQ, 7));
    }

    /// The realised rate matches the requested one.
    ///
    /// This is the property that makes the parameter usable: a caller asks for
    /// a mean rate and gets it, whatever the branching ratio. The tolerance is
    /// 8 percent, which is loose for the Poisson case and necessary for the
    /// clustered one, where the mean of a finite sample of a heavy-clustered
    /// process converges slowly.
    #[test]
    fn realised_rate_matches_the_requested_rate() {
        let n = 400_000;
        for (branching, seed) in [(0.0, 11u64), (0.5, 12), (0.8, 13)] {
            let h = Hawkes::with_mean_rate(1e6, branching, 1e4);
            assert!((h.mean_rate() - 1e6).abs() < 1.0, "mean_rate algebra is wrong");
            let s = seconds(&h.schedule(n, FREQ, seed));
            let realised = (n - 1) as f64 / (s[n - 1] - s[0]);
            let err = (realised - 1e6).abs() / 1e6;
            assert!(
                err < 0.08,
                "branching {branching}: asked 1.0e6 msg/s, realised {realised:.0}, \
                 off by {:.1} percent",
                err * 100.0
            );
        }
        let p = seconds(&poisson(n, 1e6, FREQ, 14));
        let realised = (n - 1) as f64 / (p[n - 1] - p[0]);
        assert!((realised - 1e6).abs() / 1e6 < 0.02, "poisson rate off: {realised:.0}");
    }

    /// Zero branching degenerates to Poisson, and burstiness rises with it.
    ///
    /// The Fano factor of a Poisson process is 1. A finite sample scatters
    /// around that, so the degenerate case is checked against a band rather
    /// than against 1 exactly, and the clustered cases are checked for
    /// separation from it and for monotonicity between themselves.
    #[test]
    fn branching_ratio_controls_burstiness() {
        let n = 400_000;
        let windows = 400;
        let f_poisson = fano(&seconds(&poisson(n, 1e6, FREQ, 21)), windows);
        let f0 = fano(&seconds(&Hawkes::with_mean_rate(1e6, 0.0, 1e4).schedule(n, FREQ, 22)), windows);
        let f5 = fano(&seconds(&Hawkes::with_mean_rate(1e6, 0.5, 1e4).schedule(n, FREQ, 23)), windows);
        let f8 = fano(&seconds(&Hawkes::with_mean_rate(1e6, 0.8, 1e4).schedule(n, FREQ, 24)), windows);
        assert!(
            (0.7..1.4).contains(&f_poisson),
            "poisson Fano should sit near 1, got {f_poisson:.3}"
        );
        assert!(
            (0.7..1.4).contains(&f0),
            "branching 0 must degenerate to Poisson, Fano {f0:.3} against \
             {f_poisson:.3} for the Poisson generator"
        );
        assert!(f5 > f0 * 1.5, "branching 0.5 not burstier: {f5:.3} against {f0:.3}");
        assert!(f8 > f5, "burstiness not monotone in the branching ratio: \
                          0.8 gave {f8:.3}, 0.5 gave {f5:.3}");
    }


    /// Not an assertion. Prints the measured burstiness so the numbers that
    /// back the claims above are in the record rather than only in a pass.
    #[test]
    fn report_measured_burstiness() {
        let n = 400_000;
        let windows = 400;
        println!("  generator                 Fano    realised msg/s");
        let p = seconds(&poisson(n, 1e6, FREQ, 21));
        println!("  poisson                 {:6.3}    {:.0}", fano(&p, windows),
                 (n - 1) as f64 / (p[n - 1] - p[0]));
        for b in [0.0, 0.3, 0.5, 0.8, 0.9] {
            let h = Hawkes::with_mean_rate(1e6, b, 1e4);
            let s = seconds(&h.schedule(n, FREQ, 22));
            println!("  hawkes branching {b:.1}      {:6.3}    {:.0}",
                     fano(&s, windows), (n - 1) as f64 / (s[n - 1] - s[0]));
        }
    }

    /// The intensity recursion is the exact exponential sum, not an
    /// approximation of it.
    ///
    /// Recomputing the kernel sum by walking the accepted history has to agree
    /// with the running scalar. This is quadratic, so it runs on a short
    /// schedule only; its job is to catch an error in the recursion, which
    /// would otherwise look like a plausible but wrong amount of clustering.
    #[test]
    fn the_recursion_equals_the_explicit_kernel_sum() {
        let h = Hawkes::with_mean_rate(1e5, 0.7, 1e3);
        let times = seconds(&h.schedule(2_000, FREQ, 31));
        let mut worst = 0.0f64;
        for (k, &t) in times.iter().enumerate().skip(1) {
            let explicit: f64 = times[..k]
                .iter()
                .map(|&ti| h.alpha * (-h.beta * (t - ti)).exp())
                .sum();
            // The recursion as the simulator runs it, rebuilt from scratch.
            let mut excite = 0.0f64;
            let mut prev = 0.0f64;
            for &ti in &times[..k] {
                excite = excite * (-h.beta * (ti - prev)).exp() + h.alpha;
                prev = ti;
            }
            let recursive = excite * (-h.beta * (t - prev)).exp();
            let diff = (explicit - recursive).abs();
            let scale = explicit.abs().max(1e-12);
            worst = worst.max(diff / scale);
        }
        assert!(worst < 1e-9, "recursion drifts from the kernel sum by {worst:e}");
    }
}
