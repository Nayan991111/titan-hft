//! What a consolidated book across two exchanges costs, and how often the
//! two disagree.
//!
//! Both feeds are replayed into one stream ordered by the 6-byte timestamp
//! every ITCH 5.0 message carries, each into its own L3 book. The venues
//! number their instruments independently, so the symbols of the two Stock
//! Directories are intersected first and each shared symbol gets one
//! identifier; messages for anything else still build their venue's book but
//! are not consolidated.
//!
//! Designs, per merged message:
//! * `floor`: the two venue books alone, no consolidated view.
//! * `incr`: after a message, re-read the top of that instrument on that
//!   venue and refold its consolidated top (two `best` calls per message).
//! * `rebuild`: keep no per-instrument state and re-read every venue's top
//!   for every shared instrument every `every` messages.
//!
//! Untimed, once, over as much of both days as `stats_limit` allows: the
//! same replay with the incremental design, accumulating how long the
//! consolidated market was locked or crossed inside the window, and what the
//! periodic design would have missed: changes it had not yet seen, the time
//! it stayed behind, and crossed episodes that began and ended between two
//! of its rebuilds. Timed, per design, `epochs` replays of the first `limit`
//! merged messages, order rotated. After every incremental pass the harness
//! rebuilds the consolidated view from the books and requires it to equal
//! the view kept update by update.
//!
//!   titan-cross a=PATH b=PATH [limit=N] [stats_limit=N] [from=SEC] [to=SEC]
//!               [every=K] [epochs=E] [pmu]

use std::collections::HashMap;
use std::hint::black_box;
use titan_itch::book::L3Book;
use titan_itch::combine::{Consolidated, CrossWatch, Market, Quote, VENUES};
use titan_itch::decode::{decode, Msg, Reader};
use titan_itch::{Book, Side};
use titan_platform::alloc_count::{thread_allocations, CountingAlloc};
use titan_platform::clock;
use titan_platform::kperf::{events, Kperf, Session};
use titan_platform::qos::{self, Qos};

#[global_allocator]
static A: CountingAlloc = CountingAlloc;

const DESIGNS: [&str; 3] = ["floor", "incr", "rebuild"];
const NONE: u32 = u32::MAX;

/// Nanoseconds since midnight: every ITCH 5.0 message carries a 6-byte
/// timestamp after the type, stock locate and tracking number.
#[inline(always)]
fn ts_of(b: &[u8]) -> u64 {
    if b.len() < 11 {
        return 0;
    }
    let mut t = 0u64;
    for &c in &b[5..11] {
        t = (t << 8) | c as u64;
    }
    t
}

/// Stock locate of a book message.
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
fn quote(b: &L3Book, loc: u16) -> Quote {
    Quote { bid: b.best(loc, Side::Buy).map_or(0, |t| t.0), ask: b.best(loc, Side::Sell).map_or(u32::MAX, |t| t.0) }
}

/// Symbol of every Stock Directory ('R') message, and the highest locate.
fn directory(src: &[u8]) -> (HashMap<[u8; 8], u16>, usize) {
    let mut map = HashMap::new();
    let mut top = 0usize;
    let mut r = Reader::new(src);
    while let Some(Ok(b)) = r.next_raw() {
        if b.len() >= 3 {
            top = top.max(u16::from_be_bytes([b[1], b[2]]) as usize);
        }
        if b.len() >= 19 && b[0] == b'R' {
            let mut s = [0u8; 8];
            s.copy_from_slice(&b[11..19]);
            map.insert(s, u16::from_be_bytes([b[1], b[2]]));
        }
    }
    (map, top)
}

/// Lookups built once from the two directories.
struct Map {
    /// Venue locate to shared identifier, or `NONE`.
    gid: [Vec<u32>; VENUES],
    /// Shared identifier to venue locate.
    loc: [Vec<u16>; VENUES],
    symbols: Vec<[u8; 8]>,
}

/// The two venue books and the consolidated view, rebuilt for every pass.
struct State {
    books: [L3Book; VENUES],
    cons: Consolidated,
}

impl State {
    fn new(log2: u32, instruments: [usize; VENUES], shared: usize) -> State {
        State {
            books: [L3Book::new(log2, instruments[0]), L3Book::new(log2, instruments[1])],
            cons: Consolidated::new(shared),
        }
    }

