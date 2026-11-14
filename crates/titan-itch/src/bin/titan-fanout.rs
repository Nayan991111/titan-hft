//! One book, several strategy threads: what sharing the top of book costs
//! the thread that owns the book, and how fresh it is for the readers.
//!
//! The writer (main thread, QoS USER_INTERACTIVE) replays the feed as fast as
//! it can: decode, L3 book update and, for the K subscribed instruments (the
//! K with the most book messages in the window), the new best bid and ask
//! whenever either changed. N reader threads (QoS USER_INTERACTIVE, busy
//! polling) take those updates. Designs:
//!
//!   none  every update is built and discarded (`black_box` of the 32-byte
//!         update, one stack store): the floor
//!   copy  one titan-spsc ring per reader (AcqRel, 128-byte index padding,
//!         cached indices, `ring` updates of 32 bytes); the writer pushes every
//!         update into every ring and spins while a ring is full
//!   snap  one seqlock slot per subscribed instrument, alone in a 128-byte
//!         line; the writer overwrites it, every reader polls the K slots
//!
//! Every design, `none` included, reads the generic counter (`isb; mrs`) once
//! per update for the staleness stamp, so that cost is in the floor.
//!
//! Writer: generic counter over the whole pass and, with `pmu`, the writer
//! thread's core cycles, instructions, L1D load misses and branch
//! mispredicts over the same span. Readers: the staleness of every update a
//! reader takes is the counter value when the reader holds a consistent copy
//! minus the counter value the writer read just before publishing it (the
//! counter is shared by all cores; 41.67 ns step on the M4). Staleness values
//! pool all readers of a run. A snap reader sees only the latest version of a
//! slot; versions it never saw are counted as conflated, and its staleness
//! covers only the versions it took.
//!
//! Work: `work=book` (default) is the above, at the pace the feed and the
//! book allow; values are per feed message. `work=replay` publishes the same
//! updates (recorded once, untimed) back to back, repeated until at least
//! `updates` (default 2,000,000) have been published, with no feed and no
//! book: values are per published update, and readers face the highest
//! update rate the writer can produce.
//!
//! Checks (the harness aborts on failure):
//! * the updates recorded for `work=replay` match the reference pass of the
//!   feed: count, versions and final top of book of every slot;
//! * every run publishes the reference count, versions and final top of book;
//! * every copy reader receives every update in version order, and the fold
//!   of their content equals the fold of the recorded sequence;
//! * versions taken plus versions conflated equal versions published, for
//!   every snap reader, and every reader ends holding the final top of book;
//! * in the unmeasured warm-up pass of every config, every reader checks the
//!   content of every update it takes against the recorded sequence (in the
//!   measured passes this check is off, so readers do not pay for it). A
//!   snap reader takes only some versions, so for snap this covers the
//!   versions taken, not every version published.
//!
//!   titan-fanout [messages=N] [instruments=M] [subs=K] [configs=a,b] [work=book|replay] [updates=U] [epochs=E] [ring=R] [pmu]
//!   titan-fanout file=PATH [limit=N] [subs=K] [configs=a,b] [work=book|replay] [updates=U] [epochs=E] [ring=R] [pmu]
//!
//! Configs: `none`, `snapN` (N = 0 to 8 readers) and `copyN` (N = 1 to 8).
//! Default: none,snap0,copy1,snap1,copy2,snap2,copy3,snap3.

use std::hint::black_box;
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use std::thread;
use titan_itch::book::L3Book;
use titan_itch::decode::{decode, Msg, Reader};
use titan_itch::fanout::{pack, Quote, Slot};
use titan_itch::gen::{feed, Mix};
use titan_itch::path::locate_of;
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
use titan_spsc::ring::{ring, Consumer, Producer};

#[global_allocator]
static A: CountingAlloc = CountingAlloc;

type Tx = Producer<Quote, AcqRel, Pad128, true>;
type Rx = Consumer<Quote, AcqRel, Pad128, true>;

const NONE: u8 = 0;
const COPY: u8 = 1;
const SNAP: u8 = 2;
const NO_SLOT: u16 = u16::MAX;
const MAX_READERS: usize = 8;

