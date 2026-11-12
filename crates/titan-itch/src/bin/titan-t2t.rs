//! Single-core software tick-to-trade harness.
//!
//! One thread (QoS USER_INTERACTIVE) runs the trading path for every message:
//!
//!   ingress ring pop -> ITCH decode in place -> L3 book update ->
//!   best bid and ask of the instrument touched -> signal -> egress ring push
//!
//! The ingress ring carries zero-copy descriptors (`offset << 16 | length`)
//! into a pinned, pre-faulted feed buffer; the egress ring carries order
//! intents. Both rings are the titan-spsc ring with AcqRel ordering, 128-byte
//! index padding and index caching. The feed handler side (filling the
//! ingress ring) and the order gateway side (draining the egress ring) run on
//! the same thread outside the timed region, in batches of `batch` messages:
//! what is timed is the work of the trading path on data already in the
//! ingress ring. No network, NIC or kernel I/O is involved; the result is a
//! software path cost, not a wire-to-wire latency.
//!
//! The signal keeps a 64-bit state that every message updates from the book
//! (rotate and xor of best prices and sizes), and order sizes are taken from
//! that state, so each message's work depends on the previous one and none of
//! it can be hoisted or removed by the compiler. An intent is emitted when the
//! touched instrument's spread is positive and at most `spread` (ITCH price
//! units, 4 implied decimals) and one side shows more than twice the resting
//! size of the other; the intent takes the opposite best price.
//!
//! Stages (component ablation, each a separate monomorphised loop):
//!   ring    pop, push a trivial intent          (ring cost only)
//!   decode  pop, decode                         (+ decoder)
//!   book    pop, decode, book update            (+ book)
//!   full    pop, decode, book, signal, push     (the trading path)
//!   direct  as full, descriptors read from an array and intents written to
//!           an array instead of the rings       (full minus ring operations)
//!   risk    as full, every intent passes eight pre-trade checks of the kind
//!           SEC Rule 15c3-5 requires (kill switch, size, price collar,
//!           notional, position, credit, duplicate, rate) before the push
//!   checks  the eight checks alone: an untimed pass records every intent
//!           the trading path emits with its mid and feed time, and the
//!           timed loop replays only those through the checks; reported
//!           per order, not per message
//!   replay  the same replay loop with the checks replaced by a fold of the
//!           order fields (loop and load floor for `checks`)
//!   ouch    each recorded order encoded as an OUCH 4.2 Enter Order (binary,
//!           49 bytes)
//!   fix     each recorded order encoded as a FIX 4.2 NewOrderSingle
//!           (tag=value, BodyLength and CheckSum computed)
//!
//! Timing: the generic counter around each batch, and with `pmu` the core
//! cycle, instruction, L1D load miss and branch mispredict counters around
//! each batch, minus the median cost of one counter read pair. Per message
//! values are batch means; the generic counter's 41.67 ns step and the cost
//! of a counter read (about 2,500 cycles) rule out timing single messages.
//! Batch percentiles describe the spread of batch means, not per-message tail
//! latency.
//!
//!   titan-t2t [messages=N] [instruments=M] [epochs=E] [batch=K] [spread=S] [stages=a,b] [limits=open] [pmu]
//!   titan-t2t file=PATH [limit=N] [epochs=E] [batch=K] [spread=S] [stages=a,b] [limits=open] [pmu]
//!
//! In `checks`, `replay`, `ouch` and `fix`, `messages=` and every per-message field of the
//! report count recorded orders, not feed messages.
//!
//! `limits=open` sets risk limits that no order can breach, so every check
//! runs to the end and every order is sent: `risk` minus `full` is then the
//! cost of the checks alone.

use std::hint::black_box;
use titan_itch::book::L3Book;
use titan_itch::decode::{decode, Msg, Reader};
use titan_itch::gen::{feed, Mix};
use titan_itch::reference::RefBook;
use std::sync::atomic::AtomicBool;
use titan_itch::gateway::{fix42_new_order, ouch42_enter, Session as OrderSession, FIX_MAX, OUCH42_ENTER_LEN};
use titan_itch::path::{locate_of, signal, timestamp, Intent, Limits, Risk, REJECT_NAMES};
use titan_itch::{Book, Side};
use titan_platform::alloc_count::{thread_allocations, CountingAlloc};
use titan_platform::clock;
use titan_platform::kperf::{events, Kperf, Session};
use titan_platform::mem::PinnedRegion;
use titan_platform::qos::{self, Qos};
use titan_spsc::order::AcqRel;
use titan_spsc::pad::Pad128;
use titan_spsc::ring::{ring, Consumer, Producer};

