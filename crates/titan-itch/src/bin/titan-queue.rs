//! What queue position costs to know, and what it is worth knowing.
//!
//! The fourth source brief makes queue position one of its three headline
//! tasks and states the mechanism plainly: price priority first, then time
//! priority, so an order that arrives earlier at the same price is filled
//! first. It draws the conclusion that speed buys queue position. It also
//! asserts that high frequency firms cancel half of the orders they send.
//! Neither claim is measured anywhere in the brief; both are measurable
//! exactly from a real feed, because an ITCH feed names every order.
//!
//! Queue position, for an order arriving at a price, is the number of shares
//! already resting at that price on that side: under price-time priority
//! every one of them is filled before any of the arriving order. The
//! arriving order joins the back of its level, so the count is the level's
//! total after the add less the order's own size.
//!
//! The untimed pass follows every order from its arrival to its end and
//! records which of the two ways it ended: some of it executed, or none of
//! it did and the rest was cancelled or deleted. A replacement is a new
//! arrival, since ITCH assigns it a new reference and it loses its place in
//! the queue, which is the brief's own rule.
//!
//! Designs for the timed pass: `none`, which decodes and keeps no book, as
//! the floor; `book`, the book alone; `queue`, the book and the queue
//! position of every arriving order. The query on every add is a ceiling
//! rather than a working cost, since a participant needs the position of its
//! own orders only, and it is reported that way.
//!
//!   titan-queue [file=PATH] [limit=N] [instruments=M] [log2=K] [epochs=E] [pmu]

use std::collections::HashMap;
use std::hint::black_box;
use titan_itch::book::L3Book;
use titan_itch::decode::{decode, Msg, Reader};
use titan_itch::gen::{feed, Mix};
use titan_itch::Book;
use titan_platform::alloc_count::{thread_allocations, CountingAlloc};
use titan_platform::clock;
use titan_platform::kperf::{events, Kperf, Session};
use titan_platform::qos::{self, Qos};

#[global_allocator]
static A: CountingAlloc = CountingAlloc;

const STAGES: [&str; 3] = ["none", "book", "queue"];

/// Upper edges of the queue position buckets, in shares ahead. The first
/// bucket is the front of the queue, where nothing rests ahead at all.
const EDGES: [u64; 4] = [1, 100, 1_000, 10_000];
const BUCKETS: usize = EDGES.len() + 1;