/// A value alone in 128-byte lines, so state the writer touches on every
/// update never shares a line with state the readers poll.
#[repr(C, align(128))]
struct Line<T>(T);

/// One recorded update: slot, locate, bid, ask.
type Rec = (u16, u16, u64, u64);

struct Ctx<'a> {
    /// `work=replay`: the recorded updates, replayed `reps` times; empty for
    /// `work=book`.
    recs: &'a [Rec],
    reps: usize,
    feed: &'a [u8],
    descs: &'a [u64],
    slot_of: &'a [u16],
    k: usize,
    ring: usize,
    pmu: Option<&'a Session>,
}

/// Writer state of one pass.
#[derive(Clone, PartialEq, Eq, Debug)]
struct Published {
    last: Vec<(u64, u64)>,
    ver: Vec<u32>,
    quotes: u64,
}

/// Content of every version of every slot, from the recorded updates:
/// `t[s][(v - 1) % t[s].len()]` is version `v` of slot `s` (the modulo
/// covers the repetitions of `work=replay`).
type Table = [Vec<(u64, u64)>];

/// Readers' start and termination flags.
struct Flags {
    ready: AtomicUsize,
    done: AtomicBool,
}

/// What one reader took during a pass.
struct Seen {
    taken: u64,
    conflated: u64,
    retries: u64,
    order_errors: u64,
    content_errors: u64,
    hist: Histogram,
    last: Vec<(u64, u64)>,
    ver: Vec<u32>,
    sum: u64,
}

impl Seen {
    fn new(k: usize) -> Seen {
        let mut hist = Histogram::new();
        // Touches every page, so no page of the histogram faults in the
        // measured span.
        hist.prefault();
        Seen { taken: 0, conflated: 0, retries: 0, order_errors: 0, content_errors: 0, hist, last: vec![(0, 0); k], ver: vec![0; k], sum: 0 }
    }
}

#[inline(always)]
fn fold(sum: u64, bid: u64, ask: u64) -> u64 {
    sum.rotate_left(5) ^ bid ^ ask
}

#[inline(always)]
fn expected(t: &Table, slot: usize, ver: u32) -> (u64, u64) {
    let v = &t[slot];
    v[(ver as usize - 1) % v.len()]
}

#[inline(always)]
fn body(feed: &[u8], d: u64) -> &[u8] {
    let off = (d >> 16) as usize;
    &feed[off..off + (d & 0xffff) as usize]
}

/// Pushes that found a ring full, and spin iterations spent waiting.
#[derive(Default, Clone, Copy)]
struct Waits {
    full: u64,
    spins: u64,
}

/// Publish one update with design `D`.
#[inline(always)]
fn publish<const D: u8>(q: Quote, tx: &mut [Tx], slots: &[Slot], w: &mut Waits) {
    match D {
        NONE => {
            black_box(q);
        }
        COPY => {
            for t in tx.iter_mut() {
                if !t.push(q) {
                    w.full += 1;
                    while !t.push(q) {
                        w.spins += 1;
                        std::hint::spin_loop();
                    }
                }
            }
        }
        _ => slots[q.slot as usize].write(q.bid, q.ask, q.stamp),
    }
}

/// The timed writer loop of `work=book` for design `D`. Inlined into one
/// named function per design (below), so every loop has a readable symbol
/// under either name-mangling scheme.
#[inline(always)]
fn write_pass<const D: u8>(c: &Ctx, book: &mut L3Book, p: &mut Published, tx: &mut [Tx], slots: &[Slot]) -> Waits {
    let mut w = Waits::default();
    for &d in c.descs {
        let Ok(m) = decode(body(c.feed, d)) else { continue };
        book.apply(&m);
        if matches!(m, Msg::Other { .. }) {
            continue;
        }
        let loc = locate_of(&m);
        let s = c.slot_of[loc as usize];
        if s == NO_SLOT {
            continue;
        }
        let si = s as usize;
        let bid = pack(book.best(loc, Side::Buy));
        let ask = pack(book.best(loc, Side::Sell));
        if p.last[si] == (bid, ask) {
            continue;
        }
        p.last[si] = (bid, ask);
        p.ver[si] += 1;
        p.quotes += 1;
        let q = Quote { stamp: clock::counter(), bid, ask, ver: p.ver[si], slot: s, locate: loc };
        publish::<D>(q, tx, slots, &mut w);
    }
    w
}

