//! What the inbound wire format costs: the same price-level updates decoded
//! from a fixed binary layout, from FIX 4.4 text and from JSON.
//!
//! Untimed: the feed is replayed through the reference book, and every
//! change of a price level's total quantity becomes one update (an add, an
//! execution, a cancel or a delete touches one level; a replace touches
//! two). The first `updates` of them (default 2,000,000) are encoded in each
//! format (see `titan_itch::wire`), and every decoder is checked to return
//! exactly the original record for every update before anything is timed.
//!
//! Timed, per format, one pass over all updates: decode and fold the record
//! into a running state. `floor_bin`, `floor_fix` and `floor_json` fold the
//! first and last byte and the length of every message of that format: the
//! loop and a touch of each message, without decoding; each decoder is
//! compared with the floor of its own format. Values are per
//! update: generic counter time and, with `pmu`, core cycles, instructions,
//! L1D load misses and branch mispredicts of the whole pass; allocations of
//! the pass per update. Stage order rotates between epochs.
//!
//!   titan-wire [messages=N] [instruments=M] [updates=U] [epochs=E] [pmu]
//!   titan-wire file=PATH [limit=N] [updates=U] [epochs=E] [pmu]

use std::hint::black_box;
use titan_itch::decode::{decode, Msg, Reader};
use titan_itch::gen::{feed, Mix};
use titan_itch::reference::RefBook;
use titan_itch::wire::{
    decode_bin, decode_fix, decode_json_fixed, decode_json_serde, encode_bin, encode_fix, encode_json, md_action, Level, Symbols,
};
use titan_itch::{Book, Side};
use titan_platform::alloc_count::{thread_allocations, CountingAlloc};
use titan_platform::clock;
use titan_platform::kperf::{events, Kperf, Session};
use titan_platform::qos::{self, Qos};

#[global_allocator]
static A: CountingAlloc = CountingAlloc;

const STAGES: [&str; 7] = ["floor_bin", "floor_fix", "floor_json", "bin", "fix", "json_fixed", "json_serde"];

/// Messages of one format, back to back, with (offset, length) per update.
struct Encoded {
    bytes: Vec<u8>,
    at: Vec<(u32, u16)>,
}

impl Encoded {
    fn new() -> Encoded {
        Encoded { bytes: Vec::new(), at: Vec::new() }
    }
    fn push_with(&mut self, f: impl FnOnce(&mut Vec<u8>)) {
        let start = self.bytes.len();
        f(&mut self.bytes);
        let len = self.bytes.len() - start;
        assert!(start < u32::MAX as usize && len < u16::MAX as usize);
        self.at.push((start as u32, len as u16));
    }
    #[inline(always)]
    fn get(&self, i: usize) -> &[u8] {
        let (o, l) = self.at[i];
        &self.bytes[o as usize..o as usize + l as usize]
    }
}

#[inline(always)]
fn fold(state: u64, u: &Level) -> u64 {
    state.rotate_left(7) ^ u.seq ^ ((u.locate as u64) << 48) ^ ((u.price as u64) << 1) ^ u.qty ^ u.buy as u64
}

