//! Validation of the pricing kernel against oracles that share none of its
//! machinery.
//!
//! Three independent checks, in increasing strength. The cumulative normal is
//! checked against Simpson integration of the standard normal density, which
//! uses no tabulated constant. Every analytic sensitivity is checked against
//! a central difference of a price function built on that integrated normal,
//! so a wrong derivative cannot hide behind the same approximation that
//! produced it. And put-call parity is an algebraic identity the model must
//! satisfy exactly, independent of either.

use titan_core::pricing::{
    chain, delta, gamma, greeks, greeks_f32, greeks_separate, normal_cdf, normal_pdf, price, rho,
    theta, vega, Contract, Greeks, Kind,
};

/// Standard normal cumulative distribution by composite Simpson integration
/// of the density, with no tabulated coefficient.
///
/// The tail is integrated directly rather than by subtracting a half from an
/// integral that has grown to a half, because that subtraction destroys the
/// far tail: at x = -8.9 the true value is near 1e-19 while `0.5 - half`
/// carries 1e-16 of absolute error, so every digit would be noise. Computing
/// the tail from x outward instead keeps the result accurate relatively, not
/// just absolutely, over the whole range used here.
///
/// The running sum is compensated, because 4000 terms of plain summation
/// accumulate roughly the square root of that many roundings, and the finite
/// differences below divide this result by a step squared, which would
/// amplify exactly that error.
fn cdf_oracle(x: f64) -> f64 {
    let a = x.abs();
    let tail = normal_tail(a);
    if x < 0.0 {
        tail
    } else {
        1.0 - tail
    }
}

/// The upper tail from `a >= 0` outward, by compensated composite Simpson.
///
/// The span is chosen so the omitted remainder past it is negligible against
/// the tail itself: beyond `a` the density falls off at least as fast as
/// `exp(-a*u)`, so a span of `40/max(a,1)` leaves a relative remainder near
/// `exp(-40)`.
fn normal_tail(a: f64) -> f64 {
    debug_assert!(a >= 0.0);
    if a > 40.0 {
        return 0.0;
    }
    const N: usize = 4000;
    let span = (40.0 / a.max(1.0)).max(1.0);
    let h = span / N as f64;
    // Kahan compensated summation.
    let (mut sum, mut comp) = (0.0_f64, 0.0_f64);
    let mut add = |v: f64| {
        let y = v - comp;
        let t = sum + y;
        comp = (t - sum) - y;
        sum = t;
    };
    add(normal_pdf(a));
    add(normal_pdf(a + span));
    for i in 1..N {
        let u = a + i as f64 * h;
        add(if i % 2 == 0 { 2.0 } else { 4.0 } * normal_pdf(u));
    }
    sum * h / 3.0
}

/// Price built on the integrated normal, for use as a finite-difference base.
fn price_oracle(c: &Contract, kind: Kind) -> f64 {
    let v = c.sigma * c.t.sqrt();
    let d1 = ((c.s / c.k).ln() + (c.r - c.q + 0.5 * c.sigma * c.sigma) * c.t) / v;
    let d2 = d1 - v;
    let dq = (-c.q * c.t).exp();
    let dr = (-c.r * c.t).exp();
    match kind {
        Kind::Call => c.s * dq * cdf_oracle(d1) - c.k * dr * cdf_oracle(d2),
        Kind::Put => c.k * dr * cdf_oracle(-d2) - c.s * dq * cdf_oracle(-d1),
    }
}

/// A spread of contracts covering deep in the money to deep out, a week to
/// two years, 5% to 120% volatility, zero and negative carry.
fn cases() -> Vec<Contract> {
    let mut v = Vec::new();
    for &s in &[1.0_f64, 42.0, 100.0, 4200.0] {
        for &moneyness in &[0.5_f64, 0.8, 0.95, 1.0, 1.05, 1.25, 2.0] {
            for &t in &[7.0 / 365.0, 0.25, 0.5, 1.0, 2.0] {
                for &sigma in &[0.05_f64, 0.2, 0.45, 1.2] {
                    for &(r, q) in &[(0.0, 0.0), (0.10, 0.0), (0.05, 0.03), (-0.005, 0.02)] {
                        v.push(Contract {
                            s,
                            k: s * moneyness,
                            t,
                            r,
                            q,
                            sigma,
                        });
                    }
                }
            }
        }
    }
    v
}

