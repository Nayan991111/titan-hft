//! Closed-form option pricing and the five first-order sensitivities.
//!
//! The pricing model is Black and Scholes (Journal of Political Economy
//! 81(3), 1973) in the form Merton (Journal of Financial Economics 3, 1976)
//! gives for a continuous dividend yield, so a single implementation covers
//! a non-dividend-paying stock (`q = 0`) as a special case.
//!
//! The cumulative normal is the Hastings approximation tabulated as 26.2.17
//! in Abramowitz and Stegun (Handbook of Mathematical Functions, 1964),
//! whose stated bound is 7.5e-8 absolute. That bound is not taken on trust:
//! the tests integrate the standard normal density directly and check the
//! approximation against it, so the oracle contains no tabulated constant
//! of its own.
//!
//! Every sensitivity here is an analytic derivative of `price`, which is a
//! claim that can be checked rather than asserted, and the tests check it by
//! central difference. Units are per unit change in the underlying variable,
//! not the per-1% and per-day conventions a trading desk prefers, because
//! scaling at the call site keeps one convention out of the kernel.

/// One contract's parameters. Grouped so the hot functions take one
/// argument: six loose floats invite a transposition at the call site.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Contract {
    /// Spot price of the underlying.
    pub s: f64,
    /// Strike price.
    pub k: f64,
    /// Time to expiry in years.
    pub t: f64,
    /// Continuously compounded risk-free rate.
    pub r: f64,
    /// Continuous dividend yield.
    pub q: f64,
    /// Volatility, annualized.
    pub sigma: f64,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Kind {
    Call,
    Put,
}

/// Price and the five sensitivities, in per-unit terms: `vega` is per 1.0 of
/// volatility (divide by 100 for per-point), `theta` is per year (divide by
/// 365 for per-day), `rho` is per 1.0 of rate.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Greeks {
    pub price: f64,
    pub delta: f64,
    pub gamma: f64,
    pub vega: f64,
    pub theta: f64,
    pub rho: f64,
}

const INV_SQRT_2PI: f64 = 0.398_942_280_401_432_7;

/// Standard normal density.
#[inline(always)]
pub fn normal_pdf(x: f64) -> f64 {
    INV_SQRT_2PI * (-0.5 * x * x).exp()
}

/// Standard normal cumulative distribution, Abramowitz and Stegun 26.2.17.
///
/// One polynomial evaluation and one `exp`. The reflection at zero keeps the
/// approximation on the side where it is accurate; computing the far tail by
/// subtraction from one would lose the small value to cancellation.
#[inline(always)]
pub fn normal_cdf(x: f64) -> f64 {
    const P: f64 = 0.231_641_9;
    const B: [f64; 5] = [
        0.319_381_530,
        -0.356_563_782,
        1.781_477_937,
        -1.821_255_978,
        1.330_274_429,
    ];
    let neg = x < 0.0;
    let a = if neg { -x } else { x };
    let t = 1.0 / (1.0 + P * a);
    // Horner, so the polynomial is four multiply-adds rather than five powers.
    let poly = t * (B[0] + t * (B[1] + t * (B[2] + t * (B[3] + t * B[4]))));
    let upper = normal_pdf(a) * poly;
    if neg {
        upper
    } else {
        1.0 - upper
    }
}

/// `d1`, `d2` and `sigma * sqrt(t)`, which every closed form below needs.
#[inline(always)]
fn d1_d2(c: &Contract) -> (f64, f64, f64) {
    let v = c.sigma * c.t.sqrt();
    let d1 = ((c.s / c.k).ln() + (c.r - c.q + 0.5 * c.sigma * c.sigma) * c.t) / v;
    (d1, d1 - v, v)
}

/// True when the closed form is undefined: at or past expiry, or with no
/// volatility, `d1` divides by zero. The contract is then worth its
/// discounted intrinsic value and the sensitivities are not defined, so the
/// callers below return that value and zeroed sensitivities rather than a
/// NaN that would propagate silently through a portfolio sum.
#[inline(always)]
fn degenerate(c: &Contract) -> bool {
    unusable(c.t) || unusable(c.sigma) || unusable(c.s) || unusable(c.k)
}

/// Not a strictly positive number. The NaN test is not redundant: a NaN
/// compares false against every bound, so `x <= 0.0` alone would let one
/// through into the closed form and out again as a NaN sensitivity.
#[inline(always)]
fn unusable(x: f64) -> bool {
    x.is_nan() || x <= 0.0
}

#[inline(always)]
fn intrinsic(c: &Contract, kind: Kind) -> f64 {
    let f = c.s * (-c.q * c.t).exp();
    let k = c.k * (-c.r * c.t).exp();
    match kind {
        Kind::Call => (f - k).max(0.0),
        Kind::Put => (k - f).max(0.0),
    }
}

/// Black-Scholes-Merton price.
#[inline(always)]
pub fn price(c: &Contract, kind: Kind) -> f64 {
    if degenerate(c) {
        return intrinsic(c, kind);
    }
    let (d1, d2, _) = d1_d2(c);
    let dq = (-c.q * c.t).exp();
    let dr = (-c.r * c.t).exp();
    match kind {
        Kind::Call => c.s * dq * normal_cdf(d1) - c.k * dr * normal_cdf(d2),
        Kind::Put => c.k * dr * normal_cdf(-d2) - c.s * dq * normal_cdf(-d1),
    }
}

