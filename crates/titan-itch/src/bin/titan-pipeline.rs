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
//!   titan-pipeline [messages=N] [instruments=M] [rates=...] [waits=...] [epochs=E] [path=full|risk|none] [gaps] [pmu] [split] [warm=book|mem]
//!   titan-pipeline file=PATH [from=SECONDS] [span=SECONDS] [limit=N] (rates=... | speed=S) [waits=...] [epochs=E] [gaps]
//!
//! `split` adds a `pl_split` line per run with the delay to pop (schedule to
//! pop) and the service time (pop to decision) separately; it costs one more
//! counter read per message. `warm=book|mem` (with `waits=isb`) keeps state
//! warm while the consumer waits: every `warm_us` (default 20) it runs up to
//! 16 read-only units, polling the ring between units, either best-price
//! lookups on recently touched instruments plus a decode of the last
//! message (`book`), or loads spread over a 128 MiB scratch buffer (`mem`).
//!
//! `path=none` pops and records each message without running the trading
//! path: the cost of waiting itself, as a control. `pmu` (run under sudo)
//! adds the consumer thread's cycles, instructions, L1D load misses and
//! branch mispredicts per message over each run; waiting is included, and
//! kperf counts kernel work done while the thread runs.
//!
//! `gaps` also keeps one histogram per class of scheduled gap to the previous
//! message (under 100 ns, 100 ns to 1 us, 1 to 10 us, 10 to 100 us, over
//! 100 us; the first message goes to the totals only) and prints a `pl_gap`
//! line per class: are bursts or idle gaps behind the tail?
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
use titan_platform::kperf::{events, Kperf, Session};
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
const GAP_NAMES: [&str; 5] = ["lt100ns", "100ns_1us", "1us_10us", "gt100us", "10us_100us"];
const NO_CLASS: u8 = 5;

/// Gap class of every message under `sched` (counter ticks).
fn gap_classes(sched: &[u64], freq: u64) -> Vec<u8> {
    let t = |ns: u64| ns * freq / 1_000_000_000;
    let (a, b, c, d) = (t(100), t(1_000), t(10_000), t(100_000));
    let mut out = vec![NO_CLASS; sched.len()];
    for i in 1..sched.len() {
        let g = sched[i] - sched[i - 1];
        out[i] = if g < a {
            0
        } else if g < b {
            1
        } else if g < c {
            2
        } else if g > d {
            3
        } else {
            4
        };
    }
    out
}

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
    /// Consumer thread counters over the whole run, waiting included:
    /// cycles, instructions, L1D load misses, branch mispredicts.
    pmu: [u64; 4],
    warm_units: u64,
}

struct Ctx<'a> {
    feed: &'a [u8],
    descs: &'a [u64],
    prefix: usize,
    window: std::ops::Range<usize>,
    max_spread: u32,
    with_risk: bool,
    /// `path=none`: pop and record only, no trading path (control for
    /// what waiting itself costs).
    none: bool,
    pmu: Option<&'a Session>,
    /// Also record the delay to pop (schedule to pop) and the service time
    /// (pop to decision) in two more histograms.
    split: bool,
    /// Keep-warm while waiting: 0 off, 1 book (best bid and ask of recently
    /// touched instruments, decode of the last message), 2 memory (loads
    /// from a scratch buffer larger than the system cache).
    warm: u8,
    warm_ticks: u64,
    scratch: &'a [u64],
    ring: usize,
}

fn path_name(with_risk: bool, none: bool, warm: u8) -> &'static str {
    match (none, with_risk, warm) {
        (true, _, _) => "none",
        (_, true, 0) => "risk",
        (_, false, 0) => "full",
        (_, true, 1) => "risk_warmbook",
        (_, false, 1) => "full_warmbook",
        (_, true, _) => "risk_warmmem",
        (_, false, _) => "full_warmmem",
    }
}

/// Recently touched instruments, for the book keep-warm.
const RECENT: usize = 64;
/// Keep-warm units per period; the ring is polled between units, so a
/// message waits for at most one unit.
const WARM_UNITS: usize = 16;

/// One keep-warm unit: two best-price lookups (book) or one load from the
/// scratch buffer (memory). Read-only: the book is not changed.
#[inline(never)]
fn warm_unit(c: &Ctx, book: &L3Book, recent: &[u16; RECENT], last_d: u64, unit: usize, pos: &mut usize) -> u64 {
    if c.warm == 1 {
        let loc = recent[unit % RECENT];
        let mut x = 0u64;
        if let Some((p, q)) = book.best(loc, Side::Buy) {
            x ^= p as u64 ^ q;
        }
        if let Some((p, q)) = book.best(loc, Side::Sell) {
            x ^= p as u64 ^ q;
        }
        if unit == 0 && last_d != 0 {
            if let Ok(m) = decode(body(c.feed, last_d)) {
                x ^= locate_of(&m) as u64;
            }
        }
        x
    } else {
        // Large odd stride through the buffer: defeats the prefetchers, so
        // each unit is one access well beyond the system cache.
        *pos = (*pos + 1_048_573) % c.scratch.len();
        c.scratch[*pos]
    }
}