#[global_allocator]
static A: CountingAlloc = CountingAlloc;

type Ingress = (Producer<u64, AcqRel, Pad128, true>, Consumer<u64, AcqRel, Pad128, true>);
type Egress = (Producer<Intent, AcqRel, Pad128, true>, Consumer<Intent, AcqRel, Pad128, true>);

const RING: u8 = 0;
const DECODE: u8 = 1;
const BOOK: u8 = 2;
const FULL: u8 = 3;
const DIRECT: u8 = 4;
const RISK: u8 = 5;
const CHECKS: u8 = 6;
const REPLAY: u8 = 7;
const OUCH: u8 = 8;
const FIX: u8 = 9;
const STAGES: [(u8, &str); 10] = [
    (RING, "ring"),
    (DECODE, "decode"),
    (BOOK, "book"),
    (FULL, "full"),
    (DIRECT, "direct"),
    (RISK, "risk"),
    (CHECKS, "checks"),
    (REPLAY, "replay"),
    (OUCH, "ouch"),
    (FIX, "fix"),
];

/// Stock symbol of every locate, from the feed's stock directory messages
/// (`R`); locates without one get `L` and the locate number.
fn symbols(feed: &[u8], descs: &[u64], instruments: usize) -> Vec<[u8; 8]> {
    let mut out: Vec<[u8; 8]> = (0..instruments)
        .map(|i| {
            let mut s = [b' '; 8];
            let t = format!("L{i}");
            s[..t.len().min(8)].copy_from_slice(&t.as_bytes()[..t.len().min(8)]);
            s
        })
        .collect();
    for &d in descs {
        let b = body(feed, d);
        if b.len() >= 19 && b[0] == b'R' {
            let loc = u16::from_be_bytes([b[1], b[2]]) as usize;
            if let Some(slot) = out.get_mut(loc) {
                slot.copy_from_slice(&b[11..19]);
            }
        }
    }
    out
}

/// Timed part of `ouch` and `fix`: encode every order, fold the bytes
/// written. Returns orders encoded.
#[inline(never)]
fn encode<const S: u8>(orders: &[Order], syms: &[[u8; 8]], seq: &mut u64, sum: &mut u64, s: &OrderSession) -> u64 {
    let mut ob = [0u8; OUCH42_ENTER_LEN];
    let mut fb = [0u8; FIX_MAX];
    for o in orders {
        let o = black_box(o);
        let sym = &syms[o.it.locate as usize];
        *seq += 1;
        let (last, n) = if S == OUCH {
            ouch42_enter(&mut ob, &o.it, sym, *seq, s);
            black_box(&ob);
            (ob[OUCH42_ENTER_LEN - 1], OUCH42_ENTER_LEN)
        } else {
            let n = fix42_new_order(&mut fb, &o.it, sym, *seq, o.ts, s);
            black_box(&fb);
            (fb[n - 4], n)
        };
        *sum = sum.rotate_left(7) ^ last as u64 ^ ((n as u64) << 8);
    }
    orders.len() as u64
}

/// One pass of the recorded orders through an encoder stage.
fn pass_encode<const S: u8>(c: &Ctx, orders: &[Order], syms: &[[u8; 8]], st: &mut Stats) {
    let session = OrderSession::default();
    let mut seq = 0u64;
    for chunk in orders.chunks(c.k) {
        let a0 = thread_allocations();
        let p0 = c.pmu.map(|s| s.read());
        let t0 = clock::counter();
        let n = encode::<S>(chunk, syms, &mut seq, &mut st.checksum, &session);
        let t1 = clock::counter();
        let p1 = c.pmu.map(|s| s.read());
        st.allocations += thread_allocations() - a0;
        let m = chunk.len() as f64;
        st.ticks += t1 - t0;
        st.batch_ns.push((t1 - t0) as f64 * 1e9 / clock::counter_frequency() as f64 / m);
        if let (Some(s), Some(a), Some(b)) = (c.pmu, p0, p1) {
            let d = s.delta(&a, &b);
            for ((acc, &x), &f) in st.pmu.iter_mut().zip(&d).zip(&c.pmu_floor) {
                *acc += x.saturating_sub(f);
            }
            st.batch_cycles.push(d[0].saturating_sub(c.pmu_floor[0]) as f64 / m);
        }
        st.messages += chunk.len() as u64;
        st.intents += n;
    }
}

