//! Validation of the path engine, in a chain where every link is checked
//! against something that does not share its machinery.
//!
//! 1. The generator's uniforms against a chi-square over equal bins, and
//!    their first two moments against the exact values for a uniform.
//! 2. The normal variates against their first four moments and against a
//!    Kolmogorov-Smirnov statistic computed with the cumulative normal from
//!    `pricing`, which is itself validated against Simpson integration of
//!    the density. No tabulated constant enters that chain.
//! 3. The path generator against the exact moments of geometric Brownian
//!    motion, which are closed forms: `E[S_T] = S0 exp((r-q)T)` and
//!    `Var[ln S_T] = sigma^2 T`.
//! 4. A simulated European call against the Black-Scholes price, which the
//!    `pricing` tests have already validated by central difference and by
//!    put-call parity, to within the simulation's own standard error.
//! 5. A simulated geometric Asian against its exact discrete closed form,
//!    which validates the averaging accumulator rather than the stepper.
//! 6. The barrier monitor against in-out parity, an identity that holds path
//!    by path and needs no formula at all, and against the degenerate case
//!    of a barrier that cannot be reached.
//! 7. Every pair of arms that must agree, checked on identical variates: log
//!    space against price space, branchless against branchy, rolling state
//!    against a stored path, early termination against running to maturity.

use titan_core::paths::{
    asian_arithmetic, asian_geometric, barrier_log_space, barrier_price_space,
    geometric_asian_call, lookback_branchless, lookback_branchy, rolling, stored, Accum, Gbm, Rng,
    Side,
};
use titan_core::pricing::{normal_cdf, price, Contract, Kind};

/// How many standard errors a Monte Carlo estimate may sit from a closed
/// form before the test fails.
///
/// Four is deliberate. A correct estimator exceeds four standard errors with
/// probability about 6 in 100,000, so with the handful of comparisons below
/// a spurious failure is unlikely; and a wrong formula or a biased stepper
/// is wrong by a multiple of the price, not by a few standard errors.
const SIGMAS: f64 = 4.0;

#[test]
fn the_generator_is_uniform() {
    const BINS: usize = 64;
    const PER_BIN: usize = 10_000;
    let n = BINS * PER_BIN;
    let mut rng = Rng::new(0x51A9_E333);
    let mut counts = [0usize; BINS];
    let mut sum = 0.0_f64;
    let mut sum_sq = 0.0_f64;
    let mut lowest = f64::MAX;
    let mut highest = f64::MIN;
    for _ in 0..n {
        let u = rng.next_f64();
        assert!((0.0..1.0).contains(&u), "uniform out of range: {u}");
        lowest = lowest.min(u);
        highest = highest.max(u);
        counts[(u * BINS as f64) as usize] += 1;
        sum += u;
        sum_sq += u * u;
    }
    let expect = PER_BIN as f64;
    let chi2: f64 = counts
        .iter()
        .map(|&c| {
            let d = c as f64 - expect;
            d * d / expect
        })
        .sum();
    let mean = sum / n as f64;
    let var = sum_sq / n as f64 - mean * mean;
    println!(
        "uniform: chi-square {chi2:.2} on {} degrees of freedom, mean {mean:.6} \
         (exact 0.5), variance {var:.6} (exact {:.6}), range [{lowest:.2e}, {highest:.8}]",
        BINS - 1,
        1.0 / 12.0
    );
    // 63 degrees of freedom: the 99.9th percentile is near 103.4, so 150 is
    // about seven standard deviations above the mean of 63 and will not fire
    // by chance, while a generator with a biased bit would far exceed it.
    assert!(chi2 < 150.0, "chi-square {chi2:.2} on {} bins", BINS);
    // Standard error of the mean of n uniforms is sqrt(1/12/n).
    let se = (1.0 / 12.0 / n as f64).sqrt();
    assert!(
        (mean - 0.5).abs() < SIGMAS * se,
        "mean {mean:.8} is {:.1} standard errors from 0.5",
        (mean - 0.5).abs() / se
    );
    assert!((var - 1.0 / 12.0).abs() < 1e-3, "variance {var:.8}");
}