/// The timed writer loop of `work=replay`: no feed and no book, only the
/// recorded updates published back to back.
#[inline(always)]
fn replay_pass<const D: u8>(c: &Ctx, p: &mut Published, tx: &mut [Tx], slots: &[Slot]) -> Waits {
    let mut w = Waits::default();
    for _ in 0..c.reps {
        for &(s, loc, bid, ask) in c.recs {
            let si = s as usize;
            p.last[si] = (bid, ask);
            p.ver[si] += 1;
            p.quotes += 1;
            let q = Quote { stamp: clock::counter(), bid, ask, ver: p.ver[si], slot: s, locate: loc };
            publish::<D>(q, tx, slots, &mut w);
        }
    }
    w
}

#[inline(never)]
fn write_pass_none(c: &Ctx, book: &mut L3Book, p: &mut Published, tx: &mut [Tx], slots: &[Slot]) -> Waits {
    write_pass::<NONE>(c, book, p, tx, slots)
}
#[inline(never)]
fn write_pass_copy(c: &Ctx, book: &mut L3Book, p: &mut Published, tx: &mut [Tx], slots: &[Slot]) -> Waits {
    write_pass::<COPY>(c, book, p, tx, slots)
}
#[inline(never)]
fn write_pass_snap(c: &Ctx, book: &mut L3Book, p: &mut Published, tx: &mut [Tx], slots: &[Slot]) -> Waits {
    write_pass::<SNAP>(c, book, p, tx, slots)
}
#[inline(never)]
fn replay_pass_none(c: &Ctx, p: &mut Published, tx: &mut [Tx], slots: &[Slot]) -> Waits {
    replay_pass::<NONE>(c, p, tx, slots)
}
#[inline(never)]
fn replay_pass_copy(c: &Ctx, p: &mut Published, tx: &mut [Tx], slots: &[Slot]) -> Waits {
    replay_pass::<COPY>(c, p, tx, slots)
}
#[inline(never)]
fn replay_pass_snap(c: &Ctx, p: &mut Published, tx: &mut [Tx], slots: &[Slot]) -> Waits {
    replay_pass::<SNAP>(c, p, tx, slots)
}

/// Untimed: every update the writer publishes in `work=book`, in order.
fn record(c: &Ctx, book: &mut L3Book) -> Vec<Rec> {
    book.clear();
    let mut last = vec![(0u64, 0u64); c.k];
    let mut out = Vec::new();
    for &d in c.descs {
        let Ok(m) = decode(body(c.feed, d)) else { continue };
        book.apply(&m);
        if matches!(m, Msg::Other { .. }) {
            continue;
        }
        let loc = locate_of(&m);
        let s = c.slot_of[loc as usize];
        if s == NO_SLOT {
            continue;
        }
        let bid = pack(book.best(loc, Side::Buy));
        let ask = pack(book.best(loc, Side::Sell));
        if last[s as usize] != (bid, ask) {
            last[s as usize] = (bid, ask);
            out.push((s, loc, bid, ask));
        }
    }
    out
}

fn read_copy(mut rx: Rx, k: usize, flags: &Flags, check: Option<&Table>) -> Seen {
    let _ = qos::set_current_thread(Qos::UserInteractive);
    let mut o = Seen::new(k);
    flags.ready.fetch_add(1, Ordering::Release);
    loop {
        let Some(q) = rx.pop() else {
            std::hint::spin_loop();
            continue;
        };
        // Version 0 is never published: it marks the end of the run.
        if q.ver == 0 {
            break;
        }
        let now = clock::counter();
        o.hist.record(now.saturating_sub(q.stamp));
        o.taken += 1;
        let s = q.slot as usize;
        if q.ver != o.ver[s] + 1 {
            o.order_errors += 1;
        }
        if let Some(t) = check {
            if expected(t, s, q.ver) != (q.bid, q.ask) {
                o.content_errors += 1;
            }
        }
        o.ver[s] = q.ver;
        o.last[s] = (q.bid, q.ask);
        o.sum = fold(o.sum, q.bid, q.ask);
    }
    o
}