const KINDS: [Kind; 2] = [Kind::Call, Kind::Put];

#[test]
fn normal_cdf_matches_integration_of_the_density() {
    // The approximation's own stated bound is 7.5e-8 absolute. Check it on a
    // grid that includes both tails well past where the density is resolvable.
    let mut worst = 0.0_f64;
    let mut at = 0.0_f64;
    let mut x = -9.0_f64;
    while x <= 9.0 {
        let e = (normal_cdf(x) - cdf_oracle(x)).abs();
        if e > worst {
            worst = e;
            at = x;
        }
        x += 0.01;
    }
    println!("normal_cdf worst absolute error {worst:.3e} at x = {at:.2}");
    assert!(
        worst < 7.5e-8,
        "worst error {worst:.3e} at x={at} exceeds the 7.5e-8 bound"
    );
    // Anchors that do not depend on either implementation. The value at zero
    // is 0.5 only to the approximation's own accuracy: the polynomial does not
    // sum to exactly one half, and asserting that it does would be asserting a
    // precision this cumulative normal never claimed.
    assert!((normal_cdf(0.0) - 0.5).abs() < 7.5e-8);
    assert!(normal_cdf(-40.0) >= 0.0, "no negative probability in the tail");
    assert!(normal_cdf(40.0) <= 1.0, "no probability above one");
    // Symmetry. For a strictly positive argument the reflection makes this an
    // identity up to one rounding: the positive side returns one minus the
    // tail and the negative side returns the same tail. At exactly zero it is
    // not an identity at all, because negative zero does not compare less than
    // zero, so both calls take the same branch and the sum departs by twice
    // the approximation's error. That is a property of the sign test, not a
    // defect, and the loop starts past it for that reason.
    for i in 1..400 {
        let x = i as f64 * 0.025;
        let e = (normal_cdf(x) + normal_cdf(-x) - 1.0).abs();
        assert!(e < 1e-15, "symmetry broken at {x}: {e:.3e}");
    }
    let at_zero = (normal_cdf(0.0) + normal_cdf(-0.0) - 1.0).abs();
    assert!(at_zero < 1.5e-7, "negative zero: {at_zero:.3e}");
}

