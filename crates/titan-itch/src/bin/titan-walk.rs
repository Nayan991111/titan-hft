//! What immediacy costs, from the depth a real venue actually displays.
//!
//! The fourth source brief states that price impact grows faster than the
//! size traded, writing it as a change in price equal to a constant times
//! size raised to a power above one, and it works an example in which a
//! large order is filled by walking down a tidy ladder of depth. Neither the
//! power nor the ladder is measured anywhere in the brief.
//!
//! Both are measurable from a real feed without any model. At every moment
//! the book holds the orders that are actually resting, so the cost of
//! filling a given size immediately is the sum over levels of price times
//! the shares taken there, until the size is covered or the side runs out.
//! Dividing by the size gives the average price paid, and the distance from
//! the best price is the cost of immediacy, reported in basis points so that
//! instruments of different price are comparable.
//!
//! This measures the instantaneous cost of consuming displayed depth. It is
//! not the realised impact of a trade, which would need a model of what the
//! price would have done otherwise, and this feed cannot identify that. The
//! brief's claim is tested in the form the data can answer: whether the cost
//! per share rises with the size being filled.
//!
//! Designs for the timed pass: `none`, which decodes and keeps no book, as
//! the floor; `top`, the book and the best price on each side of the
//! instrument that changed; `walk_100`, `walk_1000` and `walk_10000`, the
//! same book asked instead what filling that many shares would cost.
//!
//!   titan-walk [file=PATH] [limit=N] [instruments=M] [log2=K] [epochs=E] [pmu]

use std::hint::black_box;
use titan_itch::book::L3Book;
use titan_itch::decode::{decode, Msg, Reader};
use titan_itch::gen::{feed, Mix};
use titan_itch::{Book, Side};
use titan_platform::alloc_count::{thread_allocations, CountingAlloc};
use titan_platform::clock;
use titan_platform::kperf::{events, Kperf, Session};
use titan_platform::qos::{self, Qos};

#[global_allocator]
static A: CountingAlloc = CountingAlloc;

const STAGES: [&str; 5] = ["none", "top", "walk_100", "walk_1000", "walk_10000"];

/// Sizes measured in the untimed pass, in shares. 100 is one round lot.
const SIZES: [u64; 5] = [100, 500, 1_000, 5_000, 10_000];

#[inline(always)]
fn size_of_stage(design: &str) -> u64 {
    match design {
        "walk_100" => 100,
        "walk_1000" => 1_000,
        _ => 10_000,
    }
}

#[inline(always)]
fn locate_of(m: &Msg) -> Option<u16> {
    match *m {
        Msg::Add { locate, .. }
        | Msg::Executed { locate, .. }
        | Msg::ExecutedWithPrice { locate, .. }
        | Msg::Cancel { locate, .. }
        | Msg::Delete { locate, .. }
        | Msg::Replace { locate, .. } => Some(locate),
        Msg::Other { .. } => None,
    }
}

/// One pass over the feed that decodes and does nothing else: the work every
/// design below shares, so they can be measured above it.
#[inline(always)]
fn run_none(src: &[u8], limit: u64) -> (u64, u64) {
    let (mut n, mut sum) = (0u64, 0u64);
    let mut r = Reader::new(src);
    while n < limit {
        let Some(Ok(b)) = r.next_raw() else { break };
        n += 1;
        let Ok(m) = decode(b) else { continue };
        if let Some(loc) = locate_of(&m) {
            sum = sum.rotate_left(7) ^ loc as u64;
        }
    }
    (n, sum)
}

/// One pass with the book, asking for the best price on each side of the
/// instrument that changed.
#[inline(always)]
fn run_top(src: &[u8], book: &mut L3Book, limit: u64) -> (u64, u64) {
    let (mut n, mut sum) = (0u64, 0u64);
    let mut r = Reader::new(src);
    while n < limit {
        let Some(Ok(b)) = r.next_raw() else { break };
        n += 1;
        let Ok(m) = decode(b) else { continue };
        book.apply(&m);
        if let Some(loc) = locate_of(&m) {
            for side in [Side::Buy, Side::Sell] {
                sum = match book.best(loc, side) {
                    Some((p, s)) => sum.rotate_left(7) ^ p as u64 ^ s,
                    None => sum.rotate_left(7) ^ 0xFFFF_FFFF,
                };
            }
        }
    }
    (n, sum)
}

