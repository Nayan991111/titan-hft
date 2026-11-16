//! What path dependence costs per step.
//!
//! Ten arms over the same simulated paths. Within one epoch every arm is
//! seeded identically, so the arms walk the same paths and differ only in
//! what they accumulate along them; across launches the launch is the unit
//! of observation, as in every run from H61 onward.
//!
//!   terminal           generate the path, keep the terminal price only
//!   asian_arith        running arithmetic average, one exponential per step
//!   asian_geom         running geometric average, additive in logarithms
//!   lookback_free      running extremes, branchless minimum and maximum
//!   lookback_branchy   the same extremes as explicit comparisons
//!   barrier_price      barrier monitored in price space, one exp per step
//!   barrier_log        barrier monitored in log space, none
//!   barrier_log_exit   the same, stopping at the first touch
//!   rolling            every statistic from scalars
//!   stored             every statistic from a stored path, in two passes
//!
//! The barrier arms run at two distances. `near` sits close enough to the
//! spot that a touch is roughly even odds, so the monitor's comparison is
//! unpredictable; `far` is distant enough that almost no path reaches it, so
//! the same comparison is perfectly predicted. The difference between them
//! is the cost of that branch.
//!
//!   titan-paths [epochs] [ui|bg] [pmu]
//!
//! `ui` requests QoS USER_INTERACTIVE (performance-core preference, the
//! default); `bg` requests BACKGROUND, which XNU confines to efficiency
//! cores. `pmu` adds cycle, instruction, L1D-miss and branch-mispredict
//! counters on the measuring thread and needs sudo.

use titan_core::paths::{
    asian_arithmetic, asian_geometric, barrier_log_space, barrier_price_space, lookback_branchless,
    lookback_branchy, rolling, stored, terminal, Gbm, Rng, Side,
};
use titan_platform::clock;
use titan_platform::kperf::{events, Kperf, Session};
use titan_platform::qos::{self, Qos};

/// Paths per arm per epoch.
const PATHS: usize = 2_000;
/// Observation times per path. A year of daily fixings.
const STEPS: usize = 250;
/// Base seed. Fixed, so a rerun walks the same paths.
const SEED: u64 = 0x7A1_0BE1;

const ARMS: [&str; 10] = [
    "terminal",
    "asian_arith",
    "asian_geom",
    "lookback_free",
    "lookback_branchy",
    "barrier_price",
    "barrier_log",
    "barrier_log_exit",
    "rolling",
    "stored",
];

/// True for the arms whose cost depends on where the barrier sits.
fn barrier_sensitive(arm: &str) -> bool {
    arm.starts_with("barrier") || arm == "rolling" || arm == "stored"
}

/// Runs one arm over `PATHS` paths and returns an accumulator that depends
/// on every result, so none of the work can be elided, and the number of
/// observations actually taken.
fn run(arm: &str, g: &Gbm, seed: u64, barrier: f64, buf: &mut [f64]) -> (f64, u64) {
    let mut rng = Rng::new(seed);
    let side = Side::Down;
    let mut acc = 0.0_f64;
    let mut seen = 0u64;
    let nominal = g.steps as u64;
    match arm {
        "terminal" => {
            for _ in 0..PATHS {
                acc += terminal(g, &mut rng);
                seen += nominal;
            }
        }
        "asian_arith" => {
            for _ in 0..PATHS {
                let (s, a) = asian_arithmetic(g, &mut rng);
                acc += s + a;
                seen += nominal;
            }
        }
        "asian_geom" => {
            for _ in 0..PATHS {
                let (s, a) = asian_geometric(g, &mut rng);
                acc += s + a;
                seen += nominal;
            }
        }
        "lookback_free" => {
            for _ in 0..PATHS {
                let (s, hi, lo) = lookback_branchless(g, &mut rng);
                acc += s + hi + lo;
                seen += nominal;
            }
        }
        "lookback_branchy" => {
            for _ in 0..PATHS {
                let (s, hi, lo) = lookback_branchy(g, &mut rng);
                acc += s + hi + lo;
                seen += nominal;
            }
        }
        "barrier_price" => {
            for _ in 0..PATHS {
                let (s, t) = barrier_price_space(g, &mut rng, barrier, side);
                acc += s + t as u8 as f64;
                seen += nominal;
            }
        }
        "barrier_log" => {
            for _ in 0..PATHS {
                let (s, t, n) = barrier_log_space(g, &mut rng, barrier, side, false);
                acc += s + t as u8 as f64;
                seen += n as u64;
            }
        }
        "barrier_log_exit" => {
            for _ in 0..PATHS {
                let (s, t, n) = barrier_log_space(g, &mut rng, barrier, side, true);
                acc += s + t as u8 as f64;
                seen += n as u64;
            }
        }
        "rolling" => {
            for _ in 0..PATHS {
                let st = rolling(g, &mut rng, barrier, side);
                acc += st.terminal + st.sum + st.max + st.min + st.touched as u8 as f64;
                seen += st.observed as u64;
            }
        }
        _ => {
            for _ in 0..PATHS {
                let st = stored(g, &mut rng, barrier, side, buf).expect("buffer too short");
                acc += st.terminal + st.sum + st.max + st.min + st.touched as u8 as f64;
                seen += st.observed as u64;
            }
        }
    }
    (acc, seen)
}