#[test]
fn every_sensitivity_matches_a_central_difference_of_the_price() {
    // A central difference cannot validate a sensitivity smaller than its own
    // cancellation noise. Subtracting two prices of order P that differ in the
    // last few digits leaves an error of order eps*P, and dividing by h or by
    // h squared amplifies it. So each case computes that floor and is only
    // compared where the analytic value stands above it; below it, the test
    // instead requires the analytic value to be small too, which still catches
    // a wrong closed form, since a wrong one is wrong by a factor.
    //
    // The deep in the money short-dated case is why this is necessary: at
    // S=4200, K=3990, T=0.019 and sigma=0.05 the option is 7.7 standard
    // deviations in the money and its true gamma is near 1e-15, while the
    // second difference of a price near 218 cannot resolve below 1e-13.
    const EPS: f64 = f64::EPSILON;
    // The tolerance has two parts, because two different things limit the
    // comparison. An absolute part, set by the difference's own cancellation
    // noise: subtracting two prices of order P that agree to the last few
    // digits leaves an error of order eps*P, which dividing by h or by h
    // squared amplifies. And a relative part, set by the cumulative normal's
    // 7.5e-8 of absolute error, which enters each sensitivity scaled by that
    // sensitivity's own magnitude. Neither part alone is a usable criterion:
    // relative error is meaningless where the true value is 1e-15, and
    // absolute error is meaningless where it is 1e+3.
    // The absolute part also carries the oracle's own accumulated rounding.
    // Compensated summation reduces it but does not remove it, and the second
    // difference divides by a step squared, so the factor is kept explicit
    // rather than buried in a looser relative bound.
    const ABS_MULT: f64 = 20.0;
    const ORACLE_GROWTH: f64 = 64.0;
    const REL: f64 = 2e-3;
    let mut worst = [0.0_f64; 5];
    let mut worst_rel = [0.0_f64; 5];
    let mut where_at: [Option<Contract>; 5] = [None; 5];
    let names = ["delta", "gamma", "vega", "theta", "rho"];

    for c in cases() {
        for kind in KINDS {
            let g = greeks(&c, kind);
            // Every step is a fraction of the width of gamma's peak in spot
            // rather than of the spot itself: a short-dated low-volatility
            // option is a spike whose curvature a step sized from the spot
            // would smooth away.
            let width = c.s * c.sigma * c.t.sqrt();
            let pmag = price_oracle(&c, kind).abs().max(c.s);
            let mut rec = |i: usize, analytic: f64, fd: f64, floor: f64| {
                let dev = (analytic - fd).abs();
                let tol = ABS_MULT * ORACLE_GROWTH * floor + REL * analytic.abs();
                let ratio = dev / tol;
                if ratio > worst[i] {
                    worst[i] = ratio;
                    where_at[i] = Some(c);
                }
                // Recorded for information only, over the cases where the
                // difference has signal well above its own floor.
                if analytic.abs() > 1e4 * ORACLE_GROWTH * floor {
                    worst_rel[i] = worst_rel[i].max(dev / analytic.abs());
                }
            };

            let h = width * 1e-3;
            rec(
                0,
                g.delta,
                (price_oracle(&Contract { s: c.s + h, ..c }, kind)
                    - price_oracle(&Contract { s: c.s - h, ..c }, kind))
                    / (2.0 * h),
                EPS * pmag / h,
            );

            let h = width * 2e-2;
            rec(
                1,
                g.gamma,
                (price_oracle(&Contract { s: c.s + h, ..c }, kind)
                    - 2.0 * price_oracle(&c, kind)
                    + price_oracle(&Contract { s: c.s - h, ..c }, kind))
                    / (h * h),
                EPS * pmag / (h * h),
            );

            let h = c.sigma * 1e-4;
            rec(
                2,
                g.vega,
                (price_oracle(&Contract { sigma: c.sigma + h, ..c }, kind)
                    - price_oracle(&Contract { sigma: c.sigma - h, ..c }, kind))
                    / (2.0 * h),
                EPS * pmag / h,
            );

            // The derivative in calendar time is the negative of the
            // derivative in time remaining, so the sign is flipped here.
            let h = c.t * 1e-4;
            rec(
                3,
                g.theta,
                -(price_oracle(&Contract { t: c.t + h, ..c }, kind)
                    - price_oracle(&Contract { t: c.t - h, ..c }, kind))
                    / (2.0 * h),
                EPS * pmag / h,
            );

            let h = 1e-6;
            rec(
                4,
                g.rho,
                (price_oracle(&Contract { r: c.r + h, ..c }, kind)
                    - price_oracle(&Contract { r: c.r - h, ..c }, kind))
                    / (2.0 * h),
                EPS * pmag / h,
            );
        }
    }

    for (i, n) in names.iter().enumerate() {
        println!(
            "{n:6} worst deviation {:.3} of its tolerance; worst relative deviation where the difference has signal {:.3e}",
            worst[i], worst_rel[i]
        );
    }
    for (i, n) in names.iter().enumerate() {
        assert!(
            worst[i] < 1.0,
            "{n} exceeds its tolerance by {:.3}x at {:?}",
            worst[i],
            where_at[i]
        );
    }
}

