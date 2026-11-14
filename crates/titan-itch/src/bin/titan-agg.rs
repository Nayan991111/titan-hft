//! One exposure limit shared by several strategy threads: what the pre-trade
//! check and reservation costs when every strategy updates the same total.
//!
//! S strategy threads (the main thread is strategy 0; QoS USER_INTERACTIVE)
//! each run the same loop: take the next order's notional, check it against
//! the exposure limit and reserve it, keep it open for `window` orders, then
//! release it. Designs:
//!
//!   local  each strategy checks and updates only its own exposure (no
//!          aggregation across strategies): the floor
//!   faa    one shared total; check with a load, reserve with `fetch_add`
//!          (check and reserve are two steps, so concurrent reservations can
//!          pass the limit together)
//!   cas    one shared total; check and reserve in one `compare_exchange`
//!          loop, so the limit is never exceeded; retries are counted
//!   shard  one total per strategy, each alone in a 128-byte line; the check
//!          sums all S totals, the reservation updates the strategy's own
//!          (single writer per total: load and store, no read-modify-write).
//!          The sum of S separate loads is not a snapshot, so, as with `faa`,
//!          concurrent reservations can pass the limit together
//!
//! Releases use `fetch_sub` (faa, cas) or the strategy's own total (local,
//! shard). All accesses are `Relaxed`: the totals publish no other data.
//!
//! Notionals come from a fixed pseudo-random sequence (seeded per strategy):
//! the cost of these instructions does not depend on the values, and the
//! limit is set so it never binds (the harness aborts if any order is
//! rejected), so every order is accepted in every design.
//!
//! Strategy 0 runs `ops` orders, measured with the generic counter and, with
//! `pmu`, its own core cycles, instructions, L1D load misses and branch
//! mispredicts. Strategies 1 to S-1 run the same loop from before strategy
//! 0 starts until it is done. They may still be descheduled or run on an
//! efficiency core, so their order counts are reported, and an epoch in
//! which any of them completed fewer than ops/100 orders is marked
//! `starved=1`. Checks: every strategy drains its window at the end, so
//! every total must end at zero; no order may be rejected.
//!
//! `cas_retries_per_order`: failed compare-and-swap attempts of all
//! strategies over the whole run divided by all their orders;
//! `cas_retries_s0`: strategy 0's failed attempts per measured order.
//!
//!   titan-agg [configs=local1,faa1,...] [ops=N] [window=W] [epochs=E] [pmu]
//!
//! Configs: `localS`, `faaS`, `casS`, `shardS` with S = 1 to 8 strategies.
//! Default: local1,faa1,cas1,shard1,local2,faa2,cas2,shard2,local3,faa3,cas3,shard3.

use std::sync::atomic::{AtomicBool, AtomicI64, AtomicUsize, Ordering};
use std::thread;
use titan_platform::clock;
use titan_platform::kperf::{events, Kperf, Session};
use titan_platform::qos::{self, Qos};
use titan_platform::rusage::Usage;

const LOCAL: u8 = 0;
const FAA: u8 = 1;
const CAS: u8 = 2;
const SHARD: u8 = 3;
const MAX_STRATEGIES: usize = 8;
/// Largest window of open orders per strategy.
const MAX_WINDOW: usize = 64;

/// A strategy's open orders, in their own 128-byte lines.
type Open = Line<[i64; MAX_WINDOW]>;

/// A value alone in 128-byte lines (the line size of Apple silicon).
#[repr(C, align(128))]
#[derive(Default)]
struct Line<T>(T);

/// Shared state of one run. Every field the strategies touch sits in its own
/// line, so the only sharing is the sharing the design asks for.
struct Shared {
    total: Line<AtomicI64>,
    shards: Vec<Line<AtomicI64>>,
    start: Line<AtomicUsize>,
    done: Line<AtomicBool>,
    limit: i64,
}

/// What one strategy did.
#[derive(Default, Clone, Copy)]
struct Did {
    orders: u64,
    rejected: u64,
    retries: u64,
}

#[inline(always)]
fn next(x: &mut u64) -> u64 {
    // xorshift64*
    *x ^= *x >> 12;
    *x ^= *x << 25;
    *x ^= *x >> 27;
    x.wrapping_mul(0x2545_f491_4f6c_dd1d)
}

/// Notional of the next order, 1 to 1,000,000 units.
#[inline(always)]
fn notional(x: &mut u64) -> i64 {
    (next(x) % 1_000_000) as i64 + 1
}