/// One recorded order for the `checks` and `replay` stages.
#[derive(Clone, Copy)]
struct Order {
    it: Intent,
    mid: u64,
    ts: u64,
}

/// Untimed: every intent the trading path emits, with its mid and feed time.
fn record(feed: &[u8], descs: &[u64], book: &mut L3Book, max_spread: u32) -> Vec<Order> {
    book.clear();
    let mut state = 0x9E37_79B9_7F4A_7C15u64;
    let mut orders = Vec::new();
    for &d in descs {
        let b = body(feed, d);
        let Ok(m) = decode(b) else {
            state = state.rotate_left(1);
            continue;
        };
        book.apply(&m);
        if matches!(m, Msg::Other { .. }) {
            state = state.rotate_left(1);
            continue;
        }
        let loc = locate_of(&m);
        let bid = book.best(loc, Side::Buy);
        let ask = book.best(loc, Side::Sell);
        if let Some(it) = signal(&mut state, loc, bid, ask, max_spread) {
            let mid = match (bid, ask) {
                (Some((bp, _)), Some((ap, _))) => (bp as u64 + ap as u64) >> 1,
                _ => 0,
            };
            orders.push(Order { it, mid, ts: timestamp(b) });
        }
    }
    orders
}

/// Timed part of `checks` (S = CHECKS) and `replay` (S = REPLAY): returns
/// orders accepted, and folds them into `sum`.
#[inline(never)]
fn replay<const S: u8>(orders: &[Order], risk: &mut Risk, sum: &mut u64) -> u64 {
    let mut n = 0u64;
    for o in orders {
        let o = black_box(o);
        if S == CHECKS && !risk.check(&o.it, o.mid, o.ts) {
            continue;
        }
        *sum = sum.rotate_left(7) ^ o.it.price as u64 ^ ((o.it.shares as u64) << 32) ^ o.it.locate as u64 ^ o.it.buy as u64;
        n += 1;
    }
    n
}

/// One pass of the recorded orders through stage `S` (CHECKS or REPLAY).
fn pass_orders<const S: u8>(c: &Ctx, orders: &[Order], risk: &mut Risk, st: &mut Stats) {
    risk.reset();
    for chunk in orders.chunks(c.k) {
        let a0 = thread_allocations();
        let p0 = c.pmu.map(|s| s.read());
        let t0 = clock::counter();
        let n = replay::<S>(chunk, risk, &mut st.checksum);
        let t1 = clock::counter();
        let p1 = c.pmu.map(|s| s.read());
        st.allocations += thread_allocations() - a0;
        let m = chunk.len() as f64;
        st.ticks += t1 - t0;
        st.batch_ns.push((t1 - t0) as f64 * 1e9 / clock::counter_frequency() as f64 / m);
        if let (Some(s), Some(a), Some(b)) = (c.pmu, p0, p1) {
            let d = s.delta(&a, &b);
            for ((acc, &x), &f) in st.pmu.iter_mut().zip(&d).zip(&c.pmu_floor) {
                *acc += x.saturating_sub(f);
            }
            st.batch_cycles.push(d[0].saturating_sub(c.pmu_floor[0]) as f64 / m);
        }
        st.messages += chunk.len() as u64;
        st.intents += n;
    }
    if S == CHECKS {
        st.accepted = risk.accepted;
        st.rejected = risk.rejected;
    }
}

#[inline(always)]
fn body(feed: &[u8], d: u64) -> &[u8] {
    let off = (d >> 16) as usize;
    &feed[off..off + (d & 0xffff) as usize]
}