#[test]
fn put_call_parity_holds() {
    // C - P = S e^{-qT} - K e^{-rT}, exactly, for any model that prices both
    // legs off the same distribution. This catches a sign or discount error
    // that a one-sided test would miss.
    let mut worst = 0.0_f64;
    for c in cases() {
        let lhs = price(&c, Kind::Call) - price(&c, Kind::Put);
        let rhs = c.s * (-c.q * c.t).exp() - c.k * (-c.r * c.t).exp();
        worst = worst.max((lhs - rhs).abs() / c.s);
    }
    println!("put-call parity worst relative residual {worst:.3e}");
    assert!(worst < 1e-7, "parity residual {worst:.3e}");
}

#[test]
fn the_kind_independent_sensitivities_are_kind_independent() {
    // Gamma and vega are the same for a call and a put at the same strike,
    // and the two deltas differ by exactly the dividend discount factor.
    for c in cases() {
        let (a, b) = (greeks(&c, Kind::Call), greeks(&c, Kind::Put));
        assert_eq!(a.gamma, b.gamma, "gamma differs by kind");
        assert_eq!(a.vega, b.vega, "vega differs by kind");
        let want = (-c.q * c.t).exp();
        let got = a.delta - b.delta;
        assert!(
            (got - want).abs() < 1e-9,
            "delta_call - delta_put = {got}, expected {want}"
        );
    }
}

#[test]
fn the_fused_kernel_agrees_with_the_separate_functions() {
    // Reuse of the shared terms must not change the answer, so these agree to
    // the last bit except where the two orderings of the same products round
    // differently.
    let mut worst = 0.0_f64;
    for c in cases() {
        for kind in KINDS {
            let (a, b) = (greeks(&c, kind), greeks_separate(&c, kind));
            let s = c.s;
            for (x, y) in [
                (a.price, b.price),
                (a.delta, b.delta),
                (a.gamma, b.gamma),
                (a.vega, b.vega),
                (a.theta, b.theta),
                (a.rho, b.rho),
            ] {
                worst = worst.max((x - y).abs() / s.max(1.0));
            }
        }
    }
    println!("fused vs separate worst relative difference {worst:.3e}");
    assert!(worst < 1e-14, "fused and separate disagree by {worst:.3e}");
}

#[test]
fn the_chain_agrees_with_the_scalar_kernel_strike_by_strike() {
    // Hoisting the strike-independent terms out of the loop must not change
    // any strike's answer.
    let strikes: Vec<f64> = (1..=40).map(|i| 60.0 + i as f64 * 2.0).collect();
    let mut out = vec![Greeks::default(); strikes.len()];
    let mut worst = 0.0_f64;
    for &t in &[7.0 / 365.0, 0.5, 2.0] {
        for &sigma in &[0.05_f64, 0.2, 1.2] {
            for kind in KINDS {
                let c = Contract { s: 100.0, k: 100.0, t, r: 0.04, q: 0.01, sigma };
                let n = chain(&c, kind, &strikes, &mut out);
                assert_eq!(n, strikes.len());
                for (i, &k) in strikes.iter().enumerate() {
                    let want = greeks(&Contract { k, ..c }, kind);
                    for (x, y) in [
                        (out[i].price, want.price),
                        (out[i].delta, want.delta),
                        (out[i].gamma, want.gamma),
                        (out[i].vega, want.vega),
                        (out[i].theta, want.theta),
                        (out[i].rho, want.rho),
                    ] {
                        worst = worst.max((x - y).abs() / 100.0);
                    }
                }
            }
        }
    }
    println!("chain vs scalar worst relative difference {worst:.3e}");
    assert!(worst < 1e-14, "chain and scalar disagree by {worst:.3e}");
    // A shorter output slice must write only what fits and report that count.
    let mut two = vec![Greeks::default(); 2];
    let c = Contract { s: 100.0, k: 100.0, t: 0.5, r: 0.0, q: 0.0, sigma: 0.2 };
    assert_eq!(chain(&c, Kind::Call, &strikes, &mut two), 2);
    // A tolerance rather than equality, and for the same reason the main loop
    // above uses one: the hoisted and scalar forms are different arithmetic
    // shapes, and asserting that two such shapes agree to the last bit is not
    // a portable claim. The path engine's suite asserted it and failed on
    // aarch64 by eight units in the last place while passing on x86-64.
    let want = greeks(&Contract { k: strikes[0], ..c }, Kind::Call);
    for (x, y) in [
        (two[0].price, want.price),
        (two[0].delta, want.delta),
        (two[0].gamma, want.gamma),
        (two[0].vega, want.vega),
        (two[0].theta, want.theta),
        (two[0].rho, want.rho),
    ] {
        assert!((x - y).abs() / y.abs().max(1.0) < 1e-14);
    }
}