/// Check and reserve `n` for strategy `me`; false if the limit would be
/// passed (not reserved).
#[inline(always)]
fn reserve<const D: u8>(sh: &Shared, me: usize, n: i64, did: &mut Did) -> bool {
    match D {
        LOCAL => {
            let own = &sh.shards[me].0;
            let v = own.load(Ordering::Relaxed);
            if v + n > sh.limit {
                return false;
            }
            own.store(v + n, Ordering::Relaxed);
            true
        }
        FAA => {
            if sh.total.0.load(Ordering::Relaxed) + n > sh.limit {
                return false;
            }
            sh.total.0.fetch_add(n, Ordering::Relaxed);
            true
        }
        CAS => {
            let mut v = sh.total.0.load(Ordering::Relaxed);
            loop {
                if v + n > sh.limit {
                    return false;
                }
                match sh.total.0.compare_exchange_weak(v, v + n, Ordering::Relaxed, Ordering::Relaxed) {
                    Ok(_) => return true,
                    Err(now) => {
                        did.retries += 1;
                        v = now;
                    }
                }
            }
        }
        _ => {
            let mut sum = 0i64;
            for s in &sh.shards {
                sum += s.0.load(Ordering::Relaxed);
            }
            if sum + n > sh.limit {
                return false;
            }
            let own = &sh.shards[me].0;
            own.store(own.load(Ordering::Relaxed) + n, Ordering::Relaxed);
            true
        }
    }
}

#[inline(always)]
fn release<const D: u8>(sh: &Shared, me: usize, n: i64) {
    match D {
        FAA | CAS => {
            sh.total.0.fetch_sub(n, Ordering::Relaxed);
        }
        _ => {
            let own = &sh.shards[me].0;
            own.store(own.load(Ordering::Relaxed) - n, Ordering::Relaxed);
        }
    }
}

/// One order: reserve its notional, and release the order opened `window`
/// orders ago.
#[inline(always)]
fn order<const D: u8>(sh: &Shared, me: usize, open: &mut [i64], i: u64, rng: &mut u64, did: &mut Did) {
    let slot = (i as usize) & (open.len() - 1);
    if open[slot] != 0 {
        release::<D>(sh, me, open[slot]);
        open[slot] = 0;
    }
    let n = notional(rng);
    if reserve::<D>(sh, me, n, did) {
        open[slot] = n;
    } else {
        did.rejected += 1;
    }
    did.orders += 1;
}

fn drain<const D: u8>(sh: &Shared, me: usize, open: &mut [i64]) {
    for o in open.iter_mut() {
        if *o != 0 {
            release::<D>(sh, me, *o);
            *o = 0;
        }
    }
}

/// The measured loop of strategy 0: `ops` orders. Inlined into one named
/// function per design (below), so every loop has a readable symbol under
/// either name-mangling scheme.
#[inline(always)]
fn measured<const D: u8>(sh: &Shared, open: &mut [i64], ops: u64, rng: &mut u64) -> Did {
    let mut did = Did::default();
    for i in 0..ops {
        order::<D>(sh, 0, open, i, rng, &mut did);
    }
    did
}

/// The loop of strategies 1 to S-1: orders until strategy 0 is done.
#[inline(always)]
fn contend<const D: u8>(sh: &Shared, me: usize, window: usize, seed: u64) -> Did {
    let _ = qos::set_current_thread(Qos::UserInteractive);
    let mut open_box: Box<Open> = Box::new(Line([0; MAX_WINDOW]));
    let open = &mut open_box.0[..window];
    let mut rng = seed;
    let mut did = Did::default();
    sh.start.0.fetch_add(1, Ordering::Release);
    let mut i = 0u64;
    // Checking the flag every 64 orders keeps its line out of the hot path.
    while !sh.done.0.load(Ordering::Acquire) {
        for _ in 0..64 {
            order::<D>(sh, me, open, i, &mut rng, &mut did);
            i += 1;
        }
    }
    drain::<D>(sh, me, open);
    did
}