#[test]
fn the_normal_variates_match_the_normal_distribution() {
    const N: usize = 400_000;
    let mut rng = Rng::new(0xFCC0_539C);
    let mut v = Vec::with_capacity(N);
    while v.len() < N {
        let (a, b) = rng.next_normal_pair();
        v.push(a);
        v.push(b);
    }
    v.truncate(N);
    let n = N as f64;
    let mean: f64 = v.iter().sum::<f64>() / n;
    let m2: f64 = v.iter().map(|x| (x - mean).powi(2)).sum::<f64>() / n;
    let m3: f64 = v.iter().map(|x| (x - mean).powi(3)).sum::<f64>() / n;
    let m4: f64 = v.iter().map(|x| (x - mean).powi(4)).sum::<f64>() / n;
    let sd = m2.sqrt();
    let skew = m3 / sd.powi(3);
    let kurt = m4 / m2.powi(2) - 3.0;

    // Kolmogorov-Smirnov against the cumulative normal, which is validated
    // elsewhere against Simpson integration of the density.
    let mut sorted = v.clone();
    sorted.sort_by(|a, b| a.partial_cmp(b).unwrap());
    let mut ks = 0.0_f64;
    for (i, &x) in sorted.iter().enumerate() {
        let f = normal_cdf(x);
        let below = i as f64 / n;
        let above = (i + 1) as f64 / n;
        ks = ks.max((f - below).abs()).max((f - above).abs());
    }

    // Asymptotic standard errors: 1/sqrt(n) for the mean, sqrt(6/n) for the
    // skewness and sqrt(24/n) for the excess kurtosis.
    let (se_m, se_s, se_k) = (1.0 / n.sqrt(), (6.0 / n).sqrt(), (24.0 / n).sqrt());
    // The 5 percent critical value of the KS statistic is 1.36/sqrt(n).
    let ks_crit = 1.36 / n.sqrt();
    println!(
        "normal: mean {mean:+.6} ({:.2} se), sd {sd:.6}, skew {skew:+.5} ({:.2} se), \
         excess kurtosis {kurt:+.5} ({:.2} se), KS {ks:.6} against a 5 percent critical value of {ks_crit:.6}",
        mean.abs() / se_m,
        skew.abs() / se_s,
        kurt.abs() / se_k
    );
    assert!(mean.abs() < SIGMAS * se_m, "mean {mean:+.8}");
    assert!((sd - 1.0).abs() < SIGMAS / (2.0 * n).sqrt(), "sd {sd:.8}");
    assert!(skew.abs() < SIGMAS * se_s, "skewness {skew:+.6}");
    assert!(kurt.abs() < SIGMAS * se_k, "excess kurtosis {kurt:+.6}");
    assert!(ks < ks_crit, "KS {ks:.6} exceeds {ks_crit:.6}");
}

#[test]
fn the_path_generator_has_the_exact_moments_of_the_process() {
    // Both moments are closed forms, so this checks the stepper itself and
    // not any pricing formula built on it.
    const N: usize = 400_000;
    for &(t, sigma, steps) in &[(1.0, 0.2, 1usize), (1.0, 0.2, 250), (0.5, 0.45, 100)] {
        let (s0, r, q) = (100.0, 0.05, 0.02);
        let g = Gbm::new(s0, t, r, q, sigma, steps);
        let mut rng = Rng::new(0xC0FF_EE00 ^ steps as u64);
        let mut lvl = Accum::default();
        let mut logs = Vec::with_capacity(N);
        for _ in 0..N {
            let s = titan_core::paths::terminal(&g, &mut rng);
            lvl.push(s);
            logs.push(s.ln());
        }
        let est = lvl.finish(1.0);
        let want_mean = s0 * ((r - q) * t).exp();
        let n = N as f64;
        let lm: f64 = logs.iter().sum::<f64>() / n;
        let lv: f64 = logs.iter().map(|x| (x - lm).powi(2)).sum::<f64>() / (n - 1.0);
        let want_lv = sigma * sigma * t;
        let want_lm = s0.ln() + (r - q - 0.5 * sigma * sigma) * t;
        // Standard error of a variance is var*sqrt(2/n) asymptotically.
        let se_lv = want_lv * (2.0 / n).sqrt();
        let se_lm = (want_lv / n).sqrt();
        println!(
            "steps {steps:3}: E[S_T] {:.4} against exact {want_mean:.4} ({:.2} se); \
             E[ln S_T] {lm:.6} against {want_lm:.6} ({:.2} se); \
             Var[ln S_T] {lv:.6} against exact {want_lv:.6} ({:.2} se)",
            est.price,
            (est.price - want_mean).abs() / est.std_err,
            (lm - want_lm).abs() / se_lm,
            (lv - want_lv).abs() / se_lv
        );
        assert!(
            (est.price - want_mean).abs() < SIGMAS * est.std_err,
            "E[S_T] {:.6} against {want_mean:.6}",
            est.price
        );
        assert!((lm - want_lm).abs() < SIGMAS * se_lm, "E[ln S_T] {lm:.6}");
        assert!((lv - want_lv).abs() < SIGMAS * se_lv, "Var[ln S_T] {lv:.6}");
    }
}