/// Price and all five sensitivities from one evaluation of the shared terms.
///
/// `d1`, `d2`, the two discount factors, `N(d1)`, `N(d2)` and `n(d1)` are
/// computed once and reused. The five single-sensitivity functions below
/// each recompute all of them, which is what makes the cost of reuse
/// measurable rather than assumed.
#[inline(always)]
pub fn greeks(c: &Contract, kind: Kind) -> Greeks {
    if degenerate(c) {
        return Greeks {
            price: intrinsic(c, kind),
            ..Greeks::default()
        };
    }
    let (d1, d2, v) = d1_d2(c);
    let dq = (-c.q * c.t).exp();
    let dr = (-c.r * c.t).exp();
    let nd1 = normal_pdf(d1);
    let sdq = c.s * dq;
    let kdr = c.k * dr;

    // Gamma and vega do not depend on the kind: both kinds share the same
    // second derivative in spot and the same first derivative in volatility.
    let gamma = dq * nd1 / (c.s * v);
    let vega = sdq * nd1 * c.t.sqrt();
    let decay = -sdq * nd1 * c.sigma / (2.0 * c.t.sqrt());

    match kind {
        Kind::Call => {
            let (n1, n2) = (normal_cdf(d1), normal_cdf(d2));
            Greeks {
                price: sdq * n1 - kdr * n2,
                delta: dq * n1,
                gamma,
                vega,
                theta: decay + c.q * sdq * n1 - c.r * kdr * n2,
                rho: kdr * c.t * n2,
            }
        }
        Kind::Put => {
            let (n1, n2) = (normal_cdf(-d1), normal_cdf(-d2));
            Greeks {
                price: kdr * n2 - sdq * n1,
                delta: -dq * n1,
                gamma,
                vega,
                theta: decay - c.q * sdq * n1 + c.r * kdr * n2,
                rho: -kdr * c.t * n2,
            }
        }
    }
}

#[inline(always)]
pub fn delta(c: &Contract, kind: Kind) -> f64 {
    if degenerate(c) {
        return 0.0;
    }
    let (d1, _, _) = d1_d2(c);
    let dq = (-c.q * c.t).exp();
    match kind {
        Kind::Call => dq * normal_cdf(d1),
        Kind::Put => -dq * normal_cdf(-d1),
    }
}

#[inline(always)]
pub fn gamma(c: &Contract) -> f64 {
    if degenerate(c) {
        return 0.0;
    }
    let (d1, _, v) = d1_d2(c);
    (-c.q * c.t).exp() * normal_pdf(d1) / (c.s * v)
}

#[inline(always)]
pub fn vega(c: &Contract) -> f64 {
    if degenerate(c) {
        return 0.0;
    }
    let (d1, _, _) = d1_d2(c);
    c.s * (-c.q * c.t).exp() * normal_pdf(d1) * c.t.sqrt()
}

#[inline(always)]
pub fn theta(c: &Contract, kind: Kind) -> f64 {
    if degenerate(c) {
        return 0.0;
    }
    let (d1, d2, _) = d1_d2(c);
    let dq = (-c.q * c.t).exp();
    let dr = (-c.r * c.t).exp();
    let decay = -c.s * dq * normal_pdf(d1) * c.sigma / (2.0 * c.t.sqrt());
    match kind {
        Kind::Call => {
            decay + c.q * c.s * dq * normal_cdf(d1) - c.r * c.k * dr * normal_cdf(d2)
        }
        Kind::Put => {
            decay - c.q * c.s * dq * normal_cdf(-d1) + c.r * c.k * dr * normal_cdf(-d2)
        }
    }
}

#[inline(always)]
pub fn rho(c: &Contract, kind: Kind) -> f64 {
    if degenerate(c) {
        return 0.0;
    }
    let (_, d2, _) = d1_d2(c);
    let kdr = c.k * (-c.r * c.t).exp() * c.t;
    match kind {
        Kind::Call => kdr * normal_cdf(d2),
        Kind::Put => -kdr * normal_cdf(-d2),
    }
}

/// The five sensitivities assembled from the five single functions, so the
/// cost of recomputing the shared terms can be measured against `greeks`.
#[inline(always)]
pub fn greeks_separate(c: &Contract, kind: Kind) -> Greeks {
    Greeks {
        price: price(c, kind),
        delta: delta(c, kind),
        gamma: gamma(c),
        vega: vega(c),
        theta: theta(c, kind),
        rho: rho(c, kind),
    }
}

