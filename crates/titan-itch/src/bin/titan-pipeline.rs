//! Open-loop, two-core tick-to-decision pipeline with per-message latency.
//!
//! A feed thread releases each message at its scheduled time, as a zero-copy
//! descriptor through an SPSC ring (AcqRel ordering, 128-byte index padding,
//! index caching). A trading thread on another core pops it, decodes it in
//! place, updates the L3 book and runs the signal (and, with `path=risk`, the
//! pre-trade checks). Both threads run at QoS USER_INTERACTIVE.
//!
//! Latency of message i = (time the trading thread finished it) minus (the
//! time it was scheduled to arrive), both read from the system counter, which
//! is shared by all cores. Measuring from the schedule rather than from the
//! actual send makes the result immune to coordinated omission: if the feed
//! thread falls behind or the ring backs up, the waiting time is counted.
//! Values are quantised to the counter step (41.67 ns on Apple M4) and kept
//! in a log-linear histogram (relative error below 0.05%).
//!
//! Schedules:
//!   rates=R1,R2,...   constant offered load, messages per second
//!   speed=S           the feed's own ITCH timestamps, replayed S times faster
//!                     (needs file=; keeps the real burst structure)
//! The trading thread waits for the next message with `spin` (reload only),
//! `isb` (Rust's spin_loop) or `wfe` (exclusive monitor on the ring index).
//!
//!   titan-pipeline [messages=N] [instruments=M] [rates=...] [waits=...] [epochs=E] [path=full|risk]
//!   titan-pipeline file=PATH [from=SECONDS] [span=SECONDS] [limit=N] (rates=... | speed=S) [waits=...] [epochs=E]
//!
//! `from` and `span` select a window of feed time (seconds since midnight);
//! every message before the window is applied to the book first, untimed, so
//! the window starts from the real book state.

use std::sync::atomic::AtomicBool;
use std::thread;
use titan_itch::book::L3Book;
use titan_itch::decode::{decode, Msg, Reader};
use titan_itch::gen::{feed, Mix};
use titan_itch::path::{locate_of, signal, timestamp, Limits, Risk};
use titan_itch::reference::RefBook;
use titan_itch::{Book, Side};
use titan_platform::alloc_count::{thread_allocations, CountingAlloc};
use titan_platform::clock;
use titan_platform::hist::Histogram;
use titan_platform::mem::PinnedRegion;
use titan_platform::qos::{self, Qos};
use titan_platform::rusage::Usage;
use titan_spsc::order::AcqRel;
use titan_spsc::pad::Pad128;
use titan_spsc::ring::{channel, WaitingConsumer, WaitingProducer};
use titan_spsc::Rx;
use titan_spsc::wait::{Spin, SpinHint, Wait, Wfe};

#[global_allocator]
static A: CountingAlloc = CountingAlloc;

const STOP: u64 = u64::MAX;

fn kv<'a>(args: &'a [String], key: &str) -> Option<&'a str> {
    args.iter().find_map(|a| a.strip_prefix(key).and_then(|r| r.strip_prefix('=')))
}

fn num<T: std::str::FromStr>(args: &[String], key: &str, default: T) -> T {
    kv(args, key).and_then(|v| v.parse().ok()).unwrap_or(default)
}

struct Feed {
    descs: Vec<u64>,
    ts: Vec<u64>,
    len: usize,
    max_locate: u16,
    peak: usize,
}

/// Descriptors (`offset << 16 | length`), ITCH timestamps, maximum locate and
/// peak live orders of the first `limit` messages.
fn index(buf: &[u8], limit: usize) -> Feed {
    let mut r = Reader::new(buf);
    let (mut descs, mut ts) = (Vec::new(), Vec::new());
    let (mut max_locate, mut off, mut peak) = (0u16, 0usize, 0usize);
    let mut probe = RefBook::default();
    while descs.len() < limit {
        match r.next_raw() {
            Some(Ok(b)) => {
                assert!(b.len() < 1 << 16 && off + 2 < 1 << 48);
                descs.push(((off as u64 + 2) << 16) | b.len() as u64);
                ts.push(timestamp(b));
                off += 2 + b.len();
                if b.len() >= 3 {
                    max_locate = max_locate.max(u16::from_be_bytes([b[1], b[2]]));
                }
                if let Ok(m) = decode(b) {
                    probe.apply(&m);
                    peak = peak.max(probe.live_orders());
                }
            }
            Some(Err(e)) => {
                eprintln!("malformed input after {} messages: {e:?}", descs.len());
                break;
            }
            None => break,
        }
    }
    Feed { descs, ts, len: off, max_locate, peak }
}