    /// Back to an empty state, keeping the pinned order tables.
    fn reset(&mut self) {
        for b in &mut self.books {
            b.clear();
        }
        self.cons.clear();
    }
}

/// One merged replay. `D` is the index into [`DESIGNS`]. Returns the number
/// of merged messages and a value folded from the books, so no pass can be
/// optimised away and every design folds the same thing.
#[inline(always)]
fn replay<const D: u8>(src: [&[u8]; VENUES], map: &Map, st: &mut State, limit: u64, every: u64) -> (u64, u64) {
    let mut rd = [Reader::new(src[0]), Reader::new(src[1])];
    let mut next: [Option<&[u8]>; VENUES] = [None, None];
    for v in 0..VENUES {
        next[v] = match rd[v].next_raw() {
            Some(Ok(b)) => Some(b),
            _ => None,
        };
    }
    let mut msgs = 0u64;
    while msgs < limit {
        let which = match (next[0], next[1]) {
            (Some(x), Some(y)) => usize::from(ts_of(y) < ts_of(x)),
            (Some(_), None) => 0,
            (None, Some(_)) => 1,
            (None, None) => break,
        };
        let b = next[which].expect("chosen side is present");
        next[which] = match rd[which].next_raw() {
            Some(Ok(n)) => Some(n),
            _ => None,
        };
        msgs += 1;
        let Ok(m) = decode(b) else { continue };
        st.books[which].apply(&m);
        if D == 0 {
            continue;
        }
        if D == 1 {
            if let Some(loc) = locate_of(&m) {
                let gid = map.gid[which][loc as usize];
                if gid != NONE {
                    let q = quote(&st.books[which], loc);
                    st.cons.set(gid as usize, which, q);
                }
            }
        } else if msgs.is_multiple_of(every) {
            let books = &st.books;
            let loc = &map.loc;
            st.cons.rebuild_from(|gid, v| quote(&books[v], loc[v][gid]));
        }
    }
    (msgs, (st.books[0].live_orders() as u64).wrapping_mul(31).wrapping_add(st.books[1].live_orders() as u64))
}

/// Everything the untimed pass measures.
#[derive(Default)]
struct Stats {
    msgs: u64,
    in_window: u64,
    changes: u64,
    /// Changes the periodic design had not yet seen, and how long it stayed
    /// behind on them (summed over instruments).
    stale_changes: u64,
    stale_ns: u128,
    max_stale_ns: u64,
    rebuilds: u64,
    /// Crossed episodes that began and ended between two rebuilds.
    invisible_crossed: u64,
    first_ts: u64,
    last_ts: u64,
}