#[inline(never)]
fn measured_local(sh: &Shared, open: &mut [i64], ops: u64, rng: &mut u64) -> Did {
    measured::<LOCAL>(sh, open, ops, rng)
}
#[inline(never)]
fn measured_faa(sh: &Shared, open: &mut [i64], ops: u64, rng: &mut u64) -> Did {
    measured::<FAA>(sh, open, ops, rng)
}
#[inline(never)]
fn measured_cas(sh: &Shared, open: &mut [i64], ops: u64, rng: &mut u64) -> Did {
    measured::<CAS>(sh, open, ops, rng)
}
#[inline(never)]
fn measured_shard(sh: &Shared, open: &mut [i64], ops: u64, rng: &mut u64) -> Did {
    measured::<SHARD>(sh, open, ops, rng)
}
#[inline(never)]
fn contend_local(sh: &Shared, me: usize, window: usize, seed: u64) -> Did {
    contend::<LOCAL>(sh, me, window, seed)
}
#[inline(never)]
fn contend_faa(sh: &Shared, me: usize, window: usize, seed: u64) -> Did {
    contend::<FAA>(sh, me, window, seed)
}
#[inline(never)]
fn contend_cas(sh: &Shared, me: usize, window: usize, seed: u64) -> Did {
    contend::<CAS>(sh, me, window, seed)
}
#[inline(never)]
fn contend_shard(sh: &Shared, me: usize, window: usize, seed: u64) -> Did {
    contend::<SHARD>(sh, me, window, seed)
}

struct Run {
    did: Did,
    others: Vec<Did>,
    ticks: u64,
    pmu: [u64; 4],
    invol: i64,
    final_total: i64,
    final_shards: Vec<i64>,
}

fn run<const D: u8>(strategies: usize, ops: u64, window: usize, pmu: Option<&Session>, seed: u64) -> Run {
    let sh = Shared {
        total: Line(AtomicI64::new(0)),
        shards: (0..strategies).map(|_| Line(AtomicI64::new(0))).collect(),
        start: Line(AtomicUsize::new(0)),
        done: Line(AtomicBool::new(false)),
        // Far above the largest possible open exposure (strategies x window x
        // 1,000,000), so the limit never binds.
        limit: i64::MAX / 4,
    };
    let mut open_box: Box<Open> = Box::new(Line([0; MAX_WINDOW]));
    let open = &mut open_box.0[..window];
    let mut rng = seed;
    let u0 = Usage::now();
    let mut out = Run { did: Did::default(), others: Vec::new(), ticks: 0, pmu: [0; 4], invol: 0, final_total: 0, final_shards: Vec::new() };
    thread::scope(|s| {
        let hs: Vec<_> = (1..strategies)
            .map(|me| {
                let sh = &sh;
                let seed = seed.wrapping_add(me as u64 * 0x9e37_79b9_7f4a_7c15);
                s.spawn(move || match D {
                    LOCAL => contend_local(sh, me, window, seed),
                    FAA => contend_faa(sh, me, window, seed),
                    CAS => contend_cas(sh, me, window, seed),
                    _ => contend_shard(sh, me, window, seed),
                })
            })
            .collect();
        while sh.start.0.load(Ordering::Acquire) < strategies - 1 {
            std::hint::spin_loop();
        }
        let p0 = pmu.map(|s| s.read());
        let t0 = clock::counter();
        out.did = match D {
            LOCAL => measured_local(&sh, open, ops, &mut rng),
            FAA => measured_faa(&sh, open, ops, &mut rng),
            CAS => measured_cas(&sh, open, ops, &mut rng),
            _ => measured_shard(&sh, open, ops, &mut rng),
        };
        let t1 = clock::counter();
        if let (Some(s), Some(a)) = (pmu, p0) {
            let b = s.read();
            let d = s.delta(&a, &b);
            out.pmu = [d[0], d[1], d[2], d[3]];
        }
        out.ticks = t1 - t0;
        drain::<D>(&sh, 0, open);
        sh.done.0.store(true, Ordering::Release);
        out.others = hs.into_iter().map(|h| h.join().expect("strategy thread")).collect();
    });
    out.invol = Usage::now().since(&u0).involuntary_switches;
    out.final_total = sh.total.0.load(Ordering::Relaxed);
    out.final_shards = sh.shards.iter().map(|s| s.0.load(Ordering::Relaxed)).collect();
    out
}

fn verify(label: &str, r: &Run, ops: u64) {
    assert_eq!(r.did.orders, ops, "{label}: strategy 0 order count");
    assert_eq!(r.final_total, 0, "{label}: shared total does not return to zero");
    assert!(r.final_shards.iter().all(|&v| v == 0), "{label}: a per-strategy total does not return to zero");
    let rejected = r.did.rejected + r.others.iter().map(|d| d.rejected).sum::<u64>();
    assert_eq!(rejected, 0, "{label}: an order was rejected, the limit must never bind");
}

