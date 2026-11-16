//! What path dependence costs per step.
//!
//! A vanilla option needs one number from a simulated path, its terminal
//! value. A path-dependent payoff needs a statistic accumulated along the
//! way: a running sum for an arithmetic average, a running product for a
//! geometric one, a running extreme for a lookback, a survival flag for a
//! barrier. Each of those is a few instructions per step, and this module
//! exists so that "a few instructions per step" can be replaced by a
//! measurement.
//!
//! The underlying is geometric Brownian motion under the risk-neutral
//! measure, discretized exactly rather than by an Euler step: the log price
//! increment over `dt` is normal with mean `(r - q - sigma^2/2) dt` and
//! variance `sigma^2 dt`, so the discretization introduces no bias at any
//! step size and the only error in a price is Monte Carlo error. That
//! matters here because the arms below are compared against each other and
//! against closed forms, and a biased stepper would confound both.
//!
//! The central observation this module is built to measure: a barrier
//! comparison and a running extreme are monotone, and a geometric average is
//! additive, in the logarithm. So all three can be accumulated in log space
//! with no exponential per step, while an arithmetic average needs the price
//! itself and therefore needs one exponential every step. Both forms of each
//! arm are implemented so the difference is measured rather than argued.
//!
//! Generator: xoshiro256++ (Blackman and Vigna), seeded through splitmix64,
//! with normal variates by the Box-Muller transform. Both are validated in
//! the tests against moments and against a Kolmogorov-Smirnov statistic
//! computed with the cumulative normal from `crate::pricing`, which is
//! itself validated against Simpson integration of the density. Nothing in
//! that chain rests on a tabulated constant.

/// xoshiro256++ 1.0. Deterministic, seeded, and reproducible across runs,
/// which every measurement here depends on.
#[derive(Clone)]
pub struct Rng {
    s: [u64; 4],
}