fn read_snap(slots: &[Slot], flags: &Flags, check: Option<&Table>) -> Seen {
    let _ = qos::set_current_thread(Qos::UserInteractive);
    let mut o = Seen::new(slots.len());
    let mut seq = vec![0u64; slots.len()];
    flags.ready.fetch_add(1, Ordering::Release);
    loop {
        // Read before the pass: once the writer is done, this pass sees the
        // final version of every slot.
        let fin = flags.done.load(Ordering::Acquire);
        let mut any = false;
        for (j, sl) in slots.iter().enumerate() {
            if sl.seq() == seq[j] {
                continue;
            }
            match sl.read() {
                Some(r) => {
                    let now = clock::counter();
                    o.hist.record(now.saturating_sub(r.stamp));
                    o.taken += 1;
                    // r.seq > seq[j]: loads of one atomic never go back.
                    o.conflated += (r.seq - seq[j]) / 2 - 1;
                    seq[j] = r.seq;
                    let v = (r.seq / 2) as u32;
                    if let Some(t) = check {
                        if expected(t, j, v) != (r.bid, r.ask) {
                            o.content_errors += 1;
                        }
                    }
                    o.ver[j] = v;
                    o.last[j] = (r.bid, r.ask);
                    o.sum = fold(o.sum, r.bid, r.ask);
                    any = true;
                }
                None => o.retries += 1,
            }
        }
        if fin {
            break;
        }
        if !any {
            std::hint::spin_loop();
        }
    }
    o
}

struct Run {
    published: Published,
    ticks: u64,
    waits: Waits,
    allocations: u64,
    pmu: [u64; 4],
    invol: i64,
    seen: Vec<Seen>,
}

fn run(c: &Ctx, design: u8, readers: usize, book: &mut L3Book, check: Option<&Table>) -> Run {
    book.clear();
    let mut p = Box::new(Line(Published { last: vec![(0, 0); c.k], ver: vec![0; c.k], quotes: 0 }));
    let slots: Vec<Slot> = (0..c.k).map(|_| Slot::default()).collect();
    let (mut txs, mut rxs): (Vec<Tx>, Vec<Rx>) = (Vec::new(), Vec::new());
    if design == COPY {
        for _ in 0..readers {
            let (t, r) = ring::<Quote, AcqRel, Pad128, true>(c.ring);
            txs.push(t);
            rxs.push(r);
        }
    }
    let flags = Box::new(Line(Flags { ready: AtomicUsize::new(0), done: AtomicBool::new(false) }));
    let f = &flags.0;
    let u0 = Usage::now();
    let mut out = Run { published: p.0.clone(), ticks: 0, waits: Waits::default(), allocations: 0, pmu: [0; 4], invol: 0, seen: Vec::new() };
    thread::scope(|s| {
        let mut hs = Vec::new();
        if design == COPY {
            for r in rxs.drain(..) {
                let k = c.k;
                hs.push(s.spawn(move || read_copy(r, k, f, check)));
            }
        } else if design == SNAP {
            for _ in 0..readers {
                hs.push(s.spawn(|| read_snap(&slots, f, check)));
            }
        }
        while f.ready.load(Ordering::Acquire) < hs.len() {
            std::hint::spin_loop();
        }
        let pp = &mut p.0;
        let a0 = thread_allocations();
        let p0 = c.pmu.map(|s| s.read());
        let t0 = clock::counter();
        out.waits = match (design, c.recs.is_empty()) {
            (NONE, true) => write_pass_none(c, book, pp, &mut txs, &slots),
            (COPY, true) => write_pass_copy(c, book, pp, &mut txs, &slots),
            (_, true) => write_pass_snap(c, book, pp, &mut txs, &slots),
            (NONE, false) => replay_pass_none(c, pp, &mut txs, &slots),
            (COPY, false) => replay_pass_copy(c, pp, &mut txs, &slots),
            (_, false) => replay_pass_snap(c, pp, &mut txs, &slots),
        };
        let t1 = clock::counter();
        if let (Some(s), Some(a)) = (c.pmu, p0) {
            let b = s.read();
            let d = s.delta(&a, &b);
            out.pmu = [d[0], d[1], d[2], d[3]];
        }
        out.allocations = thread_allocations() - a0;
        out.ticks = t1 - t0;
        let stop = Quote::default();
        for t in txs.iter_mut() {
            while !t.push(stop) {
                std::hint::spin_loop();
            }
        }
        f.done.store(true, Ordering::Release);
        out.seen = hs.into_iter().map(|h| h.join().expect("reader thread")).collect();
    });
    out.invol = Usage::now().since(&u0).involuntary_switches;
    out.published = p.0;
    out
}