fn parse(w: &str) -> (u8, &'static str, usize) {
    let (d, name, rest) = if let Some(r) = w.strip_prefix("local") {
        (LOCAL, "local", r)
    } else if let Some(r) = w.strip_prefix("faa") {
        (FAA, "faa", r)
    } else if let Some(r) = w.strip_prefix("cas") {
        (CAS, "cas", r)
    } else if let Some(r) = w.strip_prefix("shard") {
        (SHARD, "shard", r)
    } else {
        panic!("unknown config {w}")
    };
    let s: usize = rest.parse().unwrap_or_else(|_| panic!("config {w}: strategy count missing"));
    assert!((1..=MAX_STRATEGIES).contains(&s), "config {w}: 1 to {MAX_STRATEGIES} strategies");
    (d, name, s)
}

fn dispatch(d: u8, s: usize, ops: u64, window: usize, pmu: Option<&Session>, seed: u64) -> Run {
    match d {
        LOCAL => run::<LOCAL>(s, ops, window, pmu, seed),
        FAA => run::<FAA>(s, ops, window, pmu, seed),
        CAS => run::<CAS>(s, ops, window, pmu, seed),
        _ => run::<SHARD>(s, ops, window, pmu, seed),
    }
}

fn kv<'a>(args: &'a [String], key: &str) -> Option<&'a str> {
    args.iter().find_map(|a| a.strip_prefix(key).and_then(|r| r.strip_prefix('=')))
}

fn num<T: std::str::FromStr>(args: &[String], key: &str, default: T) -> T {
    kv(args, key).and_then(|v| v.parse().ok()).unwrap_or(default)
}

fn main() {
    let args: Vec<String> = std::env::args().collect();
    let ops: u64 = num(&args, "ops", 5_000_000);
    let window: usize = num(&args, "window", 16);
    let epochs: usize = num(&args, "epochs", 10);
    assert!(window.is_power_of_two() && window <= MAX_WINDOW, "window must be a power of two, at most {MAX_WINDOW}");
    assert!(ops > 0);
    let configs: Vec<(u8, &str, usize)> = kv(&args, "configs")
        .unwrap_or("local1,faa1,cas1,shard1,local2,faa2,cas2,shard2,local3,faa3,cas3,shard3")
        .split(',')
        .map(parse)
        .collect();
    let pmu = if args.iter().any(|a| a == "pmu") {
        match Kperf::open().and_then(|kp| {
            kp.session(&[events::CYCLES, events::INSTRUCTIONS, events::L1D_LOAD_MISS, events::BRANCH_MISPREDICT])
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
    let _ = qos::set_current_thread(Qos::UserInteractive);
    println!(
        "agg_config ops={ops} window={window} epochs={epochs} pmu={} configs={}",
        pmu.is_some(),
        configs.iter().map(|&(_, n, s)| format!("{n}{s}")).collect::<Vec<_>>().join(",")
    );
    let freq = clock::counter_frequency() as f64;
    // One unmeasured pass of every config.
    for &(d, n, s) in &configs {
        verify(&format!("warm-up {n}{s}"), &dispatch(d, s, ops, window, pmu.as_ref(), 7), ops);
    }
    for epoch in 0..epochs {
        for j in 0..configs.len() {
            let (d, n, s) = configs[(j + epoch) % configs.len()];
            let r = dispatch(d, s, ops, window, pmu.as_ref(), 7 + epoch as u64);
            verify(&format!("epoch {epoch} {n}{s}"), &r, ops);
            let per = |x: u64| x as f64 / ops as f64;
            let secs = r.ticks as f64 / freq;
            let others: u64 = r.others.iter().map(|d| d.orders).sum();
            let min_other = r.others.iter().map(|d| d.orders).min().unwrap_or(0);
            let starved = s > 1 && min_other < ops / 100;
            let retries = r.did.retries + r.others.iter().map(|d| d.retries).sum::<u64>();
            let mut line = format!(
                "agg design={n} strategies={s} epoch={epoch} orders={ops} ns_per_order={:.3} other_orders={others} \
                 min_other_orders={min_other} starved={} cas_retries_per_order={:.5} cas_retries_s0={:.5} invol_switches={}",
                secs * 1e9 / ops as f64,
                starved as u8,
                retries as f64 / (ops + others).max(1) as f64,
                r.did.retries as f64 / ops as f64,
                r.invol,
            );
            if pmu.is_some() {
                line += &format!(
                    " msg_cycles={:.3} msg_instructions={:.3} msg_l1d_miss={:.4} msg_br_mispred={:.4} ipc={:.3} core_ghz={:.3}",
                    per(r.pmu[0]),
                    per(r.pmu[1]),
                    per(r.pmu[2]),
                    per(r.pmu[3]),
                    r.pmu[1] as f64 / r.pmu[0] as f64,
                    r.pmu[0] as f64 / secs / 1e9,
                );
            }
            println!("{line}");
        }
    }
}