/// Process `k` messages of one batch; returns intents emitted.
#[allow(clippy::too_many_arguments)]
#[inline(never)]
fn batch<const S: u8>(
    feed: &[u8],
    descs: &[u64],
    ing: &mut Consumer<u64, AcqRel, Pad128, true>,
    eg: &mut Producer<Intent, AcqRel, Pad128, true>,
    out: &mut [Intent],
    book: &mut L3Book,
    risk: &mut Risk,
    state: &mut u64,
    max_spread: u32,
) -> u64 {
    let mut emitted = 0u64;
    for (i, &dd) in descs.iter().enumerate() {
        let d = if S == DIRECT { dd } else { ing.pop().expect("ingress filled for the batch") };
        if S == RING {
            let ok = eg.push(Intent { price: d as u32, shares: 0, locate: (d >> 16) as u16, buy: false });
            debug_assert!(ok);
            emitted += 1;
            continue;
        }
        let b = body(feed, d);
        let Ok(m) = decode(b) else {
            *state = state.rotate_left(1);
            continue;
        };
        if S == DECODE {
            *state = state.rotate_left(5) ^ locate_of(&m) as u64 ^ (d >> 16);
            continue;
        }
        book.apply(&m);
        let loc = locate_of(&m);
        if S == BOOK {
            *state = state.rotate_left(5) ^ loc as u64 ^ book.live_orders() as u64;
            continue;
        }
        if matches!(m, Msg::Other { .. }) {
            *state = state.rotate_left(1);
            continue;
        }
        let bid = book.best(loc, Side::Buy);
        let ask = book.best(loc, Side::Sell);
        if let Some(it) = signal(state, loc, bid, ask, max_spread) {
            if S == RISK {
                // The signal only fires with both sides present.
                let mid = match (bid, ask) {
                    (Some((bp, _)), Some((ap, _))) => (bp as u64 + ap as u64) >> 1,
                    _ => 0,
                };
                if !risk.check(&it, mid, timestamp(b)) {
                    continue;
                }
            }
            if S == DIRECT {
                out[i] = it;
            } else {
                let ok = eg.push(it);
                debug_assert!(ok);
            }
            emitted += 1;
        }
    }
    emitted
}

struct Stats {
    messages: u64,
    intents: u64,
    ticks: u64,
    allocations: u64,
    pmu: [u64; 4],
    batch_ns: Vec<f64>,
    batch_cycles: Vec<f64>,
    checksum: u64,
    accepted: u64,
    rejected: [u64; 8],
}

struct Ctx<'a> {
    feed: &'a [u8],
    descs: &'a [u64],
    k: usize,
    max_spread: u32,
    pmu: Option<&'a Session>,
    /// Median cost of one counter read pair, per event.
    pmu_floor: [u64; 4],
}

/// One full pass of the feed through stage `S`.
#[allow(clippy::too_many_arguments)]
fn pass<const S: u8>(
    c: &Ctx,
    ing: &mut Ingress,
    eg: &mut Egress,
    out: &mut [Intent],
    book: &mut L3Book,
    risk: &mut Risk,
    st: &mut Stats,
) {
    book.clear();
    risk.reset();
    let mut state = 0x9E37_79B9_7F4A_7C15u64;
    for chunk in c.descs.chunks(c.k) {
        if S != DIRECT {
            for &d in chunk {
                assert!(ing.0.push(d), "ingress ring sized to the batch");
            }
        }
        let a0 = thread_allocations();
        let p0 = c.pmu.map(|s| s.read());
        let t0 = clock::counter();
        let n = batch::<S>(c.feed, chunk, &mut ing.1, &mut eg.0, out, book, risk, &mut state, c.max_spread);
        let t1 = clock::counter();
        let p1 = c.pmu.map(|s| s.read());
        st.allocations += thread_allocations() - a0;
        let m = chunk.len() as f64;
        st.ticks += t1 - t0;
        st.batch_ns.push((t1 - t0) as f64 * 1e9 / clock::counter_frequency() as f64 / m);
        if let (Some(s), Some(a), Some(b)) = (c.pmu, p0, p1) {
            let d = s.delta(&a, &b);
            for ((acc, &x), &f) in st.pmu.iter_mut().zip(&d).zip(&c.pmu_floor) {
                *acc += x.saturating_sub(f);
            }
            st.batch_cycles.push(d[0].saturating_sub(c.pmu_floor[0]) as f64 / m);
        }
        st.messages += chunk.len() as u64;
        st.intents += n;
        // Gateway side: drain and fold the intents into the checksum.
        if S == DIRECT {
            for it in out[..chunk.len()].iter_mut() {
                if it.shares != 0 {
                    st.checksum = st.checksum.rotate_left(7) ^ it.price as u64 ^ ((it.shares as u64) << 32) ^ it.locate as u64 ^ it.buy as u64;
                    *it = Intent::default();
                }
            }
        } else {
            let mut drained = 0u64;
            while let Some(it) = eg.1.pop() {
                st.checksum = st.checksum.rotate_left(7) ^ it.price as u64 ^ ((it.shares as u64) << 32) ^ it.locate as u64 ^ it.buy as u64;
                drained += 1;
            }
            assert_eq!(drained, n, "egress count");
            assert!(ing.1.pop().is_none(), "ingress drained by the batch");
        }
    }
    st.checksum ^= black_box(state);
    if S == RISK {
        st.accepted = risk.accepted;
        st.rejected = risk.rejected;
    }
}