#[inline(never)]
fn pass(stage: &str, bin: &Encoded, fix: &Encoded, json: &Encoded, sym: &Symbols) -> u64 {
    let n = bin.at.len();
    let mut s = 0u64;
    match stage {
        "floor_bin" | "floor_fix" | "floor_json" => {
            let e = match stage {
                "floor_bin" => bin,
                "floor_fix" => fix,
                _ => json,
            };
            for i in 0..n {
                let m = e.get(i);
                s = s.rotate_left(7) ^ m[0] as u64 ^ m[m.len() - 1] as u64 ^ m.len() as u64;
            }
        }
        "bin" => {
            for i in 0..n {
                s = fold(s, &decode_bin(bin.get(i)).expect("checked"));
            }
        }
        "fix" => {
            for i in 0..n {
                s = fold(s, &decode_fix(fix.get(i), sym).expect("checked"));
            }
        }
        "json_fixed" => {
            for i in 0..n {
                s = fold(s, &decode_json_fixed(json.get(i), sym).expect("checked"));
            }
        }
        _ => {
            for i in 0..n {
                s = fold(s, &decode_json_serde(json.get(i), sym).expect("checked"));
            }
        }
    }
    s
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
    let want: usize = num(&args, "updates", 2_000_000);
    let limit: usize = num(&args, "limit", 20_000_000);
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

    // Untimed: level updates from the reference book, symbols from 'R'.
    let mut symbols: Vec<[u8; 8]> = Vec::new();
    let mut levels: Vec<(Level, bool, u64)> = Vec::new(); // (update, level existed before, ts ns)
    let mut book = RefBook::default();
    let mut r = Reader::new(&src);
    let (mut msgs, mut seq) = (0usize, 0u64);
    while msgs < limit && levels.len() < want {
        let Some(Ok(b)) = r.next_raw() else { break };
        msgs += 1;
        if b.len() >= 19 && b[0] == b'R' {
            let loc = u16::from_be_bytes([b[1], b[2]]) as usize;
            if symbols.len() <= loc {
                symbols.resize(loc + 1, [b' '; 8]);
            }
            symbols[loc].copy_from_slice(&b[11..19]);
        }
        let Ok(m) = decode(b) else { continue };
        // The price levels this message changes, read before it is applied.
        let mut touched: [(u16, Side, u32); 2] = [(0, Side::Buy, 0); 2];
        let mut nt = 0usize;
        let ts = match m {
            Msg::Add { locate, ts, side, price, .. } => {
                touched[0] = (locate, side, price);
                nt = 1;
                ts
            }
            Msg::Executed { ts, order, .. }
            | Msg::ExecutedWithPrice { ts, order, .. }
            | Msg::Cancel { ts, order, .. }
            | Msg::Delete { ts, order, .. } => {
                if let Some(l) = book.order_level(order) {
                    touched[0] = l;
                    nt = 1;
                }
                ts
            }
            Msg::Replace { ts, old, price, .. } => {
                if let Some((l, s, p_old)) = book.order_level(old) {
                    touched[0] = (l, s, p_old);
                    nt = 1;
                    if price != p_old {
                        touched[1] = (l, s, price);
                        nt = 2;
                    }
                }
                ts
            }
            Msg::Other { .. } => 0,
        };
        let before: Vec<u64> = touched[..nt].iter().map(|&(l, s, p)| book.level_shares(l, s, p)).collect();
        book.apply(&m);
        for (k, &(l, s, p)) in touched[..nt].iter().enumerate() {
            let after = book.level_shares(l, s, p);
            if after == before[k] {
                continue;
            }
            seq += 1;
            levels.push((Level { seq, locate: l, buy: s == Side::Buy, price: p, qty: after }, before[k] > 0, ts));
        }
    }
    levels.truncate(want);
    assert!(!levels.is_empty(), "no level updates");
    let max_locate = levels.iter().map(|x| x.0.locate).max().unwrap() as usize;
    if symbols.len() <= max_locate {
        symbols.resize(max_locate + 1, [b' '; 8]);
    }
    // Locates without a stock directory entry (the synthetic feed) get "S<locate>".
    for (l, s) in symbols.iter_mut().enumerate() {
        if *s == [b' '; 8] {
            let t = format!("S{l}");
            s[..t.len()].copy_from_slice(t.as_bytes());
        }
    }
    let sym = Symbols::new(symbols);
    let (mut bin, mut fix, mut json) = (Encoded::new(), Encoded::new(), Encoded::new());
    for (u, existed, ts) in &levels {
        bin.push_with(|o| encode_bin(u, *ts, o));
        fix.push_with(|o| encode_fix(u, md_action(*existed, u.qty), *ts, &sym, o));
        json.push_with(|o| encode_json(u, ts / 1_000_000, &sym, o));
    }
    // Every decoder must return the original record for every update.
    for (i, (u, _, _)) in levels.iter().enumerate() {
        assert_eq!(decode_bin(bin.get(i)).as_ref(), Some(u), "binary decode differs at update {i}");
        assert_eq!(decode_fix(fix.get(i), &sym).as_ref(), Ok(u), "FIX decode differs at update {i}");
        assert_eq!(decode_json_fixed(json.get(i), &sym).as_ref(), Ok(u), "fixed JSON decode differs at update {i}");
        assert_eq!(decode_json_serde(json.get(i), &sym).as_ref(), Ok(u), "serde JSON decode differs at update {i}");
    }
    let n = levels.len();
    let deletes = levels.iter().filter(|x| x.0.qty == 0).count();
    println!(
        "wire_config feed={name} messages_read={msgs} updates={n} deletes={deletes} instruments={} epochs={epochs} pmu={} \
         bytes_bin={:.2} bytes_fix={:.2} bytes_json={:.2}",
        max_locate + 1,
        pmu.is_some(),
        bin.bytes.len() as f64 / n as f64,
        fix.bytes.len() as f64 / n as f64,
        json.bytes.len() as f64 / n as f64,
    );
    drop(levels);
    let freq = clock::counter_frequency() as f64;
    let run = |stage: &str, pmu: Option<&Session>| {
        let a0 = thread_allocations();
        let p0 = pmu.map(|s| s.read());
        let t0 = clock::counter();
        let s = pass(stage, &bin, &fix, &json, &sym);
        let t1 = clock::counter();
        let d = match (pmu, p0) {
            (Some(p), Some(a)) => {
                let b = p.read();
                Some(p.delta(&a, &b))
            }
            _ => None,
        };
        let allocs = thread_allocations() - a0 - d.is_some() as u64;
        (black_box(s), t1 - t0, d, allocs)
    };
    for st in STAGES {
        run(st, pmu.as_ref());
    }
    for epoch in 0..epochs {
        for j in 0..STAGES.len() {
            let st = STAGES[(j + epoch) % STAGES.len()];
            let (state, ticks, d, allocs) = run(st, pmu.as_ref());
            let secs = ticks as f64 / freq;
            let mut line = format!(
                "wire feed={name} stage={st} epoch={epoch} updates={n} ns_per_update={:.3} allocations_per_update={:.4} checksum={state:016x}",
                secs * 1e9 / n as f64,
                allocs as f64 / n as f64,
            );
            if let Some(d) = d {
                let per = |x: u64| x as f64 / n as f64;
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