#[test]
fn a_simulated_european_call_converges_to_the_closed_form() {
    const N: usize = 400_000;
    let mut worst = 0.0_f64;
    for &(k, t, sigma) in &[
        (90.0, 1.0, 0.2),
        (100.0, 1.0, 0.2),
        (110.0, 1.0, 0.2),
        (100.0, 0.25, 0.45),
        (100.0, 2.0, 0.1),
    ] {
        let (s0, r, q) = (100.0, 0.05, 0.02);
        let g = Gbm::new(s0, t, r, q, sigma, 50);
        let mut rng = Rng::new(0xBEEF_0000 ^ (k as u64) << 8 ^ (sigma * 100.0) as u64);
        let mut a = Accum::default();
        for _ in 0..N {
            let s = titan_core::paths::terminal(&g, &mut rng);
            a.push((s - k).max(0.0));
        }
        let est = a.finish(g.discount);
        let exact = price(
            &Contract { s: s0, k, t, r, q, sigma },
            Kind::Call,
        );
        let sigmas = (est.price - exact).abs() / est.std_err;
        worst = worst.max(sigmas);
        println!(
            "K={k:5.0} T={t:4.2} sigma={sigma:4.2}: simulated {:.5} +/- {:.5}, \
             closed form {exact:.5}, {sigmas:.2} standard errors apart",
            est.price, est.std_err
        );
        assert!(
            sigmas < SIGMAS,
            "simulated {:.6} is {sigmas:.2} standard errors from {exact:.6}",
            est.price
        );
    }
    println!("worst departure across the five contracts: {worst:.2} standard errors");
}

#[test]
fn a_simulated_geometric_asian_converges_to_its_exact_discrete_form() {
    // This checks the averaging accumulator, not the stepper: the same paths
    // priced as a European already agree with Black-Scholes above.
    const N: usize = 200_000;
    for &(k, steps) in &[(90.0, 12usize), (100.0, 12), (100.0, 250), (110.0, 250)] {
        let (s0, t, r, q, sigma) = (100.0, 1.0, 0.05, 0.02, 0.25);
        let g = Gbm::new(s0, t, r, q, sigma, steps);
        let mut rng = Rng::new(0x5A5A_0000 ^ steps as u64);
        let mut a = Accum::default();
        for _ in 0..N {
            let (_, geo) = asian_geometric(&g, &mut rng);
            a.push((geo - k).max(0.0));
        }
        let est = a.finish(g.discount);
        let exact = geometric_asian_call(s0, k, t, r, q, sigma, steps);
        let sigmas = (est.price - exact).abs() / est.std_err;
        println!(
            "K={k:5.0} steps={steps:3}: simulated {:.5} +/- {:.5}, exact discrete {exact:.5}, \
             {sigmas:.2} standard errors apart",
            est.price, est.std_err
        );
        assert!(
            sigmas < SIGMAS,
            "simulated {:.6} is {sigmas:.2} standard errors from the exact {exact:.6}",
            est.price
        );
    }
    // The discrete closed form must tend to the continuous one, which is a
    // check on the discrete moments rather than on the simulation.
    let (s0, k, t, r, q, sigma) = (100.0, 100.0, 1.0, 0.05, 0.02, 0.25);
    let continuous = price(
        &Contract {
            s: s0,
            k,
            t,
            r,
            q: 0.5 * (r + q) + sigma * sigma / 12.0,
            sigma: sigma / 3.0_f64.sqrt(),
        },
        Kind::Call,
    );
    let mut last = f64::MAX;
    for &n in &[10usize, 100, 1_000, 100_000] {
        let gap = (geometric_asian_call(s0, k, t, r, q, sigma, n) - continuous).abs();
        println!("  n={n:6}: gap to the continuous limit {gap:.3e}");
        assert!(gap < last, "the gap grew at n={n}");
        last = gap;
    }
    assert!(last < 1e-4, "the discrete form does not reach its limit: {last:.3e}");
}