/// Abort unless the run published what the reference pass did and every
/// reader saw it as its design promises.
fn verify(label: &str, design: u8, r: &Run, want: &Published, copy_sum: u64) {
    assert_eq!(&r.published, want, "{label}: published updates differ from the reference pass");
    for (i, s) in r.seen.iter().enumerate() {
        assert_eq!(s.content_errors, 0, "{label} reader {i}: update content differs from the recorded sequence");
        assert_eq!(s.last, want.last, "{label} reader {i}: final top of book differs");
        assert_eq!(s.ver, want.ver, "{label} reader {i}: final versions differ");
        if design == COPY {
            assert_eq!(s.taken, want.quotes, "{label} reader {i}: copy reader lost updates");
            assert_eq!(s.order_errors, 0, "{label} reader {i}: copy reader saw versions out of order");
            assert_eq!(s.sum, copy_sum, "{label} reader {i}: copy reader content differs from the recorded sequence");
        } else {
            assert_eq!(s.taken + s.conflated, want.quotes, "{label} reader {i}: taken plus conflated differs from published");
        }
    }
}

fn report(feed: &str, design: &str, readers: usize, epoch: usize, c: &Ctx, r: &Run) {
    // Per feed message in `work=book`, per published update in `work=replay`.
    let units = if c.recs.is_empty() { c.descs.len() } else { c.recs.len() * c.reps };
    let n = units as f64;
    let freq = clock::counter_frequency() as f64;
    let secs = r.ticks as f64 / freq;
    let q = r.published.quotes;
    let mut line = format!(
        "fo feed={feed} work={} design={design} readers={readers} epoch={epoch} messages={units} quotes={q} quote_rate={:.7} \
         writer_ns_per_msg={:.3} full_waits={} wait_spins={} allocations={} invol_switches={}",
        if c.recs.is_empty() { "book" } else { "replay" },
        q as f64 / n,
        secs * 1e9 / n,
        r.waits.full,
        r.waits.spins,
        r.allocations,
        r.invol,
    );
    if c.pmu.is_some() {
        let per = |x: u64| x as f64 / n;
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
    if !r.seen.is_empty() {
        let mut all = Histogram::new();
        let (mut taken, mut conflated, mut retries, mut sum, mut worst) = (0u64, 0u64, 0u64, 0u64, 0u64);
        for s in &r.seen {
            all.merge(&s.hist);
            taken += s.taken;
            conflated += s.conflated;
            retries += s.retries;
            sum = sum.rotate_left(13).wrapping_add(s.sum);
            worst = worst.max(s.hist.quantile(0.99));
        }
        let ns = |t: u64| t as f64 * 1e9 / freq;
        line += &format!(
            " taken={taken} conflated={conflated} conflated_frac={:.6} retries={retries} stale_p50_ns={:.1} stale_p90_ns={:.1} \
             stale_p99_ns={:.1} stale_p999_ns={:.1} stale_max_ns={:.1} worst_reader_p99_ns={:.1} checksum={sum:016x}",
            conflated as f64 / (taken + conflated).max(1) as f64,
            ns(all.quantile(0.5)),
            ns(all.quantile(0.9)),
            ns(all.quantile(0.99)),
            ns(all.quantile(0.999)),
            ns(all.max()),
            ns(worst),
        );
    }
    println!("{line}");
}

fn kv<'a>(args: &'a [String], key: &str) -> Option<&'a str> {
    args.iter().find_map(|a| a.strip_prefix(key).and_then(|r| r.strip_prefix('=')))
}

fn num<T: std::str::FromStr>(args: &[String], key: &str, default: T) -> T {
    kv(args, key).and_then(|v| v.parse().ok()).unwrap_or(default)
}