fn prime(c: &Ctx, book: &mut L3Book, risk: &mut Risk) {
    book.clear();
    risk.reset();
    let mut state = 0u64;
    for &d in &c.descs[..c.prefix] {
        step::<false>(c.feed, d, book, risk, &mut state, c.max_spread);
    }
}

/// One timed run of the window under `sched` (counter ticks after start).
#[allow(clippy::too_many_arguments)]
fn run<W: Wait>(
    c: &Ctx,
    sched: &[u64],
    cls: &[u8],
    book: &mut L3Book,
    risk: &mut Risk,
    hist: &mut Histogram,
    gh: &mut [Histogram],
    sh: &mut [Histogram],
) -> RunResult {
    prime(c, book, risk);
    hist.reset();
    gh.iter_mut().for_each(Histogram::reset);
    sh.iter_mut().for_each(Histogram::reset);
    let freq = clock::counter_frequency();
    let (tx, rx): (WaitingProducer<AcqRel, Pad128, W, true>, WaitingConsumer<AcqRel, Pad128, W, true>) =
        channel(c.ring);
    let descs = &c.descs[c.window.clone()];
    let u0 = Usage::now();
    // Start 5 ms ahead so both threads are running and waiting.
    let start = clock::counter() + freq / 200;
    let mut result = RunResult { messages: 0, span_ticks: 0, late_max: 0, late_over_1us: 0, orders: 0, allocations: 0, invol: 0, pmu: [0; 4], warm_units: 0 };
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
        let p0 = c.pmu.map(|s| s.read());
        let (mut i, mut orders, mut last) = (0usize, 0u64, start);
        let mut recent = [0u16; RECENT];
        let (mut last_d, mut pos, mut wsum, mut units) = (0u64, 0usize, 0u64, 0u64);
        let mut next_warm = start;
        loop {
            let d = if c.warm == 0 {
                rx.recv()
            } else {
                let mut unit = WARM_UNITS;
                loop {
                    if let Some(v) = rx.inner.pop() {
                        break v;
                    }
                    if unit < WARM_UNITS {
                        wsum ^= warm_unit(c, book, &recent, last_d, unit, &mut pos);
                        unit += 1;
                        units += 1;
                    } else if clock::counter() >= next_warm {
                        unit = 0;
                        next_warm = clock::counter() + c.warm_ticks;
                    } else {
                        std::hint::spin_loop();
                    }
                }
            };
            if d == STOP {
                break;
            }
            let tp = if c.split { clock::counter() } else { 0 };
            if c.warm == 1 && !c.none {
                let b = body(c.feed, d);
                if b.len() >= 3 {
                    recent[i % RECENT] = u16::from_be_bytes([b[1], b[2]]);
                }
                last_d = d;
            }
            orders += if c.none {
                std::hint::black_box(d);
                0
            } else if c.with_risk {
                step::<true>(c.feed, d, book, risk, &mut state, c.max_spread)
            } else {
                step::<false>(c.feed, d, book, risk, &mut state, c.max_spread)
            };
            let t = clock::counter();
            let v = t.saturating_sub(start + sched[i]);
            hist.record(v);
            if c.split {
                sh[0].record(tp.saturating_sub(start + sched[i]));
                sh[1].record(t.saturating_sub(tp));
            }
            if let Some(&k) = cls.get(i) {
                if k != NO_CLASS {
                    gh[k as usize].record(v);
                }
            }
            last = t;
            i += 1;
        }
        if let (Some(s), Some(a)) = (c.pmu, p0) {
            let b = s.read();
            let d = s.delta(&a, &b);
            result.pmu = [d[0], d[1], d[2], d[3]];
        }
        result.allocations = thread_allocations() - a0;
        std::hint::black_box(wsum);
        result.warm_units = units;
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
    let none = kv(&args, "path") == Some("none");
    let split = args.iter().any(|a| a == "split");
    let warm: u8 = match kv(&args, "warm") {
        None | Some("off") => 0,
        Some("book") => 1,
        Some("mem") => 2,
        Some(o) => panic!("unknown warm {o}"),
    };
    let warm_us: u64 = num(&args, "warm_us", 20);
    // 128 MiB of scratch, written once so every page is resident.
    let scratch: Vec<u64> = if warm == 2 { (0..(128usize << 20) / 8).map(|i| i as u64).collect() } else { Vec::new() };
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
    let gaps = args.iter().any(|a| a == "gaps");
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
        path_name(with_risk, none, warm),
        (f.ts[window.clone()].iter().max().copied().unwrap_or(0).saturating_sub(f.ts[window.start])) as f64 / 1e9,
    );

    let c = Ctx { feed: feed_buf, descs: &f.descs, prefix, window, max_spread, with_risk, none, pmu: pmu.as_ref(), split, warm,
        warm_ticks: warm_us * clock::counter_frequency() / 1_000_000, scratch: &scratch, ring };
    let kill = AtomicBool::new(false);
    let mut risk = Risk::new(instruments, Limits::default(), &kill);
    let mut book = L3Book::new(log2, instruments);
    let mut hist = Histogram::new();
    let mut gh: Vec<Histogram> = (0..GAP_NAMES.len()).map(|_| Histogram::new()).collect();
    let mut sh: Vec<Histogram> = (0..2).map(|_| Histogram::new()).collect();
    // With keep-warm the consumer polls the ring itself (pop, keep-warm
    // unit or isb), whatever the wait type, so only isb is meaningful.
    assert!(c.warm == 0 || waits.iter().all(|w| w == "isb"), "warm= needs waits=isb");
    let classes: Vec<Vec<u8>> = loads.iter().map(|(_, s)| if gaps { gap_classes(s, freq) } else { Vec::new() }).collect();

    // Warm-up: one untimed single-thread pass of the whole window grows the
    // level vectors to their working size.
    prime(&c, &mut book, &mut risk);
    let mut state = 0u64;
    for &d in &c.descs[c.window.clone()] {
        step::<true>(c.feed, d, &mut book, &mut risk, &mut state, c.max_spread);
    }

    let to_ns = |ticks: u64| ticks as f64 * 1e9 / freq as f64;
    for epoch in 0..epochs {
        for (li, (load, sched)) in loads.iter().enumerate() {
            for j in 0..waits.len() {
                let w = &waits[(j + epoch + li) % waits.len()];
                let cls = &classes[li];
                let r = match w.as_str() {
                    "spin" => run::<Spin>(&c, sched, cls, &mut book, &mut risk, &mut hist, &mut gh, &mut sh),
                    "isb" => run::<SpinHint>(&c, sched, cls, &mut book, &mut risk, &mut hist, &mut gh, &mut sh),
                    "wfe" => run::<Wfe>(&c, sched, cls, &mut book, &mut risk, &mut hist, &mut gh, &mut sh),
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
                     feed_late_max_ns={:.1} feed_late_over_1us={} orders={} allocations={} invol_switches={}{}",
                    path_name(c.with_risk, c.none, c.warm),
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
                    if c.pmu.is_some() {
                        let m = r.messages.max(1) as f64;
                        format!(
                            " msg_cycles={:.2} msg_instructions={:.2} msg_l1d_miss={:.4} msg_br_mispred={:.4}",
                            r.pmu[0] as f64 / m,
                            r.pmu[1] as f64 / m,
                            r.pmu[2] as f64 / m,
                            r.pmu[3] as f64 / m
                        )
                    } else {
                        String::new()
                    },
                );
                if c.split {
                    for (part, h) in ["pop", "service"].iter().zip(&sh) {
                        println!(
                            "pl_split feed={name} load={load} wait={w} path={} epoch={epoch} saturated={} part={part} messages={} \
                             p50_ns={:.1} p90_ns={:.1} p99_ns={:.1} p999_ns={:.1} max_ns={:.1} mean_ns={:.1} warm_units_per_msg={:.2}",
                            path_name(c.with_risk, c.none, c.warm),
                            saturated as u8,
                            h.count(),
                            to_ns(h.quantile(0.5)),
                            to_ns(h.quantile(0.9)),
                            to_ns(h.quantile(0.99)),
                            to_ns(h.quantile(0.999)),
                            to_ns(h.max()),
                            h.mean() * 1e9 / freq as f64,
                            r.warm_units as f64 / r.messages.max(1) as f64,
                        );
                    }
                }
                if gaps {
                    for (g, h) in GAP_NAMES.iter().zip(&gh) {
                        println!(
                            "pl_gap feed={name} load={load} wait={w} path={} epoch={epoch} saturated={} gap={g} messages={} \
                             p50_ns={:.1} p90_ns={:.1} p99_ns={:.1} p999_ns={:.1} max_ns={:.1} mean_ns={:.1}",
                            path_name(c.with_risk, c.none, c.warm),
                            saturated as u8,
                            h.count(),
                            to_ns(h.quantile(0.5)),
                            to_ns(h.quantile(0.9)),
                            to_ns(h.quantile(0.99)),
                            to_ns(h.quantile(0.999)),
                            to_ns(h.max()),
                            h.mean() * 1e9 / freq as f64,
                        );
                    }
                }
            }
        }
    }
}