#[test]
fn in_out_parity_holds_path_by_path() {
    // A knock-in and a knock-out on the same path and the same barrier sum
    // to the vanilla payoff, because exactly one of them is alive. This is
    // an identity, so it holds exactly and needs no closed form. It is the
    // strongest available check on the monitor's state logic.
    let (s0, t, r, q, sigma) = (100.0, 1.0, 0.03, 0.0, 0.3);
    let k = 100.0;
    for &(barrier, side) in &[
        (80.0, Side::Down),
        (95.0, Side::Down),
        (105.0, Side::Up),
        (130.0, Side::Up),
    ] {
        let g = Gbm::new(s0, t, r, q, sigma, 100);
        let seed = 0x1234_5678 ^ (barrier as u64);
        let mut rng = Rng::new(seed);
        let (mut in_sum, mut out_sum, mut van_sum) = (0.0_f64, 0.0_f64, 0.0_f64);
        let mut touches = 0usize;
        const N: usize = 50_000;
        for _ in 0..N {
            let (s, touched, _) = barrier_log_space(&g, &mut rng, barrier, side, false);
            let payoff = (s - k).max(0.0);
            if touched {
                in_sum += payoff;
                touches += 1;
            } else {
                out_sum += payoff;
            }
            van_sum += payoff;
        }
        let residual = (in_sum + out_sum - van_sum).abs() / van_sum.max(1.0);
        println!(
            "barrier {barrier:5.0} {side:?}: touched {:.2}% of paths, in-out parity residual {residual:.3e}",
            100.0 * touches as f64 / N as f64
        );
        assert!(residual < 1e-12, "parity residual {residual:.3e}");
        // Both legs must be non-trivial, or the parity check is vacuous.
        assert!(
            touches > N / 1000 && touches < N - N / 1000,
            "barrier {barrier} touched {touches} of {N} paths, so one leg is empty"
        );
    }
}

#[test]
fn an_unreachable_barrier_gives_the_vanilla_path_exactly() {
    // With a barrier that cannot be reached the monitor must not alter the
    // path, so the terminal price has to match the plain generator bit for
    // bit on the same variates. This catches a monitor that consumes a
    // variate, reorders one, or perturbs the accumulator.
    let g = Gbm::new(100.0, 1.0, 0.04, 0.01, 0.25, 250);
    for &(barrier, side) in &[(1e-12, Side::Down), (1e12, Side::Up)] {
        let seed = 0x0DD0_BEEF;
        let mut a = Rng::new(seed);
        let mut b = Rng::new(seed);
        let mut c = Rng::new(seed);
        for _ in 0..2_000 {
            let plain = titan_core::paths::terminal(&g, &mut a);
            let (log_s, t1, seen) = barrier_log_space(&g, &mut b, barrier, side, true);
            let (price_s, t2) = barrier_price_space(&g, &mut c, barrier, side);
            assert_eq!(plain, log_s, "log-space monitor changed the path");
            assert!(!t1 && !t2, "an unreachable barrier was reported as touched");
            assert_eq!(seen, g.steps as u32, "early exit fired without a touch");
            // Price space accumulates the same logarithm and exponentiates
            // it the same way, so this is also exact.
            assert_eq!(plain, price_s, "price-space monitor changed the path");
        }
    }
}

