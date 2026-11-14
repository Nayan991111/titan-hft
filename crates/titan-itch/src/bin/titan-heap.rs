//! What the third source brief's order book costs against this one.
//!
//! The brief specifies a priority queue of orders per side, "ordered by
//! price (highest first), then time", with insertion and removal in
//! O(log n) and the best quote in O(1). `titan_itch::book::L3Book` instead
//! keeps a sorted array of aggregated price levels per instrument and side.
//! Both are driven by the same ITCH feed, and after every book message both
//! are asked what a market maker asks continuously: what is the best bid and
//! the best ask on the instrument that just changed.
//!
//! Designs: `none`, which decodes and keeps no book, as the floor every
//! other design is measured above; `array`, the sorted levels; `heap_lazy`, a heap whose deletes
//! leave the entry behind to be skipped at the top; `heap_indexed`, a heap
//! that tracks every order's position so a delete is a true O(log n)
//! removal. Workloads: `top`, the best price alone, and `depth`, the best
//! price and every share resting at it, which the brief's own success
//! criteria require ("Quote Depth: Shares available at best bid/ask"). The
//! array book answers both with one operation, since a level is already the
//! sum of its orders; the heaps must visit every order at the best price,
//! and the two workloads are reported separately for that reason.
//!
//! The untimed pass also measures what the brief calls fleeting quotes,
//! which it recommends posting "for extremely short duration
//! (milliseconds)": how long each order rests before it is cancelled or
//! fully executed, and the same for the orders that arrive at the best
//! price, where the brief's advice would bite.
//!
//! Untimed first: all three books are driven through the same messages and
//! required to report the same best price, and the heaps the same depth as
//! the array book, after every message. Timed: `epochs` passes per design
//! and workload, order rotated, with the PMU.
//!
//!   titan-heap [file=PATH] [limit=N] [instruments=M] [log2=K] [epochs=E] [pmu]

use std::collections::HashMap;
use std::hint::black_box;
use titan_itch::book::L3Book;
use titan_itch::decode::{decode, Msg, Reader};
use titan_itch::gen::{feed, Mix};
use titan_itch::heapbook::{HeapBook, Policy};
use titan_itch::{Book, Side};
use titan_platform::alloc_count::{thread_allocations, CountingAlloc};
use titan_platform::clock;
use titan_platform::kperf::{events, Kperf, Session};
use titan_platform::qos::{self, Qos};

#[global_allocator]
static A: CountingAlloc = CountingAlloc;

const STAGES: [(&str, &str); 7] = [
    ("none", "none"),
    ("array", "top"),
    ("array", "depth"),
    ("heap_lazy", "top"),
    ("heap_lazy", "depth"),
    ("heap_indexed", "top"),
    ("heap_indexed", "depth"),
];

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

#[inline(always)]
fn fold(state: u64, t: Option<(u32, u64)>) -> u64 {
    match t {
        Some((p, s)) => state.rotate_left(7) ^ p as u64 ^ s,
        None => state.rotate_left(7) ^ 0xFFFF_FFFF,
    }
}

/// One pass over the feed that decodes and does nothing else: the work
/// every design below shares, so the books can be compared above it.
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

/// One pass over the feed with the array book.
#[inline(always)]
fn run_array(src: &[u8], book: &mut L3Book, limit: u64) -> (u64, u64) {
    let (mut n, mut sum) = (0u64, 0u64);
    let mut r = Reader::new(src);
    while n < limit {
        let Some(Ok(b)) = r.next_raw() else { break };
        n += 1;
        let Ok(m) = decode(b) else { continue };
        book.apply(&m);
        if let Some(loc) = locate_of(&m) {
            sum = fold(sum, book.best(loc, Side::Buy));
            sum = fold(sum, book.best(loc, Side::Sell));
        }
    }
    (n, sum)
}

