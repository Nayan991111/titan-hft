//! What a pricing kernel costs, and where the cost goes.
//!
//! The trading path measured elsewhere in this work ends in a decision that
//! costs 22 cycles of toxicity update and 21 of pre-trade risk checks. An
//! options market maker's decision is a pricing kernel instead, so the
//! question of whether that path is representative reduces to a measurement:
//! how many cycles one contract's price and five sensitivities take, and
//! whether that fits inside the per-message budget of the feed.
//!
//! Seven arms, each processing the same number of contracts so every figure
//! is per contract and directly comparable:
//!
//! * `cdf`       the cumulative normal alone, to size the transcendental
//! * `price`     price with no sensitivities
//! * `fused`     price and all five sensitivities, shared terms computed once
//! * `separate`  the same six numbers from six independent functions
//! * `f32`       the fused kernel in single precision
//! * `chain`     one expiry's strike ladder, strike-independent terms hoisted
//! * `loop`      the same ladder one contract at a time, for comparison
//!
//! Two strike ladders, because the cumulative normal branches on the sign of
//! its argument. `straddle` crosses the spot, so `d1` changes sign along the
//! ladder and the branch is unpredictable; `otm` stays far out of the money,
//! so it does not. The difference between them is the cost of that branch.
//!
//!   titan-greeks [epochs] [ui|bg] [pmu]
//!
//! `ui` requests QoS USER_INTERACTIVE (performance-core preference, default);
//! `bg` requests QoS BACKGROUND, which XNU confines to efficiency cores.
//! `pmu` adds cycle, instruction, L1D-miss and branch-mispredict counters on
//! the measuring thread and must run under sudo.

use titan_core::pricing::{
    chain, greeks, greeks_f32, greeks_separate, normal_cdf, price, Contract, Greeks, Kind,
};
use titan_platform::clock;
use titan_platform::kperf::{events, Kperf, Session};
use titan_platform::qos::{self, Qos};

/// Contracts priced per epoch, per arm.
const CONTRACTS: usize = 4096;
/// Strikes in one `chain` call. A quoted ladder of this depth is ordinary.
const CHAIN: usize = 64;

const ARMS: [&str; 7] = ["cdf", "price", "fused", "separate", "f32", "chain", "loop"];
const LADDERS: [&str; 2] = ["straddle", "otm"];

/// The base contract. Fixed, not random: the question is what the kernel
/// costs, and a varying spot would add a source of difference that is not
/// the thing being measured.
const BASE: Contract = Contract {
    s: 100.0,
    k: 100.0,
    t: 0.25,
    r: 0.04,
    q: 0.01,
    sigma: 0.22,
};

/// `CONTRACTS` strikes, deterministic.
///
/// `straddle` sweeps half the spot to one and a half times it, so `d1`
/// crosses zero inside every window of `CHAIN`. `otm` sweeps twice the spot
/// to three times it, where `d1` is negative throughout.
fn ladder(kind: &str) -> Vec<f64> {
    let (lo, hi) = match kind {
        "straddle" => (0.5, 1.5),
        _ => (2.0, 3.0),
    };
    (0..CONTRACTS)
        .map(|i| {
            let f = (i % CHAIN) as f64 / (CHAIN - 1) as f64;
            BASE.s * (lo + (hi - lo) * f)
        })
        .collect()
}

#[inline(always)]
fn fold(acc: &mut f64, g: &Greeks) {
    *acc += g.price + g.delta + g.gamma + g.vega + g.theta + g.rho;
}