#[inline(always)]
fn bucket(ahead: u64) -> usize {
    EDGES.iter().position(|&e| ahead < e).unwrap_or(EDGES.len())
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

/// One pass over the feed that decodes and does nothing else: the work the
/// two designs below share, so they can be measured above it.
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

/// One pass with the book. `QUEUE` adds the queue position of every arriving
/// order, which is the only difference between the two designs.
#[inline(always)]
fn run_book<const QUEUE: bool>(src: &[u8], book: &mut L3Book, limit: u64) -> (u64, u64) {
    let (mut n, mut sum) = (0u64, 0u64);
    let mut r = Reader::new(src);
    while n < limit {
        let Some(Ok(b)) = r.next_raw() else { break };
        n += 1;
        let Ok(m) = decode(b) else { continue };
        book.apply(&m);
        if QUEUE {
            // The arriving order is at the back of its level, so everything
            // else resting at its price is ahead of it.
            if let Msg::Add { locate, side, shares, price, .. } = m {
                let ahead = book.level_shares(locate, side, price).saturating_sub(shares as u64);
                sum = sum.rotate_left(7) ^ ahead;
            }
            if let Msg::Replace { locate, new, shares, price, .. } = m {
                if let Some((side, _)) = book.order_info(new) {
                    let ahead = book.level_shares(locate, side, price).saturating_sub(shares as u64);
                    sum = sum.rotate_left(7) ^ ahead;
                }
            }
        }
        if let Some(loc) = locate_of(&m) {
            sum = sum.rotate_left(7) ^ loc as u64;
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

    // Untimed: every order followed from arrival to end.
    let mut book = L3Book::new(log2, instruments);
    // order reference -> queue position bucket, size on arrival, shares executed
    let mut live: HashMap<u64, (usize, u32, u32)> = HashMap::with_capacity(1 << 20);
    let mut ended = [0u64; BUCKETS];
    let mut any_fill = [0u64; BUCKETS];
    let mut full_fill = [0u64; BUCKETS];
    let mut ahead_sum = [0u128; BUCKETS];
    let mut arrivals = [0u64; BUCKETS];
    let (mut adds, mut replaces, mut front) = (0u64, 0u64, 0u64);
    let mut r = Reader::new(&src);
    let mut n = 0u64;
    while n < limit {
        let Some(Ok(b)) = r.next_raw() else { break };
        n += 1;
        let Ok(m) = decode(b) else { continue };
        // Executions are credited before the book can remove the order. A
        // non-printable execution still fills the resting order, so both
        // execution messages count.
        if let Msg::Executed { order, shares, .. } | Msg::ExecutedWithPrice { order, shares, .. } = m {
            if let Some(e) = live.get_mut(&order) {
                e.2 += shares;
            }
        }
        // The side of a replacement comes from the order it replaces, which
        // is still live until the message is applied.
        let old_side = match m {
            Msg::Replace { old, .. } => book.order_info(old).map(|(s, _)| s),
            _ => None,
        };
        let ending: [u64; 2] = match m {
            Msg::Executed { order, .. } | Msg::ExecutedWithPrice { order, .. } => [order, 0],
            Msg::Cancel { order, .. } | Msg::Delete { order, .. } => [order, 0],
            Msg::Replace { old, .. } => [old, 0],
            _ => [0, 0],
        };
        book.apply(&m);
        // Arrivals, after the add so the level already holds the new order.
        let arrived = match m {
            Msg::Add { locate, order, side, shares, price, .. } => {
                adds += 1;
                Some((locate, order, side, shares, price))
            }
            Msg::Replace { locate, new, shares, price, .. } => {
                replaces += 1;
                old_side.map(|side| (locate, new, side, shares, price))
            }
            _ => None,
        };
        if let Some((locate, order, side, shares, price)) = arrived {
            let ahead = book.level_shares(locate, side, price).saturating_sub(shares as u64);
            let b = bucket(ahead);
            arrivals[b] += 1;
            ahead_sum[b] += ahead as u128;
            if ahead == 0 {
                front += 1;
            }
            live.insert(order, (b, shares, 0));
        }
        for o in ending {
            if o == 0 || book.order_info(o).is_some() {
                continue;
            }
            let Some((b, size, done)) = live.remove(&o) else { continue };
            ended[b] += 1;
            if done > 0 {
                any_fill[b] += 1;
            }
            if done >= size {
                full_fill[b] += 1;
            }
        }
    }
    let total_ended: u64 = ended.iter().sum();
    let total_any: u64 = any_fill.iter().sum();
    let total_full: u64 = full_fill.iter().sum();
    println!(
        "queue_config feed={name} messages={n} instruments={instruments} log2={log2} limit={limit} epochs={epochs} pmu={} adds={adds} replaces={replaces} arrived_at_front={front} orders_ended={total_ended} still_resting={} unknown={}",
        pmu.is_some(),
        live.len(),
        book.unknown,
    );
    let csv = |c: &[u64; BUCKETS]| c.iter().map(|x| x.to_string()).collect::<Vec<_>>().join(",");
    let rate = |a: &[u64; BUCKETS], b: &[u64; BUCKETS]| {
        (0..BUCKETS).map(|i| format!("{:.4}", a[i] as f64 / b[i].max(1) as f64)).collect::<Vec<_>>().join(",")
    };
    println!(
        "queue_pos feed={name} buckets=0,1_99,100_999,1000_9999,10000_up arrivals={} ended={} any_fill={} full_fill={} any_rate={} full_rate={} mean_ahead={}",
        csv(&arrivals),
        csv(&ended),
        csv(&any_fill),
        csv(&full_fill),
        rate(&any_fill, &ended),
        rate(&full_fill, &ended),
        (0..BUCKETS).map(|i| format!("{:.1}", ahead_sum[i] as f64 / arrivals[i].max(1) as f64)).collect::<Vec<_>>().join(","),
    );
    println!(
        "queue_cancel feed={name} orders_ended={total_ended} never_executed={} never_executed_rate={:.4} any_fill_rate={:.4} full_fill_rate={:.4}",
        total_ended - total_any,
        (total_ended - total_any) as f64 / total_ended.max(1) as f64,
        total_any as f64 / total_ended.max(1) as f64,
        total_full as f64 / total_ended.max(1) as f64,
    );
    drop(live);

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
            "book" => run_book::<false>(&src, book, limit),
            _ => run_book::<true>(&src, book, limit),
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
                "queue feed={name} design={design} epoch={epoch} messages={msgs} ns_per_msg={:.3} allocations_pass={allocs} checksum={sum:016x}",
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