#[inline(always)]
fn body(feed: &[u8], d: u64) -> &[u8] {
    let off = (d >> 16) as usize;
    &feed[off..off + (d & 0xffff) as usize]
}

/// The trading path for one message. Returns 1 if an order left the path.
#[inline(always)]
fn step<const RISK: bool>(feed: &[u8], d: u64, book: &mut L3Book, risk: &mut Risk, state: &mut u64, max_spread: u32) -> u64 {
    let b = body(feed, d);
    let Ok(m) = decode(b) else {
        *state = state.rotate_left(1);
        return 0;
    };
    book.apply(&m);
    if matches!(m, Msg::Other { .. }) {
        *state = state.rotate_left(1);
        return 0;
    }
    let loc = locate_of(&m);
    let bid = book.best(loc, Side::Buy);
    let ask = book.best(loc, Side::Sell);
    let Some(it) = signal(state, loc, bid, ask, max_spread) else {
        return 0;
    };
    if RISK {
        let mid = match (bid, ask) {
            (Some((bp, _)), Some((ap, _))) => (bp as u64 + ap as u64) >> 1,
            _ => 0,
        };
        if !risk.check(&it, mid, timestamp(b)) {
            return 0;
        }
    }
    *state ^= it.price as u64 ^ ((it.shares as u64) << 32);
    1
}

struct RunResult {
    messages: u64,
    span_ticks: u64,
    late_max: u64,
    late_over_1us: u64,
    orders: u64,
    allocations: u64,
    invol: i64,
}

struct Ctx<'a> {
    feed: &'a [u8],
    descs: &'a [u64],
    prefix: usize,
    window: std::ops::Range<usize>,
    max_spread: u32,
    with_risk: bool,
    ring: usize,
}

fn warm(c: &Ctx, book: &mut L3Book, risk: &mut Risk) {
    book.clear();
    risk.reset();
    let mut state = 0u64;
    for &d in &c.descs[..c.prefix] {
        step::<false>(c.feed, d, book, risk, &mut state, c.max_spread);
    }
}

/// One timed run of the window under `sched` (counter ticks after start).
fn run<W: Wait>(c: &Ctx, sched: &[u64], book: &mut L3Book, risk: &mut Risk, hist: &mut Histogram) -> RunResult {
    warm(c, book, risk);
    hist.reset();
    let freq = clock::counter_frequency();
    let (tx, rx): (WaitingProducer<AcqRel, Pad128, W, true>, WaitingConsumer<AcqRel, Pad128, W, true>) =
        channel(c.ring);
    let descs = &c.descs[c.window.clone()];
    let u0 = Usage::now();
    // Start 5 ms ahead so both threads are running and waiting.
    let start = clock::counter() + freq / 200;
    let mut result = RunResult { messages: 0, span_ticks: 0, late_max: 0, late_over_1us: 0, orders: 0, allocations: 0, invol: 0 };
    let one_us = freq / 1_000_000;
    thread::scope(|s| {
        let producer = s.spawn(move || {
            let _ = qos::set_current_thread(Qos::UserInteractive);
            let mut tx = tx;
            let (mut late_max, mut late_over) = (0u64, 0u64);
            for (i, &d) in descs.iter().enumerate() {
                let due = start + sched[i];
                while clock::counter() < due {
                    std::hint::spin_loop();
                }
                while !tx.inner.push(d) {
                    std::hint::spin_loop();
                }
                let late = clock::counter().saturating_sub(due);
                late_max = late_max.max(late);
                late_over += (late > one_us) as u64;
            }
            while !tx.inner.push(STOP) {
                std::hint::spin_loop();
            }
            (late_max, late_over)
        });
        let _ = qos::set_current_thread(Qos::UserInteractive);
        let mut rx = rx;
        let mut state = 0x9E37_79B9_7F4A_7C15u64;
        let a0 = thread_allocations();
        let (mut i, mut orders, mut last) = (0usize, 0u64, start);
        loop {
            let d = rx.recv();
            if d == STOP {
                break;
            }
            orders += if c.with_risk {
                step::<true>(c.feed, d, book, risk, &mut state, c.max_spread)
            } else {
                step::<false>(c.feed, d, book, risk, &mut state, c.max_spread)
            };
            let t = clock::counter();
            hist.record(t.saturating_sub(start + sched[i]));
            last = t;
            i += 1;
        }
        result.allocations = thread_allocations() - a0;
        result.messages = i as u64;
        result.span_ticks = last - start;
        std::hint::black_box(state);
        result.orders = orders;
        let (lm, lo) = producer.join().expect("feed thread");
        result.late_max = lm;
        result.late_over_1us = lo;
    });
    result.invol = Usage::now().since(&u0).involuntary_switches;
    result
}