/// Descriptors of the first `limit` messages, bytes used, maximum stock
/// locate, peak live orders.
fn index(buf: &[u8], limit: usize) -> (Vec<u64>, usize, u16, usize) {
    let mut r = Reader::new(buf);
    let mut descs = Vec::new();
    let mut max_locate = 0u16;
    let mut off = 0usize;
    let mut probe = RefBook::default();
    let mut peak = 0usize;
    while descs.len() < limit {
        match r.next_raw() {
            Some(Ok(b)) => {
                assert!(b.len() < 1 << 16 && off + 2 < 1 << 48);
                descs.push(((off as u64 + 2) << 16) | b.len() as u64);
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
    (descs, off, max_locate, peak)
}

/// Stock symbol of every locate from the stock directory messages (`R`);
/// locates without one get `L` and the locate number.
fn symbols(feed: &[u8], descs: &[u64], instruments: usize) -> Vec<String> {
    let mut out: Vec<String> = (0..instruments).map(|i| format!("L{i}")).collect();
    for &d in descs {
        let b = body(feed, d);
        if b.len() >= 19 && b[0] == b'R' {
            let loc = u16::from_be_bytes([b[1], b[2]]) as usize;
            if let Some(slot) = out.get_mut(loc) {
                *slot = String::from_utf8_lossy(&b[11..19]).trim().to_string();
            }
        }
    }
    out
}

/// The `k` locates with the most book messages in the window (ties: lower
/// locate first), and the share of all messages they account for.
fn subscriptions(feed: &[u8], descs: &[u64], instruments: usize, k: usize) -> (Vec<u16>, f64) {
    let mut count = vec![0u64; instruments];
    for &d in descs {
        if let Ok(m) = decode(body(feed, d)) {
            if !matches!(m, Msg::Other { .. }) {
                count[locate_of(&m) as usize] += 1;
            }
        }
    }
    let mut order: Vec<usize> = (0..instruments).filter(|&i| count[i] > 0).collect();
    order.sort_by(|&a, &b| count[b].cmp(&count[a]).then(a.cmp(&b)));
    order.truncate(k);
    let share = order.iter().map(|&i| count[i]).sum::<u64>() as f64 / descs.len().max(1) as f64;
    (order.into_iter().map(|i| i as u16).collect(), share)
}

fn parse_config(w: &str) -> (u8, &'static str, usize) {
    let (design, name, rest) = if w == "none" {
        return (NONE, "none", 0);
    } else if let Some(r) = w.strip_prefix("copy") {
        (COPY, "copy", r)
    } else if let Some(r) = w.strip_prefix("snap") {
        (SNAP, "snap", r)
    } else {
        panic!("unknown config {w}")
    };
    let n: usize = rest.parse().unwrap_or_else(|_| panic!("config {w}: reader count missing"));
    assert!(n <= MAX_READERS, "config {w}: at most {MAX_READERS} readers");
    assert!(design != COPY || n >= 1, "config {w}: copy needs at least one reader");
    (design, name, n)
}

fn main() {
    let args: Vec<String> = std::env::args().collect();
    let epochs: usize = num(&args, "epochs", 10);
    let k: usize = num(&args, "subs", 8);
    let ring_cap: usize = num(&args, "ring", 4096);
    assert!((1..NO_SLOT as usize).contains(&k), "subs must be at least 1");
    assert!(ring_cap.is_power_of_two(), "ring must be a power of two");
    let configs: Vec<(u8, &str, usize)> =
        kv(&args, "configs").unwrap_or("none,snap0,copy1,snap1,copy2,snap2,copy3,snap3").split(',').map(parse_config).collect();
    let replay = match kv(&args, "work") {
        None | Some("book") => false,
        Some("replay") => true,
        Some(o) => panic!("unknown work {o}"),
    };

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
            let name = std::path::Path::new(path).file_name().and_then(|f| f.to_str()).unwrap_or("file").replace('.', "_");
            (raw, name)
        }
        None => {
            let messages: usize = num(&args, "messages", 5_000_000);
            let instruments: u16 = num(&args, "instruments", 500);
            (feed(42, instruments, messages, Mix::default()), "synthetic_default".to_string())
        }
    };
    let limit: usize = num(&args, "limit", 20_000_000);
    let (descs, len, max_locate, peak) = index(&src, limit);
    let mut region = PinnedRegion::new(len.max(1)).expect("feed region");
    region.as_mut_slice()[..len].copy_from_slice(&src[..len]);
    drop(src);
    let locked = region.is_locked();
    let feed_buf = &region.as_mut_slice()[..len];
    let mut log2 = 10u32;
    while ((1usize << log2) / 2) < peak {
        log2 += 1;
    }
    let instruments = max_locate as usize + 1;
    let (subs, share) = subscriptions(feed_buf, &descs, instruments, k);
    let k = subs.len();
    let mut slot_of = vec![NO_SLOT; instruments];
    for (j, &l) in subs.iter().enumerate() {
        slot_of[l as usize] = j as u16;
    }
    let syms = symbols(feed_buf, &descs, instruments);
    println!(
        "fo_config feed={name} messages={} bytes={len} instruments={instruments} peak_live_orders={peak} order_table_slots={} \
         epochs={epochs} subs={k} ring={ring_cap} pmu={} feed_locked={locked} configs={}",
        descs.len(),
        1usize << log2,
        pmu.is_some(),
        configs.iter().map(|&(_, n, r)| if n == "none" { n.to_string() } else { format!("{n}{r}") }).collect::<Vec<_>>().join(","),
    );
    println!(
        "fo_subs feed={name} subs={k} book_message_share={share:.4} locates={} symbols={}",
        subs.iter().map(|l| l.to_string()).collect::<Vec<_>>().join(","),
        subs.iter().map(|&l| syms[l as usize].as_str()).collect::<Vec<_>>().join(","),
    );

    let mut book = L3Book::new(log2, instruments);
    let base = Ctx { recs: &[], reps: 1, feed: feed_buf, descs: &descs, slot_of: &slot_of, k, ring: ring_cap, pmu: pmu.as_ref() };
    // Reference pass of the feed (unmeasured) and the recorded updates
    // (untimed), checked against each other.
    let want_book = run(&base, NONE, 0, &mut book, None).published;
    let recs = record(&base, &mut book);
    assert!(!recs.is_empty(), "no update was published");
    let mut table: Vec<Vec<(u64, u64)>> = vec![Vec::new(); k];
    for &(s, _, bid, ask) in &recs {
        table[s as usize].push((bid, ask));
    }
    assert_eq!(recs.len() as u64, want_book.quotes, "recorded updates: count differs from the reference pass");
    for (s, t) in table.iter().enumerate() {
        assert_eq!(t.len(), want_book.ver[s] as usize, "recorded updates: versions of slot {s} differ");
        assert_eq!(t.last().copied().unwrap_or((0, 0)), want_book.last[s], "recorded updates: final top of book of slot {s}");
    }
    let c = if replay {
        let updates: usize = num(&args, "updates", 2_000_000);
        Ctx { recs: &recs, reps: updates.div_ceil(recs.len()), ..base }
    } else {
        base
    };
    let want = Published {
        last: want_book.last.clone(),
        ver: want_book.ver.iter().map(|&v| v * c.reps as u32).collect(),
        quotes: want_book.quotes * c.reps as u64,
    };
    // A copy reader folds every update in publication order.
    let mut copy_sum = 0u64;
    for _ in 0..c.reps {
        for &(_, _, bid, ask) in &recs {
            copy_sum = fold(copy_sum, bid, ask);
        }
    }
    println!(
        "fo_work feed={name} work={} feed_updates={} reps={} updates_per_pass={}",
        if replay { "replay" } else { "book" },
        want_book.quotes,
        c.reps,
        want.quotes
    );
    // One unmeasured pass of every config, readers checking every update's content.
    for &(d, n, r) in &configs {
        let x = run(&c, d, r, &mut book, Some(&table));
        verify(&format!("warm-up {n}{r}"), d, &x, &want, copy_sum);
    }
    for epoch in 0..epochs {
        for j in 0..configs.len() {
            let (d, n, r) = configs[(j + epoch) % configs.len()];
            let x = run(&c, d, r, &mut book, None);
            verify(&format!("epoch {epoch} {n}{r}"), d, &x, &want, copy_sum);
            report(&name, n, r, epoch, &c, &x);
        }
    }
}