impl Rng {
    /// Seeds the state with splitmix64, which avoids the poor initial
    /// behaviour an all-zero or low-entropy state would give.
    pub fn new(seed: u64) -> Self {
        let mut z = seed;
        let mut next = || {
            z = z.wrapping_add(0x9E37_79B9_7F4A_7C15);
            let mut x = z;
            x = (x ^ (x >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
            x = (x ^ (x >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
            x ^ (x >> 31)
        };
        Self {
            s: [next(), next(), next(), next()],
        }
    }

    #[inline(always)]
    pub fn next_u64(&mut self) -> u64 {
        let r = self.s[0]
            .wrapping_add(self.s[3])
            .rotate_left(23)
            .wrapping_add(self.s[0]);
        let t = self.s[1] << 17;
        self.s[2] ^= self.s[0];
        self.s[3] ^= self.s[1];
        self.s[1] ^= self.s[2];
        self.s[0] ^= self.s[3];
        self.s[2] ^= t;
        self.s[3] = self.s[3].rotate_left(45);
        r
    }

    /// Uniform on [0, 1). 53 significant bits, the most a double holds.
    #[inline(always)]
    pub fn next_f64(&mut self) -> f64 {
        (self.next_u64() >> 11) as f64 * (1.0 / (1u64 << 53) as f64)
    }

    /// Uniform on (0, 1]. Box-Muller takes a logarithm of its first
    /// argument, so zero must be excluded rather than clamped away later.
    #[inline(always)]
    fn next_f64_nonzero(&mut self) -> f64 {
        ((self.next_u64() >> 11) + 1) as f64 * (1.0 / (1u64 << 53) as f64)
    }

    /// Two independent standard normal variates, by Box-Muller.
    ///
    /// Returned in pairs because the transform produces two for the price of
    /// one square root, one logarithm and one sine-cosine pair; discarding
    /// the second would double the cost per variate.
    #[inline(always)]
    pub fn next_normal_pair(&mut self) -> (f64, f64) {
        let u1 = self.next_f64_nonzero();
        let u2 = self.next_f64();
        let r = (-2.0 * u1.ln()).sqrt();
        let theta = std::f64::consts::TAU * u2;
        (r * theta.cos(), r * theta.sin())
    }
}

/// The process and the discretization, with the per-step constants computed
/// once rather than inside the stepping loop.
#[derive(Clone, Copy, Debug)]
pub struct Gbm {
    /// Spot at time zero.
    pub s0: f64,
    /// Drift per step in log space, `(r - q - sigma^2/2) * dt`.
    pub mu_dt: f64,
    /// Standard deviation per step in log space, `sigma * sqrt(dt)`.
    pub sig_sqrt_dt: f64,
    /// Discount factor over the whole horizon, `exp(-r * T)`.
    pub discount: f64,
    /// Steps to maturity.
    pub steps: usize,
}

impl Gbm {
    pub fn new(s0: f64, t: f64, r: f64, q: f64, sigma: f64, steps: usize) -> Self {
        let dt = t / steps as f64;
        Self {
            s0,
            mu_dt: (r - q - 0.5 * sigma * sigma) * dt,
            sig_sqrt_dt: sigma * dt.sqrt(),
            discount: (-r * t).exp(),
            steps,
        }
    }
}

/// Everything a path-dependent payoff in this module can need, accumulated
/// as the path is generated.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Stats {
    /// Terminal price.
    pub terminal: f64,
    /// Sum of the prices at the observation times, for an arithmetic average.
    pub sum: f64,
    /// Sum of the log prices, for a geometric average.
    pub log_sum: f64,
    /// Highest price at an observation time.
    ///
    /// The observation times are `t_1 .. t_n`, the ends of the steps, and
    /// they exclude the spot at time zero. That is the same window the two
    /// averaging fields above use, so a contract that averages and one that
    /// tracks an extreme see the same dates and one `Stats` has one meaning.
    /// Including time zero in the extremes and not in the averages is how an
    /// earlier version of this module disagreed with itself by six percent.
    pub max: f64,
    /// Lowest price at an observation time, over the same window as `max`.
    pub min: f64,
    /// Whether the barrier was touched at an observation time.
    pub touched: bool,
    /// Observations actually taken, which early termination can cut short.
    pub observed: u32,
}

/// Which side of the spot the barrier sits on, and so which comparison the
/// monitor makes.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Side {
    /// Touched when the price falls to the barrier or below.
    Down,
    /// Touched when the price rises to the barrier or above.
    Up,
}

/// Terminal price only: the vanilla baseline.
///
/// The log price is accumulated and exponentiated once at the end, so the
/// whole path costs a single exponential. Every other arm below is measured
/// against this one.
#[inline(always)]
pub fn terminal(g: &Gbm, rng: &mut Rng) -> f64 {
    let mut x = g.s0.ln();
    let mut i = 0;
    while i < g.steps {
        let (z1, z2) = rng.next_normal_pair();
        x += g.mu_dt + g.sig_sqrt_dt * z1;
        i += 1;
        if i < g.steps {
            x += g.mu_dt + g.sig_sqrt_dt * z2;
            i += 1;
        }
    }
    x.exp()
}

/// Arithmetic running average, which needs the price at every step and so
/// needs one exponential per step.
#[inline(always)]
pub fn asian_arithmetic(g: &Gbm, rng: &mut Rng) -> (f64, f64) {
    let mut x = g.s0.ln();
    let mut sum = 0.0;
    let mut i = 0;
    while i < g.steps {
        let (z1, z2) = rng.next_normal_pair();
        x += g.mu_dt + g.sig_sqrt_dt * z1;
        sum += x.exp();
        i += 1;
        if i < g.steps {
            x += g.mu_dt + g.sig_sqrt_dt * z2;
            sum += x.exp();
            i += 1;
        }
    }
    (x.exp(), sum / g.steps as f64)
}

/// Geometric running average, which is additive in the logarithm and so
/// needs no exponential until the path ends.
#[inline(always)]
pub fn asian_geometric(g: &Gbm, rng: &mut Rng) -> (f64, f64) {
    let mut x = g.s0.ln();
    let mut log_sum = 0.0;
    let mut i = 0;
    while i < g.steps {
        let (z1, z2) = rng.next_normal_pair();
        x += g.mu_dt + g.sig_sqrt_dt * z1;
        log_sum += x;
        i += 1;
        if i < g.steps {
            x += g.mu_dt + g.sig_sqrt_dt * z2;
            log_sum += x;
            i += 1;
        }
    }
    (x.exp(), (log_sum / g.steps as f64).exp())
}

/// Running extreme in log space with branchless minimum and maximum.
///
/// The comparison is monotone under the logarithm, so the extremes of the
/// log path exponentiate to the extremes of the price path, and the whole
/// path costs three exponentials rather than one per step.
#[inline(always)]
pub fn lookback_branchless(g: &Gbm, rng: &mut Rng) -> (f64, f64, f64) {
    debug_assert!(g.steps > 0);
    let mut x = g.s0.ln();
    // Unset, so the first observation sets both; see `Stats::max` for why
    // time zero is outside the window.
    let (mut hi, mut lo) = (f64::NEG_INFINITY, f64::INFINITY);
    let mut i = 0;
    while i < g.steps {
        let (z1, z2) = rng.next_normal_pair();
        x += g.mu_dt + g.sig_sqrt_dt * z1;
        hi = hi.max(x);
        lo = lo.min(x);
        i += 1;
        if i < g.steps {
            x += g.mu_dt + g.sig_sqrt_dt * z2;
            hi = hi.max(x);
            lo = lo.min(x);
            i += 1;
        }
    }
    (x.exp(), hi.exp(), lo.exp())
}

/// The same extremes written as explicit comparisons, so the cost of the
/// branch can be measured against the branchless form above.
#[inline(always)]
pub fn lookback_branchy(g: &Gbm, rng: &mut Rng) -> (f64, f64, f64) {
    debug_assert!(g.steps > 0);
    let mut x = g.s0.ln();
    // Unset, so the first observation sets both; see `Stats::max` for why
    // time zero is outside the window.
    let (mut hi, mut lo) = (f64::NEG_INFINITY, f64::INFINITY);
    let mut i = 0;
    while i < g.steps {
        let (z1, z2) = rng.next_normal_pair();
        x += g.mu_dt + g.sig_sqrt_dt * z1;
        if x > hi {
            hi = x;
        }
        if x < lo {
            lo = x;
        }
        i += 1;
        if i < g.steps {
            x += g.mu_dt + g.sig_sqrt_dt * z2;
            if x > hi {
                hi = x;
            }
            if x < lo {
                lo = x;
            }
            i += 1;
        }
    }
    (x.exp(), hi.exp(), lo.exp())
}

/// Barrier monitoring in price space: one exponential per step, then a
/// comparison against the barrier.
#[inline(always)]
pub fn barrier_price_space(g: &Gbm, rng: &mut Rng, barrier: f64, side: Side) -> (f64, bool) {
    let mut x = g.s0.ln();
    let mut touched = false;
    let mut i = 0;
    while i < g.steps {
        let (z1, z2) = rng.next_normal_pair();
        x += g.mu_dt + g.sig_sqrt_dt * z1;
        let s = x.exp();
        touched |= match side {
            Side::Down => s <= barrier,
            Side::Up => s >= barrier,
        };
        i += 1;
        if i < g.steps {
            x += g.mu_dt + g.sig_sqrt_dt * z2;
            let s = x.exp();
            touched |= match side {
                Side::Down => s <= barrier,
                Side::Up => s >= barrier,
            };
            i += 1;
        }
    }
    (x.exp(), touched)
}

/// Barrier monitoring in log space: the barrier's logarithm is taken once
/// and the comparison happens on the accumulator, so the path costs one
/// exponential in total.
///
/// With `early_exit` the loop stops at the first touch. That saves the rest
/// of the path for a knocked-out contract and adds a branch that the
/// accumulate-and-continue form does not have, which is the tradeoff the
/// measurement is for.
#[inline(always)]
pub fn barrier_log_space(
    g: &Gbm,
    rng: &mut Rng,
    barrier: f64,
    side: Side,
    early_exit: bool,
) -> (f64, bool, u32) {
    let b = barrier.ln();
    let mut x = g.s0.ln();
    let mut touched = false;
    let mut seen = 0u32;
    let mut i = 0;
    while i < g.steps {
        let (z1, z2) = rng.next_normal_pair();
        x += g.mu_dt + g.sig_sqrt_dt * z1;
        seen += 1;
        touched |= match side {
            Side::Down => x <= b,
            Side::Up => x >= b,
        };
        i += 1;
        if early_exit && touched {
            break;
        }
        if i < g.steps {
            x += g.mu_dt + g.sig_sqrt_dt * z2;
            seen += 1;
            touched |= match side {
                Side::Down => x <= b,
                Side::Up => x >= b,
            };
            i += 1;
            if early_exit && touched {
                break;
            }
        }
    }
    (x.exp(), touched, seen)
}

/// Every statistic from rolling state, holding nothing but scalars.
#[inline(always)]
pub fn rolling(g: &Gbm, rng: &mut Rng, barrier: f64, side: Side) -> Stats {
    debug_assert!(g.steps > 0);
    let b = barrier.ln();
    let mut x = g.s0.ln();
    // Unset, so the first observation sets both. Seeding these with the spot
    // would put time zero in the extremes but not in the averages.
    let (mut hi, mut lo) = (f64::NEG_INFINITY, f64::INFINITY);
    let (mut sum, mut log_sum) = (0.0, 0.0);
    let mut touched = false;
    let mut seen = 0u32;
    let mut i = 0;
    while i < g.steps {
        let (z1, z2) = rng.next_normal_pair();
        for z in [z1, z2] {
            if i >= g.steps {
                break;
            }
            x += g.mu_dt + g.sig_sqrt_dt * z;
            sum += x.exp();
            log_sum += x;
            hi = hi.max(x);
            lo = lo.min(x);
            touched |= match side {
                Side::Down => x <= b,
                Side::Up => x >= b,
            };
            seen += 1;
            i += 1;
        }
    }
    Stats {
        terminal: x.exp(),
        sum,
        log_sum,
        max: hi.exp(),
        min: lo.exp(),
        touched,
        observed: seen,
    }
}

/// The same statistics, but with every observation written to `out` first
/// and the statistics computed in a second pass over it.
///
/// This is the shape the brief this module answers warns against, and the
/// reason to implement it is to measure what it costs: `out` must hold one
/// double per step, so a 250-step path touches 2 KiB that the rolling form
/// keeps in registers. Returns `None` if `out` is too short to hold the
/// path, rather than silently measuring a shorter one.
pub fn stored(
    g: &Gbm,
    rng: &mut Rng,
    barrier: f64,
    side: Side,
    out: &mut [f64],
) -> Option<Stats> {
    if out.len() < g.steps {
        return None;
    }
    let mut x = g.s0.ln();
    let mut i = 0;
    while i < g.steps {
        let (z1, z2) = rng.next_normal_pair();
        for z in [z1, z2] {
            if i >= g.steps {
                break;
            }
            x += g.mu_dt + g.sig_sqrt_dt * z;
            out[i] = x;
            i += 1;
        }
    }
    let b = barrier.ln();
    let (mut hi, mut lo) = (out[0], out[0]);
    let (mut sum, mut log_sum) = (0.0, 0.0);
    let mut touched = false;
    for &v in &out[..g.steps] {
        sum += v.exp();
        log_sum += v;
        hi = hi.max(v);
        lo = lo.min(v);
        touched |= match side {
            Side::Down => v <= b,
            Side::Up => v >= b,
        };
    }
    Some(Stats {
        terminal: out[g.steps - 1].exp(),
        sum,
        log_sum,
        max: hi.exp(),
        min: lo.exp(),
        touched,
        observed: g.steps as u32,
    })
}

/// Discounted mean and standard error of a sample of payoffs.
///
/// The standard error is what makes a Monte Carlo price checkable against a
/// closed form: without it, agreement and disagreement look the same.
#[derive(Clone, Copy, Debug)]
pub struct Estimate {
    pub price: f64,
    pub std_err: f64,
    pub paths: usize,
}

/// Accumulates payoffs in one pass, keeping a running mean and a running
/// sum of squared deviations (Welford), so the variance does not come from
/// differencing two large sums.
#[derive(Clone, Copy, Debug, Default)]
pub struct Accum {
    n: usize,
    mean: f64,
    m2: f64,
}

impl Accum {
    #[inline(always)]
    pub fn push(&mut self, x: f64) {
        self.n += 1;
        let d = x - self.mean;
        self.mean += d / self.n as f64;
        self.m2 += d * (x - self.mean);
    }

    pub fn finish(&self, discount: f64) -> Estimate {
        let var = if self.n > 1 {
            self.m2 / (self.n - 1) as f64
        } else {
            0.0
        };
        Estimate {
            price: discount * self.mean,
            std_err: discount * (var / self.n.max(1) as f64).sqrt(),
            paths: self.n,
        }
    }
}

/// The exact price of a geometric Asian call averaged over `n` discrete
/// observation times, the contract the arms above actually simulate.
///
/// The geometric average of a lognormal path is itself lognormal, so this
/// contract has a closed form. Averaging the log price at `t_i = i T / n`
/// for `i = 1..n` gives a normal with
///
/// ```text
///   mean     = ln S0 + (r - q - sigma^2/2) * T (n+1) / (2n)
///   variance = sigma^2 T (n+1)(2n+1) / (6 n^2)
/// ```
///
/// using `sum_{i,j} min(i,j) = n(n+1)(2n+1)/6`, after which the price is the
/// standard lognormal call expectation. As `n` grows the variance tends to
/// `sigma^2 T / 3` and the mean factor to one half, which is the continuous
/// result of Kemna and Vorst (Journal of Banking and Finance 14, 1990); the
/// tests check that limit as well as the discrete value.
///
/// Writing the discrete form rather than the continuous one matters: at 250
/// observations the two differ by 0.6 percent in variance, which enough
/// simulated paths would report as a failure of a correct engine.
///
/// This exists only as a test oracle. No arm above calls it, and the tests
/// check it against an independent simulation of the same contract, so an
/// error in either would appear as a disagreement far outside the
/// simulation's standard error.
pub fn geometric_asian_call(
    s0: f64,
    k: f64,
    t: f64,
    r: f64,
    q: f64,
    sigma: f64,
    n: usize,
) -> f64 {
    use crate::pricing::normal_cdf;
    let nf = n as f64;
    let v = sigma * sigma * t * (nf + 1.0) * (2.0 * nf + 1.0) / (6.0 * nf * nf);
    let m = s0.ln() + (r - q - 0.5 * sigma * sigma) * t * (nf + 1.0) / (2.0 * nf);
    let sd = v.sqrt();
    let d2 = (m - k.ln()) / sd;
    let d1 = d2 + sd;
    (-r * t).exp() * ((m + 0.5 * v).exp() * normal_cdf(d1) - k * normal_cdf(d2))
}