/// One pass with the book, asking instead what filling `shares` would cost
/// on each side. The size is a parameter because a real caller supplies one.
#[inline(always)]
fn run_walk(src: &[u8], book: &mut L3Book, limit: u64, shares: u64) -> (u64, u64) {
    let (mut n, mut sum) = (0u64, 0u64);
    let mut r = Reader::new(src);
    while n < limit {
        let Some(Ok(b)) = r.next_raw() else { break };
        n += 1;
        let Ok(m) = decode(b) else { continue };
        book.apply(&m);
        if let Some(loc) = locate_of(&m) {
            for side in [Side::Buy, Side::Sell] {
                sum = match book.walk(loc, side, shares) {
                    Some((c, l, p)) => sum.rotate_left(7) ^ c ^ l as u64 ^ p as u64,
                    None => sum.rotate_left(7) ^ 0xFFFF_FFFF,
                };
            }
        }
    }
    (n, sum)
}

fn kv<'a>(args: &'a [String], k: &str) -> Option<&'a str> {
    args.iter().find_map(|a| a.strip_prefix(k).and_then(|r| r.strip_prefix('=')))
}

fn num<T: std::str::FromStr>(args: &[String], k: &str, default: T) -> T {
    kv(args, k).and_then(|v| v.parse().ok()).unwrap_or(default)
}

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let limit: u64 = num(&args, "limit", u64::MAX);
    let log2: u32 = num(&args, "log2", 23);
    let epochs: usize = num(&args, "epochs", 10);
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
    let (src, name) = match kv(&args, "file") {
        Some(path) => {
            let raw = std::fs::read(path).unwrap_or_else(|e| panic!("cannot read {path}: {e}"));
            let n = std::path::Path::new(path).file_name().and_then(|f| f.to_str()).unwrap_or("file").replace('.', "_");
            (raw, n)
        }
        None => {
            let instruments: u16 = num(&args, "instruments", 500);
            (feed(42, instruments, limit.min(5_000_000) as usize, Mix::default()), "synthetic_default".to_string())
        }
    };

    // Highest locate in the window, so the book is sized to the feed.
    let mut max_locate = 0u16;
    let mut messages = 0u64;
    let mut r = Reader::new(&src);
    while messages < limit {
        let Some(Ok(b)) = r.next_raw() else { break };
        messages += 1;
        if b.len() >= 3 {
            max_locate = max_locate.max(u16::from_be_bytes([b[1], b[2]]));
        }
    }
    let instruments = max_locate as usize + 1;

    // Untimed: the cost of immediacy at each size, both sides, measured
    // whenever the instrument that changed has a two sided book.
    let mut book = L3Book::new(log2, instruments);
    let mut quoted = [0u64; SIZES.len()];
    let mut unfillable = [0u64; SIZES.len()];
    let mut bps_sum = [0f64; SIZES.len()];
    let mut move_sum = [0f64; SIZES.len()];
    let mut levels_sum = [0u64; SIZES.len()];
    let mut worst_bps = [0f64; SIZES.len()];
    let (mut opportunities, mut one_sided) = (0u64, 0u64);
    let mut r = Reader::new(&src);
    let mut n = 0u64;
    while n < limit {
        let Some(Ok(b)) = r.next_raw() else { break };
        n += 1;
        let Ok(m) = decode(b) else { continue };
        book.apply(&m);
        let Some(loc) = locate_of(&m) else { continue };
        for side in [Side::Buy, Side::Sell] {
            let Some((best, _)) = book.best(loc, side) else {
                one_sided += 1;
                continue;
            };
            opportunities += 1;
            for (i, &size) in SIZES.iter().enumerate() {
                match book.walk(loc, side, size) {
                    Some((cost, levels, last)) => {
                        // Two distances from the best price, both in basis
                        // points of it and positive on either side, since a
                        // buyer pays up and a seller sells down. The first is
                        // what the size costs on average, which is what the
                        // trader pays. The second is how far the quote has to
                        // move to fill it, which is what the brief calls the
                        // change in price.
                        let avg = cost as f64 / size as f64;
                        let bps = 10_000.0 * (avg - best as f64).abs() / best as f64;
                        let moved = 10_000.0 * (last as f64 - best as f64).abs() / best as f64;
                        quoted[i] += 1;
                        bps_sum[i] += bps;
                        move_sum[i] += moved;
                        levels_sum[i] += levels as u64;
                        if bps > worst_bps[i] {
                            worst_bps[i] = bps;
                        }
                    }
                    None => unfillable[i] += 1,
                }
            }
        }
    }
    println!(
        "walk_config feed={name} messages={n} instruments={instruments} log2={log2} limit={limit} epochs={epochs} pmu={} sizes={} quotable_sides={opportunities} empty_sides={one_sided} unknown={}",
        pmu.is_some(),
        SIZES.iter().map(|s| s.to_string()).collect::<Vec<_>>().join(","),
        book.unknown,
    );
    let join = |f: &dyn Fn(usize) -> String| (0..SIZES.len()).map(f).collect::<Vec<_>>().join(",");
    println!(
        "walk_cost feed={name} sizes={} fillable={} unfillable={} unfillable_rate={} bps_mean={} bps_per_100_shares={} move_bps_mean={} move_bps_per_100_shares={} levels_mean={} bps_max={}",
        SIZES.iter().map(|s| s.to_string()).collect::<Vec<_>>().join(","),
        join(&|i| quoted[i].to_string()),
        join(&|i| unfillable[i].to_string()),
        join(&|i| format!("{:.4}", unfillable[i] as f64 / (quoted[i] + unfillable[i]).max(1) as f64)),
        join(&|i| format!("{:.4}", bps_sum[i] / quoted[i].max(1) as f64)),
        // Each cost scaled to a common size, so a rising series is the
        // superlinear growth the brief asserts and a falling one is not.
        join(&|i| format!("{:.6}", 100.0 * bps_sum[i] / quoted[i].max(1) as f64 / SIZES[i] as f64)),
        join(&|i| format!("{:.4}", move_sum[i] / quoted[i].max(1) as f64)),
        join(&|i| format!("{:.6}", 100.0 * move_sum[i] / quoted[i].max(1) as f64 / SIZES[i] as f64)),
        join(&|i| format!("{:.3}", levels_sum[i] as f64 / quoted[i].max(1) as f64)),
        join(&|i| format!("{:.2}", worst_bps[i])),
    );

    let freq = clock::counter_frequency() as f64;
    let run = |design: &str, pmu: Option<&Session>, book: &mut L3Book| {
        if design != "none" {
            book.clear();
        }
        let a0 = thread_allocations();
        let p0 = pmu.map(|s| s.read());
        let t0 = clock::counter();
        let (msgs, sum) = match design {
            "none" => run_none(&src, limit),
            "top" => run_top(&src, book, limit),
            _ => run_walk(&src, book, limit, size_of_stage(design)),
        };
        let t1 = clock::counter();
        let d = match (pmu, p0) {
            (Some(p), Some(x)) => {
                let y = p.read();
                Some(p.delta(&x, &y))
            }
            _ => None,
        };
        let allocs = thread_allocations() - a0 - d.is_some() as u64;
        (black_box(sum), msgs, t1 - t0, d, allocs)
    };
    for design in STAGES {
        run(design, pmu.as_ref(), &mut book);
    }
    for epoch in 0..epochs {
        for j in 0..STAGES.len() {
            let design = STAGES[(j + epoch) % STAGES.len()];
            let (sum, msgs, ticks, d, allocs) = run(design, pmu.as_ref(), &mut book);
            let secs = ticks as f64 / freq;
            let mut line = format!(
                "walk feed={name} design={design} epoch={epoch} messages={msgs} ns_per_msg={:.3} allocations_pass={allocs} checksum={sum:016x}",
                secs * 1e9 / msgs as f64,
            );
            if let Some(d) = d {
                let per = |x: u64| x as f64 / msgs as f64;
                line += &format!(
                    " msg_cycles={:.3} msg_instructions={:.3} msg_l1d_miss={:.4} msg_br_mispred={:.4} ipc={:.3} core_ghz={:.3}",
                    per(d[0]),
                    per(d[1]),
                    per(d[2]),
                    per(d[3]),
                    d[1] as f64 / d[0] as f64,
                    d[0] as f64 / secs / 1e9,
                );
            }
            println!("{line}");
        }
    }
}