/// One expiry's strike ladder, written into `out`.
///
/// A quoted option chain is exactly this shape: one spot and one expiry
/// against many strikes, so the terms that do not depend on the strike are
/// hoisted out of the loop. Returns the number of strikes written, which is
/// the shorter of the two slices.
pub fn chain(c: &Contract, kind: Kind, strikes: &[f64], out: &mut [Greeks]) -> usize {
    let n = strikes.len().min(out.len());
    if unusable(c.t) || unusable(c.sigma) || unusable(c.s) {
        for (slot, &k) in out[..n].iter_mut().zip(&strikes[..n]) {
            let one = Contract { k, ..*c };
            *slot = greeks(&one, kind);
        }
        return n;
    }
    // Strike-independent terms, hoisted.
    let sqrt_t = c.t.sqrt();
    let v = c.sigma * sqrt_t;
    let drift = (c.r - c.q + 0.5 * c.sigma * c.sigma) * c.t;
    let dq = (-c.q * c.t).exp();
    let dr = (-c.r * c.t).exp();
    let sdq = c.s * dq;
    let half = c.sigma / (2.0 * sqrt_t);

    for (slot, &k) in out[..n].iter_mut().zip(&strikes[..n]) {
        if unusable(k) {
            *slot = greeks(&Contract { k, ..*c }, kind);
            continue;
        }
        let d1 = ((c.s / k).ln() + drift) / v;
        let d2 = d1 - v;
        let nd1 = normal_pdf(d1);
        let kdr = k * dr;
        let gam = dq * nd1 / (c.s * v);
        let veg = sdq * nd1 * sqrt_t;
        let decay = -sdq * nd1 * half;
        *slot = match kind {
            Kind::Call => {
                let (n1, n2) = (normal_cdf(d1), normal_cdf(d2));
                Greeks {
                    price: sdq * n1 - kdr * n2,
                    delta: dq * n1,
                    gamma: gam,
                    vega: veg,
                    theta: decay + c.q * sdq * n1 - c.r * kdr * n2,
                    rho: kdr * c.t * n2,
                }
            }
            Kind::Put => {
                let (n1, n2) = (normal_cdf(-d1), normal_cdf(-d2));
                Greeks {
                    price: kdr * n2 - sdq * n1,
                    delta: -dq * n1,
                    gamma: gam,
                    vega: veg,
                    theta: decay - c.q * sdq * n1 + c.r * kdr * n2,
                    rho: -kdr * c.t * n2,
                }
            }
        };
    }
    n
}

/// Single-precision price and sensitivities.
///
/// Same closed forms in `f32`, so the question of what halving the width
/// buys and what it costs in accuracy is a measurement rather than an
/// opinion. The parameters stay `f64` at the boundary and are narrowed once,
/// so the comparison is of the arithmetic and not of the call signature.
#[inline(always)]
pub fn greeks_f32(c: &Contract, kind: Kind) -> Greeks {
    if degenerate(c) {
        return Greeks {
            price: intrinsic(c, kind),
            ..Greeks::default()
        };
    }
    let (s, k, t) = (c.s as f32, c.k as f32, c.t as f32);
    let (r, q, sg) = (c.r as f32, c.q as f32, c.sigma as f32);
    let sqrt_t = t.sqrt();
    let v = sg * sqrt_t;
    let d1 = ((s / k).ln() + (r - q + 0.5 * sg * sg) * t) / v;
    let d2 = d1 - v;
    let dq = (-q * t).exp();
    let dr = (-r * t).exp();
    let nd1 = normal_pdf32(d1);
    let sdq = s * dq;
    let kdr = k * dr;
    let gam = dq * nd1 / (s * v);
    let veg = sdq * nd1 * sqrt_t;
    let decay = -sdq * nd1 * sg / (2.0 * sqrt_t);
    let (sign, n1, n2) = match kind {
        Kind::Call => (1.0f32, normal_cdf32(d1), normal_cdf32(d2)),
        Kind::Put => (-1.0f32, normal_cdf32(-d1), normal_cdf32(-d2)),
    };
    let (p, th) = match kind {
        Kind::Call => (
            sdq * n1 - kdr * n2,
            decay + q * sdq * n1 - r * kdr * n2,
        ),
        Kind::Put => (
            kdr * n2 - sdq * n1,
            decay - q * sdq * n1 + r * kdr * n2,
        ),
    };
    Greeks {
        price: p as f64,
        delta: (sign * dq * n1) as f64,
        gamma: gam as f64,
        vega: veg as f64,
        theta: th as f64,
        rho: (sign * kdr * t * n2) as f64,
    }
}

const INV_SQRT_2PI_32: f32 = 0.398_942_3;

#[inline(always)]
fn normal_pdf32(x: f32) -> f32 {
    INV_SQRT_2PI_32 * (-0.5 * x * x).exp()
}

#[inline(always)]
fn normal_cdf32(x: f32) -> f32 {
    const P: f32 = 0.231_641_9;
    const B: [f32; 5] = [0.319_381_53, -0.356_563_78, 1.781_477_9, -1.821_256, 1.330_274_4];
    let neg = x < 0.0;
    let a = if neg { -x } else { x };
    let t = 1.0 / (1.0 + P * a);
    let poly = t * (B[0] + t * (B[1] + t * (B[2] + t * (B[3] + t * B[4]))));
    let upper = normal_pdf32(a) * poly;
    if neg {
        upper
    } else {
        1.0 - upper
    }
}