fn main() {
    let args: Vec<String> = std::env::args().collect();
    let epochs: usize = num(&args, "epochs", 3);
    let max_spread: u32 = num(&args, "spread", 300);
    let ring: usize = num(&args, "ring", 4096);
    let with_risk = kv(&args, "path") == Some("risk");
    let waits: Vec<String> = kv(&args, "waits").unwrap_or("spin,isb,wfe").split(',').map(str::to_string).collect();
    let _ = qos::set_current_thread(Qos::UserInteractive);

    let (src, name) = match kv(&args, "file") {
        Some(path) => {
            let raw = std::fs::read(path).unwrap_or_else(|e| panic!("cannot read {path}: {e}"));
            let n = std::path::Path::new(path).file_name().and_then(|f| f.to_str()).unwrap_or("file").replace('.', "_");
            (raw, n)
        }
        None => {
            let messages: usize = num(&args, "messages", 2_000_000);
            let instruments: u16 = num(&args, "instruments", 500);
            (feed(42, instruments, messages, Mix::default()), "synthetic_default".to_string())
        }
    };
    let f = index(&src, usize::MAX);
    let mut region = PinnedRegion::new(f.len.max(1)).expect("feed region");
    region.as_mut_slice()[..f.len].copy_from_slice(&src[..f.len]);
    drop(src);
    let locked = region.is_locked();
    let feed_buf = &region.as_mut_slice()[..f.len];

    // Window of feed time; everything before it is applied untimed.
    let from_ns = (num::<f64>(&args, "from", 0.0) * 1e9) as u64;
    let span_ns = kv(&args, "span").and_then(|v| v.parse::<f64>().ok()).map(|s| (s * 1e9) as u64);
    let limit: usize = num(&args, "limit", usize::MAX);
    // Linear scans, not binary search: a file may carry messages whose
    // timestamps are out of order.
    let prefix = if from_ns == 0 { 0 } else { f.ts.iter().position(|&t| t >= from_ns).unwrap_or(f.ts.len()) };
    let mut end = f.descs.len();
    if let (Some(sp), Some(&t0)) = (span_ns, f.ts.get(prefix)) {
        end = prefix + f.ts[prefix..].iter().position(|&t| t >= t0 + sp).unwrap_or(f.ts.len() - prefix);
    }
    end = end.min(prefix.saturating_add(limit));
    assert!(end > prefix, "empty window");
    let window = prefix..end;
    let n = window.len();

    let freq = clock::counter_frequency();
    let mut loads: Vec<(String, Vec<u64>)> = Vec::new();
    let mut ts_backwards = 0u64;
    if let Some(sp) = kv(&args, "speed") {
        let speed: f64 = sp.parse().expect("speed");
        let t0 = f.ts[window.start];
        // A sequenced feed cannot deliver a message before the one ahead of
        // it: a timestamp that goes backwards is released with its
        // predecessor, and counted.
        let mut latest = t0;
        let sched = f.ts[window.clone()]
            .iter()
            .map(|&t| {
                if t < latest {
                    ts_backwards += 1;
                } else {
                    latest = t;
                }
                ((latest - t0) as f64 / speed * freq as f64 / 1e9) as u64
            })
            .collect();
        loads.push((format!("speed_{sp}"), sched));
    } else {
        for r in kv(&args, "rates").unwrap_or("1000000,5000000,10000000,15000000,20000000").split(',') {
            let rate: f64 = r.parse().expect("rate");
            let sched = (0..n).map(|i| (i as f64 * freq as f64 / rate) as u64).collect();
            loads.push((format!("rate_{r}"), sched));
        }
    }

    let mut log2 = 10u32;
    while ((1usize << log2) / 2) < f.peak {
        log2 += 1;
    }
    let instruments = f.max_locate as usize + 1;
    println!(
        "pl_config feed={name} messages={} window={}..{} window_messages={n} instruments={instruments} peak_live_orders={} \
         order_table_slots={} ring={ring} path={} epochs={epochs} feed_locked={locked} counter_hz={freq} \
         window_feed_seconds={:.3} ts_backwards={ts_backwards}",
        f.descs.len(),
        window.start,
        window.end,
        f.peak,
        1usize << log2,
        if with_risk { "risk" } else { "full" },
        (f.ts[window.clone()].iter().max().copied().unwrap_or(0).saturating_sub(f.ts[window.start])) as f64 / 1e9,
    );

    let c = Ctx { feed: feed_buf, descs: &f.descs, prefix, window, max_spread, with_risk, ring };
    let kill = AtomicBool::new(false);
    let mut risk = Risk::new(instruments, Limits::default(), &kill);
    let mut book = L3Book::new(log2, instruments);
    let mut hist = Histogram::new();

    // Warm-up: one untimed single-thread pass of the whole window grows the
    // level vectors to their working size.
    warm(&c, &mut book, &mut risk);
    let mut state = 0u64;
    for &d in &c.descs[c.window.clone()] {
        step::<true>(c.feed, d, &mut book, &mut risk, &mut state, c.max_spread);
    }

    let to_ns = |ticks: u64| ticks as f64 * 1e9 / freq as f64;
    for epoch in 0..epochs {
        for (li, (load, sched)) in loads.iter().enumerate() {
            for j in 0..waits.len() {
                let w = &waits[(j + epoch + li) % waits.len()];
                let r = match w.as_str() {
                    "spin" => run::<Spin>(&c, sched, &mut book, &mut risk, &mut hist),
                    "isb" => run::<SpinHint>(&c, sched, &mut book, &mut risk, &mut hist),
                    "wfe" => run::<Wfe>(&c, sched, &mut book, &mut risk, &mut hist),
                    other => panic!("unknown wait {other}"),
                };
                let offered_span = to_ns(*sched.last().unwrap()) / 1e9;
                let achieved = r.messages as f64 / (to_ns(r.span_ticks) / 1e9);
                let offered = if offered_span > 0.0 { (n as f64 - 1.0) / offered_span } else { f64::NAN };
                // Offered load above capacity: the queue grows for the whole run
                // and percentiles then depend on run length, not on the system.
                let saturated = achieved < 0.97 * offered;
                println!(
                    "pl feed={name} load={load} wait={w} path={} epoch={epoch} saturated={} messages={} offered_mps={:.3} achieved_mps={:.3} \
                     p50_ns={:.1} p90_ns={:.1} p99_ns={:.1} p999_ns={:.1} p9999_ns={:.1} p99999_ns={:.1} max_ns={:.1} mean_ns={:.1} \
                     feed_late_max_ns={:.1} feed_late_over_1us={} orders={} allocations={} invol_switches={}",
                    if c.with_risk { "risk" } else { "full" },
                    saturated as u8,
                    r.messages,
                    offered / 1e6,
                    achieved / 1e6,
                    to_ns(hist.quantile(0.5)),
                    to_ns(hist.quantile(0.9)),
                    to_ns(hist.quantile(0.99)),
                    to_ns(hist.quantile(0.999)),
                    to_ns(hist.quantile(0.9999)),
                    to_ns(hist.quantile(0.99999)),
                    to_ns(hist.max()),
                    hist.mean() * 1e9 / freq as f64,
                    to_ns(r.late_max),
                    r.late_over_1us,
                    r.orders,
                    r.allocations,
                    r.invol,
                );
            }
        }
    }
}