/// The untimed pass: the incremental view, the locked and crossed episodes
/// inside the window, and what the periodic design would have missed.
fn measure(src: [&[u8]; VENUES], map: &Map, st: &mut State, limit: u64, every: u64, from: u64, to: u64) -> (Stats, CrossWatch) {
    let shared = map.symbols.len();
    let mut watch = CrossWatch::new(shared);
    let mut s = Stats::default();
    let mut state = vec![Market::OneSided; shared];
    let mut cross_at = vec![0u64; shared];
    let mut pending_since = vec![0u64; shared];
    let mut pending: Vec<usize> = Vec::new();
    let mut rebuild_idx = 1u64;
    let mut rd = [Reader::new(src[0]), Reader::new(src[1])];
    let mut next: [Option<&[u8]>; VENUES] = [None, None];
    for v in 0..VENUES {
        next[v] = match rd[v].next_raw() {
            Some(Ok(b)) => Some(b),
            _ => None,
        };
    }
    let mut ts = 0u64;
    while s.msgs < limit {
        let which = match (next[0], next[1]) {
            (Some(x), Some(y)) => usize::from(ts_of(y) < ts_of(x)),
            (Some(_), None) => 0,
            (None, Some(_)) => 1,
            (None, None) => break,
        };
        let b = next[which].expect("chosen side is present");
        next[which] = match rd[which].next_raw() {
            Some(Ok(n)) => Some(n),
            _ => None,
        };
        s.msgs += 1;
        ts = ts_of(b).max(ts);
        if s.first_ts == 0 {
            s.first_ts = ts;
        }
        s.last_ts = ts;
        let Ok(m) = decode(b) else { continue };
        st.books[which].apply(&m);
        let window = (from..to).contains(&ts);
        if window {
            s.in_window += 1;
        }
        if let Some(loc) = locate_of(&m) {
            let gid = map.gid[which][loc as usize];
            if gid != NONE {
                let gid = gid as usize;
                let before = st.cons.top(gid);
                let q = quote(&st.books[which], loc);
                if st.cons.set(gid, which, q) && st.cons.top(gid) != before {
                    s.changes += 1;
                    if pending_since[gid] == 0 {
                        pending_since[gid] = ts.max(1);
                        pending.push(gid);
                        s.stale_changes += 1;
                    }
                    if window {
                        let now = st.cons.top(gid).market();
                        if state[gid] != now {
                            if state[gid] == Market::Crossed && cross_at[gid] == rebuild_idx {
                                s.invisible_crossed += 1;
                            }
                            if now == Market::Crossed {
                                cross_at[gid] = rebuild_idx;
                            }
                            state[gid] = now;
                        }
                        watch.observe(gid, st.cons.top(gid), ts);
                    }
                }
            }
        }
        if s.msgs.is_multiple_of(every) {
            s.rebuilds += 1;
            rebuild_idx += 1;
            for &gid in &pending {
                let d = ts.saturating_sub(pending_since[gid]);
                s.stale_ns += d as u128;
                s.max_stale_ns = s.max_stale_ns.max(d);
                pending_since[gid] = 0;
            }
            pending.clear();
        }
    }
    watch.close(ts.min(to));
    (s, watch)
}

fn kv<'a>(args: &'a [String], key: &str) -> Option<&'a str> {
    args.iter().find_map(|a| a.strip_prefix(key).and_then(|r| r.strip_prefix('=')))
}

fn num<T: std::str::FromStr>(args: &[String], key: &str, default: T) -> T {
    kv(args, key).and_then(|v| v.parse().ok()).unwrap_or(default)
}

fn name_of(path: &str) -> String {
    std::path::Path::new(path).file_name().and_then(|f| f.to_str()).unwrap_or("feed").replace('.', "_")
}