fn main() {
    let a: Vec<String> = std::env::args().skip(1).collect();
    let epochs: usize = a.first().and_then(|x| x.parse().ok()).unwrap_or(3);
    let qos_name = if a.iter().any(|x| x == "bg") { "bg" } else { "ui" };
    let want_pmu = a.iter().any(|x| x == "pmu");

    let q = if qos_name == "bg" {
        Qos::Background
    } else {
        Qos::UserInteractive
    };
    if let Err(e) = qos::set_current_thread(q) {
        eprintln!("qos {qos_name} unavailable: {e}");
    }

    let pmu: Option<Session> = if want_pmu {
        match Kperf::open().and_then(|k| {
            k.session(&[
                events::CYCLES,
                events::INSTRUCTIONS,
                events::L1D_LOAD_MISS,
                events::BRANCH_MISPREDICT,
            ])
        }) {
            Ok(s) => Some(s),
            Err(e) => {
                eprintln!("pmu unavailable: {e}");
                std::process::exit(2);
            }
        }
    } else {
        None
    };
    let freq = clock::counter_frequency() as f64;

    let (s0, t, r, q_div, sigma) = (100.0_f64, 1.0_f64, 0.03_f64, 0.0_f64, 0.30_f64);
    let g = Gbm::new(s0, t, r, q_div, sigma, STEPS);
    // `near` is placed so a touch is roughly even odds over the horizon, so
    // the monitor's comparison is unpredictable; `far` is beyond anything a
    // path reaches, so it is perfectly predicted.
    let near = s0 * 0.80;
    let far = s0 * 0.01;

    println!(
        "paths_config paths={PATHS} steps={STEPS} epochs={epochs} qos={qos_name} pmu={} \
         counter_hz={} s0={s0} t={t} r={r} q={q_div} sigma={sigma} near={near} far={far} seed={SEED:#x}",
        pmu.is_some() as u8,
        clock::counter_frequency()
    );

    let mut buf = vec![0.0_f64; STEPS];
    for epoch in 0..epochs {
        // Every arm in this epoch walks the same paths.
        let seed = SEED ^ (epoch as u64).wrapping_mul(0x9E37_79B9_7F4A_7C15);
        for arm in ARMS {
            let dists: &[(&str, f64)] = if barrier_sensitive(arm) {
                &[("near", f64::NAN), ("far", f64::NAN)]
            } else {
                &[("na", f64::NAN)]
            };
            for (dist_name, _) in dists {
                let barrier = match *dist_name {
                    "near" => near,
                    "far" => far,
                    _ => far,
                };
                // One untimed pass, so the first epoch does not pay for cold
                // instruction cache and cold branch predictors.
                std::hint::black_box(run(arm, &g, seed, barrier, &mut buf));

                let c0 = pmu.as_ref().map(|s| s.read());
                let t0 = clock::counter();
                let (acc, seen) = run(arm, &g, seed, barrier, &mut buf);
                let t1 = clock::counter();
                std::hint::black_box(acc);

                let paths = PATHS as f64;
                let nominal = (PATHS * STEPS) as f64;
                let secs = (t1 - t0) as f64 / freq;
                let per_path = secs * 1e9 / paths;
                let per_step = secs * 1e9 / nominal;
                let observed = seen as f64 / paths;
                match (pmu.as_ref(), c0) {
                    (Some(s), Some(c0)) => {
                        let d = s.delta(&c0, &s.read());
                        let pp = |x: u64| x as f64 / paths;
                        let ps = |x: u64| x as f64 / nominal;
                        println!(
                            "paths_cost arm={arm} dist={dist_name} qos={qos_name} epoch={epoch} \
                             ns_path={per_path:.2} ns_step={per_step:.4} \
                             cyc_path={:.2} cyc_step={:.4} instr_step={:.4} \
                             l1d_step={:.5} brmis_step={:.5} observed={observed:.2} \
                             core_ghz={:.3}",
                            pp(d[0]),
                            ps(d[0]),
                            ps(d[1]),
                            ps(d[2]),
                            ps(d[3]),
                            d[0] as f64 / secs / 1e9
                        );
                    }
                    _ => {
                        println!(
                            "paths_cost arm={arm} dist={dist_name} qos={qos_name} epoch={epoch} \
                             ns_path={per_path:.2} ns_step={per_step:.4} observed={observed:.2}"
                        );
                    }
                }
            }
        }
    }
}