fn pct(v: &mut [f64], q: f64) -> f64 {
    if v.is_empty() {
        return f64::NAN;
    }
    v.sort_by(|a, b| a.partial_cmp(b).unwrap());
    let i = ((v.len() - 1) as f64 * q).round() as usize;
    v[i]
}

fn report(feed_name: &str, stage: &str, epoch: usize, with_risk: bool, st: &mut Stats) {
    let n = st.messages as f64;
    let secs = st.ticks as f64 / clock::counter_frequency() as f64;
    let ns = secs * 1e9 / n;
    let mut line = format!(
        "t2t feed={feed_name} stage={stage} epoch={epoch} messages={} batches={} ns_per_msg={ns:.3} mmsg_per_s={:.2} \
         batch_p50_ns={:.3} batch_p99_ns={:.3} batch_max_ns={:.3} intents={} intent_rate={:.5} allocations={} checksum={:016x}",
        st.messages,
        st.batch_ns.len(),
        1e3 / ns,
        pct(&mut st.batch_ns, 0.5),
        pct(&mut st.batch_ns, 0.99),
        pct(&mut st.batch_ns, 1.0),
        st.intents,
        st.intents as f64 / n,
        st.allocations,
        st.checksum,
    );
    if with_risk {
        line += &format!(" accepted={}", st.accepted);
        for (name, r) in REJECT_NAMES.iter().zip(st.rejected) {
            line += &format!(" reject_{name}={r}");
        }
    }
    if !st.batch_cycles.is_empty() {
        let per = |x: u64| x as f64 / n;
        line += &format!(
            " msg_cycles={:.2} msg_instructions={:.2} msg_l1d_miss={:.4} msg_br_mispred={:.4} ipc={:.3} core_ghz={:.3} batch_p50_cycles={:.2} batch_p99_cycles={:.2}",
            per(st.pmu[0]),
            per(st.pmu[1]),
            per(st.pmu[2]),
            per(st.pmu[3]),
            st.pmu[1] as f64 / st.pmu[0] as f64,
            st.pmu[0] as f64 / secs / 1e9,
            pct(&mut st.batch_cycles, 0.5),
            pct(&mut st.batch_cycles, 0.99),
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

/// Descriptors of the first `limit` messages, maximum stock locate, peak live orders.
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

fn main() {
    let args: Vec<String> = std::env::args().collect();
    let epochs: usize = num(&args, "epochs", 5);
    let k: usize = num(&args, "batch", 4096);
    let max_spread: u32 = num(&args, "spread", 300);
    assert!(k.is_power_of_two(), "batch must be a power of two");

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

    let open_limits = kv(&args, "limits") == Some("open");
    let mut floor = [0u64; 4];
    if let Some(s) = pmu.as_ref() {
        let rows = s.read_overhead(201);
        for (e, f) in floor.iter_mut().enumerate() {
            let mut col: Vec<u64> = rows.iter().map(|r| r[e]).collect();
            col.sort_unstable();
            *f = col[col.len() / 2];
        }
    }
    println!(
        "t2t_config feed={name} messages={} bytes={len} instruments={} peak_live_orders={peak} order_table_slots={} \
         epochs={epochs} batch={k} spread={max_spread} limits={} pmu={} feed_locked={} pmu_read_pair_cycles={} pmu_read_pair_instructions={}",
        descs.len(),
        max_locate as usize + 1,
        1usize << log2,
        if open_limits { "open" } else { "default" },
        pmu.is_some(),
        locked,
        floor[0],
        floor[1],
    );

    let c = Ctx { feed: feed_buf, descs: &descs, k, max_spread, pmu: pmu.as_ref(), pmu_floor: floor };
    let mut ing: Ingress = ring(k);
    let mut eg: Egress = ring(k);
    let mut out = vec![Intent::default(); k];
    let mut book = L3Book::new(log2, max_locate as usize + 1);
    let batches = descs.len().div_ceil(k);
    let fresh = || Stats {
        messages: 0,
        intents: 0,
        ticks: 0,
        allocations: 0,
        pmu: [0; 4],
        batch_ns: Vec::with_capacity(batches),
        batch_cycles: Vec::with_capacity(batches),
        checksum: 0,
        accepted: 0,
        rejected: [0; 8],
    };

    let kill = AtomicBool::new(false);
    let limits = if open_limits { Limits::open() } else { Limits::default() };
    let mut risk = Risk::new(max_locate as usize + 1, limits, &kill);
    let orders = record(feed_buf, &descs, &mut book, max_spread);
    let syms = symbols(feed_buf, &descs, max_locate as usize + 1);
    println!("t2t_orders feed={name} orders={} per_message={:.5}", orders.len(), orders.len() as f64 / descs.len() as f64);
    let mut run = |stage: u8, ing: &mut Ingress, eg: &mut Egress, out: &mut [Intent], book: &mut L3Book, st: &mut Stats| {
        let r = &mut risk;
        match stage {
            CHECKS => pass_orders::<CHECKS>(&c, &orders, r, st),
            REPLAY => pass_orders::<REPLAY>(&c, &orders, r, st),
            OUCH => pass_encode::<OUCH>(&c, &orders, &syms, st),
            FIX => pass_encode::<FIX>(&c, &orders, &syms, st),
            RING => pass::<RING>(&c, ing, eg, out, book, r, st),
            DECODE => pass::<DECODE>(&c, ing, eg, out, book, r, st),
            BOOK => pass::<BOOK>(&c, ing, eg, out, book, r, st),
            FULL => pass::<FULL>(&c, ing, eg, out, book, r, st),
            DIRECT => pass::<DIRECT>(&c, ing, eg, out, book, r, st),
            _ => pass::<RISK>(&c, ing, eg, out, book, r, st),
        }
    };

    let chosen: Vec<(u8, &str)> = match kv(&args, "stages") {
        Some(list) => list
            .split(',')
            .map(|w| *STAGES.iter().find(|(_, l)| *l == w).unwrap_or_else(|| panic!("unknown stage {w}")))
            .collect(),
        None => STAGES.to_vec(),
    };

    // Warm-up: one unmeasured pass of every stage (grows level vectors, warms code).
    for &(s, _) in &chosen {
        let mut st = fresh();
        run(s, &mut ing, &mut eg, &mut out, &mut book, &mut st);
    }
    let mut full_sum = None;
    for epoch in 0..epochs {
        for j in 0..chosen.len() {
            let (s, label) = chosen[(j + epoch) % chosen.len()];
            let mut st = fresh();
            run(s, &mut ing, &mut eg, &mut out, &mut book, &mut st);
            if (s == CHECKS && open_limits) || s == REPLAY {
                // Every recorded order passes open limits.
                assert_eq!(st.intents as usize, orders.len(), "{label}: every recorded order must pass");
            }
            if s == FULL || s == DIRECT || (s == RISK && open_limits) {
                // full, direct, and risk with limits nobody breaches run the
                // same trading path: their intents must agree.
                let key = (st.intents, st.checksum);
                match full_sum {
                    None => full_sum = Some(key),
                    Some(prev) => assert_eq!(prev, key, "full and direct disagree"),
                }
            }
            report(&name, label, epoch, s == RISK || s == CHECKS, &mut st);
        }
    }
}