fn main() {
    let args: Vec<String> = std::env::args().collect();
    let (Some(pa), Some(pb)) = (kv(&args, "a"), kv(&args, "b")) else {
        eprintln!("usage: titan-cross a=PATH b=PATH [limit=N] [stats_limit=N] [from=SEC] [to=SEC] [every=K] [epochs=E] [pmu]");
        std::process::exit(2);
    };
    let epochs: usize = num(&args, "epochs", 10);
    let limit: u64 = num(&args, "limit", 4_000_000);
    let stats_limit: u64 = num(&args, "stats_limit", u64::MAX);
    let every: u64 = num(&args, "every", 1_000);
    let from: u64 = num::<u64>(&args, "from", 34_200) * 1_000_000_000;
    let to: u64 = num::<u64>(&args, "to", 57_600) * 1_000_000_000;
    assert!(every > 0 && from < to, "every must be above 0 and from below to");
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

    let raw: Vec<Vec<u8>> =
        [pa, pb].iter().map(|p| std::fs::read(p).unwrap_or_else(|e| panic!("cannot read {p}: {e}"))).collect();
    let src: [&[u8]; VENUES] = [&raw[0], &raw[1]];

    // Shared instruments, from the two Stock Directories.
    let (da, ta) = directory(src[0]);
    let (db, tb) = directory(src[1]);
    let mut symbols: Vec<[u8; 8]> = da.keys().filter(|s| db.contains_key(*s)).copied().collect();
    symbols.sort_unstable();
    assert!(!symbols.is_empty(), "the two feeds share no symbol");
    let instruments = [ta + 1, tb + 1];
    let mut map = Map { gid: [vec![NONE; instruments[0]], vec![NONE; instruments[1]]], loc: [Vec::new(), Vec::new()], symbols };
    for (gid, s) in map.symbols.iter().enumerate() {
        for (v, d) in [&da, &db].iter().enumerate() {
            let loc = d[s];
            map.gid[v][loc as usize] = gid as u32;
            map.loc[v].push(loc);
        }
    }
    let shared = map.symbols.len();
    assert_eq!(map.loc[0].len(), shared);
    assert_eq!(map.loc[1].len(), shared);

    let log2: u32 = num(&args, "log2", 23);
    println!(
        "cross_config a={} b={} bytes_a={} bytes_b={} locates_a={} locates_b={} shared={shared} limit={limit} every={every} from={from} to={to} epochs={epochs} pmu={}",
        name_of(pa),
        name_of(pb),
        raw[0].len(),
        raw[1].len(),
        instruments[0],
        instruments[1],
        pmu.is_some(),
    );

    // Untimed: the market statistics and what the periodic design misses.
    let mut st = State::new(log2, instruments, shared);
    let t_setup = clock::counter();
    let (s, watch) = measure(src, &map, &mut st, stats_limit, every, from, to);
    assert_eq!(st.cons.refolded(), (0..shared).map(|g| st.cons.top(g)).collect::<Vec<_>>(), "incremental view drifted");
    let c = watch.stats;
    let secs = (s.last_ts.saturating_sub(s.first_ts)) as f64 / 1e9;
    println!(
        "cross_market msgs={} in_window={} span_s={secs:.1} changes={} locked_episodes={} crossed_episodes={} locked_s={:.3} crossed_s={:.3} max_locked_ms={:.3} max_crossed_ms={:.3} crossed_depth_mean_units={:.2} instruments_crossed={} open_at_end={}",
        s.msgs,
        s.in_window,
        s.changes,
        c.locked_episodes,
        c.crossed_episodes,
        c.locked_ns as f64 / 1e9,
        c.crossed_ns as f64 / 1e9,
        c.max_locked_ns as f64 / 1e6,
        c.max_crossed_ns as f64 / 1e6,
        if c.crossed_episodes > 0 { c.crossed_depth_units as f64 / c.crossed_episodes as f64 } else { 0.0 },
        c.instruments_crossed,
        c.open_at_end,
    );
    println!(
        "cross_stale every={every} rebuilds={} stale_changes={} stale_mean_us={:.3} max_stale_ms={:.3} invisible_crossed={} invisible_share={:.4}",
        s.rebuilds,
        s.stale_changes,
        if s.stale_changes > 0 { s.stale_ns as f64 / s.stale_changes as f64 / 1e3 } else { 0.0 },
        s.max_stale_ns as f64 / 1e6,
        s.invisible_crossed,
        if c.crossed_episodes > 0 { s.invisible_crossed as f64 / c.crossed_episodes as f64 } else { 0.0 },
    );
    let freq = clock::counter_frequency() as f64;
    println!("cross_setup seconds={:.1}", (clock::counter() - t_setup) as f64 / freq);


    let run = |design: &str, pmu: Option<&Session>, st: &mut State| {
        st.reset();
        let a0 = thread_allocations();
        let p0 = pmu.map(|s| s.read());
        let t0 = clock::counter();
        let (msgs, sum) = match design {
            "floor" => replay::<0>(src, &map, st, limit, every),
            "incr" => replay::<1>(src, &map, st, limit, every),
            _ => replay::<2>(src, &map, st, limit, every),
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
        if design == "incr" {
            let kept: Vec<Quote> = (0..shared).map(|g| st.cons.top(g)).collect();
            let mut fresh = Consolidated::new(shared);
            let books = &st.books;
            let loc = &map.loc;
            fresh.rebuild_from(|gid, v| quote(&books[v], loc[v][gid]));
            let rebuilt: Vec<Quote> = (0..shared).map(|g| fresh.top(g)).collect();
            assert_eq!(kept, rebuilt, "the view kept update by update differs from the books");
        }
        (black_box(sum), msgs, t1 - t0, d, allocs)
    };
    for design in DESIGNS {
        run(design, pmu.as_ref(), &mut st);
    }
    for epoch in 0..epochs {
        for j in 0..DESIGNS.len() {
            let design = DESIGNS[(j + epoch) % DESIGNS.len()];
            let (sum, msgs, ticks, d, allocs) = run(design, pmu.as_ref(), &mut st);
            let secs = ticks as f64 / freq;
            let mut line = format!(
                "cross design={design} epoch={epoch} msgs={msgs} ns_per_msg={:.3} allocations_pass={allocs} checksum={sum}",
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