#[test]
fn monitoring_more_often_can_only_find_more_touches() {
    // The brief this module answers claims that checking a barrier less often
    // raises survival. On one fixed set of paths that is a monotonicity
    // statement, and it is a property of the monitor: a coarser schedule
    // observes a subset of a finer schedule's points, so it cannot see a
    // touch the finer one misses.
    let (s0, t, r, q, sigma) = (100.0, 1.0, 0.0, 0.0, 0.35);
    let barrier = 85.0;
    let mut previous = 0.0_f64;
    for &steps in &[4usize, 12, 52, 250, 1_000] {
        let g = Gbm::new(s0, t, r, q, sigma, steps);
        let mut rng = Rng::new(0xA11C_E000);
        let mut touched = 0usize;
        const N: usize = 40_000;
        for _ in 0..N {
            let (_, hit, _) = barrier_log_space(&g, &mut rng, barrier, Side::Down, false);
            touched += hit as usize;
        }
        let rate = touched as f64 / N as f64;
        println!("steps {steps:5}: touch probability {:.4}", rate);
        assert!(
            rate >= previous - 0.01,
            "touch probability fell from {previous:.4} to {rate:.4} when monitoring got finer"
        );
        previous = rate;
    }
    assert!(
        previous > 0.05,
        "the finest schedule found almost no touches, so the test proves nothing"
    );
}