#[test]
fn degenerate_inputs_give_intrinsic_value_and_never_a_nan() {
    // At expiry, with no volatility, or with a nonsensical spot or strike the
    // closed form divides by zero. A NaN would propagate silently through a
    // portfolio sum, so these return the discounted intrinsic value instead.
    let base = Contract { s: 100.0, k: 95.0, t: 0.5, r: 0.04, q: 0.01, sigma: 0.2 };
    let bad = [
        Contract { t: 0.0, ..base },
        Contract { t: -1.0, ..base },
        Contract { sigma: 0.0, ..base },
        Contract { sigma: -0.2, ..base },
        Contract { s: 0.0, ..base },
        Contract { k: 0.0, ..base },
        Contract { t: f64::NAN, ..base },
        Contract { sigma: f64::NAN, ..base },
    ];
    for c in bad {
        for kind in KINDS {
            for g in [greeks(&c, kind), greeks_separate(&c, kind), greeks_f32(&c, kind)] {
                for (n, x) in [
                    ("price", g.price),
                    ("delta", g.delta),
                    ("gamma", g.gamma),
                    ("vega", g.vega),
                    ("theta", g.theta),
                    ("rho", g.rho),
                ] {
                    assert!(x.is_finite(), "{n} is {x} for {c:?} {kind:?}");
                }
            }
            assert!(price(&c, kind).is_finite());
            for x in [
                delta(&c, kind),
                gamma(&c),
                vega(&c),
                theta(&c, kind),
                rho(&c, kind),
            ] {
                assert!(x.is_finite());
            }
        }
    }
    // At expiry the price is the intrinsic value and nothing else.
    let expired = Contract { t: 0.0, ..base };
    assert_eq!(price(&expired, Kind::Call), 5.0);
    assert_eq!(price(&expired, Kind::Put), 0.0);
}

#[test]
fn single_precision_stays_within_a_measured_bound_of_double() {
    // Not an assertion that f32 is accurate enough for any purpose: a
    // measurement of how far it departs, so the paper can state the cost of
    // the narrower width rather than guess it.
    let mut worst = [0.0_f64; 6];
    let names = ["price", "delta", "gamma", "vega", "theta", "rho"];
    for c in cases() {
        for kind in KINDS {
            let (a, b) = (greeks(&c, kind), greeks_f32(&c, kind));
            let scale = c.s;
            for (i, (x, y)) in [
                (a.price, b.price),
                (a.delta, b.delta),
                (a.gamma, b.gamma),
                (a.vega, b.vega),
                (a.theta, b.theta),
                (a.rho, b.rho),
            ]
            .into_iter()
            .enumerate()
            {
                let rel = (x - y).abs() / scale.max(1.0);
                worst[i] = worst[i].max(rel);
            }
        }
    }
    for (n, w) in names.iter().zip(worst) {
        println!("f32 vs f64 worst relative difference, {n:6} {w:.3e}");
    }
    for (n, w) in names.iter().zip(worst) {
        assert!(w < 1e-3, "{n} departs from double by {w:.3e}");
    }
}