/// Runs one arm once over `strikes` and returns an accumulator that depends
/// on every result, so none of the work can be elided.
fn run(arm: &str, strikes: &[f64], out: &mut [Greeks]) -> f64 {
    let base = std::hint::black_box(BASE);
    let kind = std::hint::black_box(Kind::Call);
    let mut acc = 0.0_f64;
    match arm {
        "cdf" => {
            // Arguments spread over the range the kernel actually uses, and
            // crossing zero, so this arm carries the same branch as the rest.
            for (i, _) in strikes.iter().enumerate() {
                let x = -4.0 + 8.0 * (i % CHAIN) as f64 / (CHAIN - 1) as f64;
                acc += normal_cdf(std::hint::black_box(x));
            }
        }
        "price" => {
            for &k in strikes {
                acc += price(&Contract { k, ..base }, kind);
            }
        }
        "fused" => {
            for &k in strikes {
                fold(&mut acc, &greeks(&Contract { k, ..base }, kind));
            }
        }
        "separate" => {
            for &k in strikes {
                fold(&mut acc, &greeks_separate(&Contract { k, ..base }, kind));
            }
        }
        "f32" => {
            for &k in strikes {
                fold(&mut acc, &greeks_f32(&Contract { k, ..base }, kind));
            }
        }
        "chain" => {
            for w in strikes.chunks(CHAIN) {
                let n = chain(&base, kind, w, out);
                for g in &out[..n] {
                    fold(&mut acc, g);
                }
            }
        }
        _ => {
            for &k in strikes {
                fold(&mut acc, &greeks(&Contract { k, ..base }, kind));
            }
        }
    }
    acc
}

struct Cfg {
    pmu: Option<Session>,
    freq: f64,
}

fn main() {
    let a: Vec<String> = std::env::args().skip(1).collect();
    let epochs: usize = a.first().and_then(|x| x.parse().ok()).unwrap_or(3);
    let qos_name = a.iter().find(|x| *x == "bg").map(|_| "bg").unwrap_or("ui");
    let want_pmu = a.iter().any(|x| x == "pmu");

    let q = if qos_name == "bg" {
        Qos::Background
    } else {
        Qos::UserInteractive
    };
    if let Err(e) = qos::set_current_thread(q) {
        eprintln!("qos {qos_name} unavailable: {e}");
    }

    let pmu = if want_pmu {
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
    let cfg = Cfg {
        pmu,
        freq: clock::counter_frequency() as f64,
    };

    println!(
        "greeks_config contracts={} chain={} epochs={} qos={} pmu={} counter_hz={} \
         s={} t={} r={} q={} sigma={}",
        CONTRACTS,
        CHAIN,
        epochs,
        qos_name,
        cfg.pmu.is_some() as u8,
        clock::counter_frequency(),
        BASE.s,
        BASE.t,
        BASE.r,
        BASE.q,
        BASE.sigma
    );

    let mut out = vec![Greeks::default(); CHAIN];
    for ladder_name in LADDERS {
        let strikes = ladder(ladder_name);
        for arm in ARMS {
            // One untimed pass, so the first epoch does not pay for cold
            // instruction cache and cold branch predictors.
            std::hint::black_box(run(arm, &strikes, &mut out));

            for epoch in 0..epochs {
                let c0 = cfg.pmu.as_ref().map(|s| s.read());
                let t0 = clock::counter();
                let acc = run(arm, &strikes, &mut out);
                let t1 = clock::counter();
                std::hint::black_box(acc);

                let n = CONTRACTS as f64;
                let secs = (t1 - t0) as f64 / cfg.freq;
                let ns = secs * 1e9 / n;
                match (cfg.pmu.as_ref(), c0) {
                    (Some(s), Some(c0)) => {
                        let d = s.delta(&c0, &s.read());
                        let per = |x: u64| x as f64 / n;
                        println!(
                            "greeks_cost arm={arm} ladder={ladder_name} qos={qos_name} \
                             epoch={epoch} ns={ns:.4} cycles={:.3} instructions={:.3} \
                             l1d_miss={:.4} br_mispredict={:.4} core_ghz={:.3}",
                            per(d[0]),
                            per(d[1]),
                            per(d[2]),
                            per(d[3]),
                            d[0] as f64 / secs / 1e9
                        );
                    }
                    _ => {
                        println!(
                            "greeks_cost arm={arm} ladder={ladder_name} qos={qos_name} \
                             epoch={epoch} ns={ns:.4}"
                        );
                    }
                }
            }
        }
    }
}