/// One pass over the feed with the brief's heap book. `depth` selects the
/// workload that also totals the shares resting at the best price.
#[inline(always)]
fn run_heap<const DEPTH: bool>(src: &[u8], book: &mut HeapBook, limit: u64) -> (u64, u64) {
    let (mut n, mut sum) = (0u64, 0u64);
    let mut r = Reader::new(src);
    while n < limit {
        let Some(Ok(b)) = r.next_raw() else { break };
        n += 1;
        let Ok(m) = decode(b) else { continue };
        book.apply(&m);
        if let Some(loc) = locate_of(&m) {
            for side in [Side::Buy, Side::Sell] {
                sum = fold(sum, if DEPTH { book.best_depth(loc, side) } else { book.best(loc, side) });
            }
        }
    }
    (n, sum)
}

fn kv<'a>(args: &'a [String], key: &str) -> Option<&'a str> {
    args.iter().find_map(|a| a.strip_prefix(key).and_then(|r| r.strip_prefix('=')))
}

fn num<T: std::str::FromStr>(args: &[String], key: &str, default: T) -> T {
    kv(args, key).and_then(|v| v.parse().ok()).unwrap_or(default)
}

fn main() {
    let args: Vec<String> = std::env::args().collect();
    let epochs: usize = num(&args, "epochs", 10);
    let limit: u64 = num(&args, "limit", 10_000_000);
    let log2: u32 = num(&args, "log2", 22);
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

    // Highest locate in the window, so every book is sized the same way.
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

    // Untimed: the three books must agree message for message.
    let mut fast = L3Book::new(log2, instruments);
    let mut lazy = HeapBook::new(log2, instruments, Policy::Lazy);
    let mut indexed = HeapBook::new(log2, instruments, Policy::Indexed);
    let (mut checks, mut scanned, mut queries) = (0u64, 0u64, 0u64);
    // Order lifetimes: when each order arrived and whether it arrived at the
    // best price on its side.
    let mut born: HashMap<u64, (u64, bool)> = HashMap::with_capacity(1 << 20);
    const EDGES: [u64; 5] = [100_000, 1_000_000, 10_000_000, 100_000_000, 1_000_000_000];
    let mut all = [0u64; 6];
    let mut at_best = [0u64; 6];
    let (mut all_n, mut best_n, mut all_ns, mut best_ns) = (0u64, 0u64, 0u128, 0u128);
    let mut r = Reader::new(&src);
    let mut n = 0u64;
    while n < limit {
        let Some(Ok(b)) = r.next_raw() else { break };
        n += 1;
        let Ok(m) = decode(b) else { continue };
        fast.apply(&m);
        lazy.apply(&m);
        indexed.apply(&m);
        // Lifetimes, from this message's own timestamp.
        let (ts, named): (u64, [u64; 2]) = match m {
            Msg::Add { ts, order, .. } => (ts, [order, 0]),
            Msg::Executed { ts, order, .. } | Msg::ExecutedWithPrice { ts, order, .. } => (ts, [order, 0]),
            Msg::Cancel { ts, order, .. } | Msg::Delete { ts, order, .. } => (ts, [order, 0]),
            Msg::Replace { ts, old, new, .. } => (ts, [old, new]),
            Msg::Other { .. } => (0, [0, 0]),
        };
        if let Msg::Add { locate, order, side, price, .. } = m {
            let best = fast.best(locate, side).map(|t| t.0) == Some(price);
            born.insert(order, (ts, best));
        }
        if let Msg::Replace { new, ts, locate, .. } = m {
            if let Some((side, price)) = fast.order_info(new) {
                let best = fast.best(locate, side).map(|t| t.0) == Some(price);
                born.insert(new, (ts, best));
            }
        }
        for o in named {
            if o == 0 || fast.order_info(o).is_some() {
                continue;
            }
            let Some((t0, was_best)) = born.remove(&o) else { continue };
            let life = ts.saturating_sub(t0);
            let b = EDGES.iter().position(|&e| life < e).unwrap_or(5);
            all[b] += 1;
            all_n += 1;
            all_ns += life as u128;
            if was_best {
                at_best[b] += 1;
                best_n += 1;
                best_ns += life as u128;
            }
        }
        let Some(loc) = locate_of(&m) else { continue };
        for side in [Side::Buy, Side::Sell] {
            let want = fast.best(loc, side);
            assert_eq!(want.map(|t| t.0), lazy.best(loc, side).map(|t| t.0), "lazy best price at message {n}");
            assert_eq!(want.map(|t| t.0), indexed.best(loc, side).map(|t| t.0), "indexed best price at message {n}");
            assert_eq!(want, lazy.best_depth(loc, side), "lazy depth at message {n}");
            assert_eq!(want, indexed.best_depth(loc, side), "indexed depth at message {n}");
            if let Some((_, shares)) = want {
                // Orders the heap must visit to total that depth, as a share
                // of the work the array book does not do.
                scanned += shares;
            }
            queries += 1;
            checks += 4;
        }
    }
    assert_eq!(lazy.live_orders(), fast.live_orders());
    assert_eq!(indexed.live_orders(), fast.live_orders());
    println!(
        "heap_config feed={name} messages={n} instruments={instruments} log2={log2} limit={limit} epochs={epochs} pmu={} checks={checks} live_at_end={} stale_popped_lazy={} shares_at_best_per_query={:.2}",
        pmu.is_some(),
        fast.live_orders(),
        lazy.stale_popped,
        scanned as f64 / queries.max(1) as f64,
    );
    let share = |c: &[u64; 6], n: u64| {
        let mut cum = 0u64;
        (0..5)
            .map(|i| {
                cum += c[i];
                format!("{:.4}", cum as f64 / n.max(1) as f64)
            })
            .collect::<Vec<_>>()
            .join(",")
    };
    println!(
        "heap_life feed={name} orders_ended={all_n} mean_ms={:.3} within_100us_1ms_10ms_100ms_1s={} at_best_ended={best_n} at_best_mean_ms={:.3} at_best_within={} still_resting={}",
        all_ns as f64 / all_n.max(1) as f64 / 1e6,
        share(&all, all_n),
        best_ns as f64 / best_n.max(1) as f64 / 1e6,
        share(&at_best, best_n),
        born.len(),
    );
    drop(born);
    drop(fast);
    drop(lazy);
    drop(indexed);

    let freq = clock::counter_frequency() as f64;
    let mut fast = L3Book::new(log2, instruments);
    let mut lazy = HeapBook::new(log2, instruments, Policy::Lazy);
    let mut indexed = HeapBook::new(log2, instruments, Policy::Indexed);
    let run = |design: &str, workload: &str, pmu: Option<&Session>, fast: &mut L3Book, lazy: &mut HeapBook, indexed: &mut HeapBook| {
        match design {
            "none" => {}
            "array" => fast.clear(),
            "heap_lazy" => lazy.clear(),
            _ => indexed.clear(),
        }
        let a0 = thread_allocations();
        let p0 = pmu.map(|s| s.read());
        let t0 = clock::counter();
        let (msgs, sum) = match (design, workload) {
            ("none", _) => run_none(&src, limit),
            ("array", _) => run_array(&src, fast, limit),
            ("heap_lazy", "top") => run_heap::<false>(&src, lazy, limit),
            ("heap_lazy", _) => run_heap::<true>(&src, lazy, limit),
            (_, "top") => run_heap::<false>(&src, indexed, limit),
            (_, _) => run_heap::<true>(&src, indexed, limit),
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
    for (design, workload) in STAGES {
        run(design, workload, pmu.as_ref(), &mut fast, &mut lazy, &mut indexed);
    }
    for epoch in 0..epochs {
        for j in 0..STAGES.len() {
            let (design, workload) = STAGES[(j + epoch) % STAGES.len()];
            let (sum, msgs, ticks, d, allocs) = run(design, workload, pmu.as_ref(), &mut fast, &mut lazy, &mut indexed);
            let secs = ticks as f64 / freq;
            let mut line = format!(
                "heap feed={name} design={design} workload={workload} epoch={epoch} messages={msgs} ns_per_msg={:.3} allocations_pass={allocs} checksum={sum:016x}",
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