#[test]
fn the_arms_that_must_agree_do_agree_on_identical_variates() {
    // Every comparison below reseeds both arms per path rather than sharing
    // one stream across the loop. That is not fastidiousness: an arm that
    // stops early consumes fewer variates than one that runs to maturity, so
    // a shared stream falls out of step after the first path and every
    // comparison afterwards is between two different paths. The first
    // version of this test shared a stream and reported a disagreement that
    // was entirely its own.
    let g = Gbm::new(100.0, 1.0, 0.04, 0.015, 0.3, 250);
    let (barrier, side) = (88.0, Side::Down);
    let mut buf = vec![0.0_f64; g.steps];

    // Branchless and branchy extremes are the same arithmetic in a different
    // shape, so they agree to the last bit. Both consume the same variates,
    // so one stream is safe here.
    let mut a = Rng::new(0xFEED_FACE);
    let mut b = Rng::new(0xFEED_FACE);
    for _ in 0..2_000 {
        assert_eq!(
            lookback_branchless(&g, &mut a),
            lookback_branchy(&g, &mut b),
            "branchless and branchy extremes differ"
        );
    }

    // Log space against price space. One compares the accumulator to the
    // logarithm of the barrier, the other exponentiates and compares to the
    // barrier itself, so a path that passes within a rounding of the barrier
    // may be classified differently. The terminal price must still match
    // exactly, and the flags must agree on all but a negligible share.
    let mut disagree = 0usize;
    const FLAG_PATHS: usize = 20_000;
    for p in 0..FLAG_PATHS as u64 {
        let mut c = Rng::new(0xC0DE_0000 ^ p);
        let mut d = Rng::new(0xC0DE_0000 ^ p);
        let (s1, t1, _) = barrier_log_space(&g, &mut c, barrier, side, false);
        let (s2, t2) = barrier_price_space(&g, &mut d, barrier, side);
        assert_eq!(s1, s2, "terminal prices differ between the two monitors");
        disagree += (t1 != t2) as usize;
    }

    // Early termination must change the work and not the verdict.
    let (mut exits_saved, mut full_steps) = (0u64, 0u64);
    const EXIT_PATHS: usize = 20_000;
    for p in 0..EXIT_PATHS as u64 {
        let mut e = Rng::new(0xEE00_0000 ^ p);
        let mut f = Rng::new(0xEE00_0000 ^ p);
        let (s_full, t_full, seen_full) = barrier_log_space(&g, &mut e, barrier, side, false);
        let (_, t_exit, seen_exit) = barrier_log_space(&g, &mut f, barrier, side, true);
        assert_eq!(t_full, t_exit, "early exit changed the touch verdict");
        assert!(seen_exit <= seen_full, "early exit observed more steps");
        assert_eq!(seen_full, g.steps as u32, "the full monitor cut a path short");
        if !t_exit {
            assert_eq!(seen_exit, seen_full, "early exit stopped without a touch");
        }
        assert!(s_full.is_finite());
        exits_saved += (seen_full - seen_exit) as u64;
        full_steps += seen_full as u64;
    }

    // Rolling state against a stored path: the same statistics in one pass
    // and in two. The sums accumulate in the same order, so these agree to
    // the last bit.
    let mut worst_stats = 0.0_f64;
    for p in 0..2_000u64 {
        let mut h = Rng::new(0x5111_0000 ^ p);
        let mut i = Rng::new(0x5111_0000 ^ p);
        let roll = rolling(&g, &mut h, barrier, side);
        let st = stored(&g, &mut i, barrier, side, &mut buf).expect("buffer too short");
        assert_eq!(roll.touched, st.touched);
        assert_eq!(roll.observed, st.observed);
        for (x, y) in [
            (roll.terminal, st.terminal),
            (roll.sum, st.sum),
            (roll.log_sum, st.log_sum),
            (roll.max, st.max),
            (roll.min, st.min),
        ] {
            worst_stats = worst_stats.max((x - y).abs() / y.abs().max(1.0));
        }
    }

    let flag_rate = disagree as f64 / FLAG_PATHS as f64;
    println!(
        "log space against price space: touch flags differed on {} of {FLAG_PATHS} paths ({:.4}%)",
        disagree,
        100.0 * flag_rate
    );
    println!(
        "early termination saved {:.1}% of observations over {EXIT_PATHS} paths",
        100.0 * exits_saved as f64 / full_steps as f64
    );
    println!("rolling against stored: worst relative difference {worst_stats:.3e}");
    // A disagreement at the rounding of the barrier is expected; a
    // systematic one is not.
    assert!(
        flag_rate < 0.002,
        "the two monitors disagreed on {:.3}% of paths",
        100.0 * flag_rate
    );
    assert!(worst_stats < 1e-15, "rolling and stored differ by {worst_stats:.3e}");
    // Early termination must actually save work, or the arm is pointless.
    assert!(
        exits_saved > full_steps / 100,
        "early termination saved almost nothing, so the comparison is vacuous"
    );
    // A buffer shorter than the path must be refused, not quietly truncated.
    let mut tiny = vec![0.0; g.steps - 1];
    let mut j = Rng::new(1);
    assert!(stored(&g, &mut j, barrier, side, &mut tiny).is_none());
}

#[test]
fn the_arithmetic_and_geometric_averages_order_correctly() {
    // The geometric mean of positive numbers never exceeds the arithmetic
    // mean, with equality only for a constant path. That is an inequality the
    // two accumulators must respect on every path, and it is independent of
    // any pricing formula.
    let g = Gbm::new(100.0, 1.0, 0.03, 0.0, 0.4, 250);
    let seed = 0x7E57_7E57;
    let mut a = Rng::new(seed);
    let mut b = Rng::new(seed);
    let mut worst_gap = 0.0_f64;
    for _ in 0..5_000 {
        let (s1, arith) = asian_arithmetic(&g, &mut a);
        let (s2, geo) = asian_geometric(&g, &mut b);
        assert_eq!(s1, s2, "the two averaging arms walked different paths");
        assert!(
            geo <= arith * (1.0 + 1e-12),
            "geometric {geo:.8} exceeded arithmetic {arith:.8}"
        );
        assert!(geo > 0.0 && arith > 0.0, "a non-positive average");
        worst_gap = worst_gap.max((arith - geo) / arith);
    }
    println!("largest relative gap between the arithmetic and geometric averages: {worst_gap:.4}");
    assert!(worst_gap > 1e-4, "the two averages never separated, so the test is vacuous");
}
