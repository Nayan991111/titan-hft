//! Order-book replay benchmark.
//!
//! Generates a deterministic ITCH 5.0 feed, copies it into a pinned region
//! and replays it on one thread (QoS USER_INTERACTIVE). Per message the loop
//! decodes, applies the message to the book and reads the best bid of the
//! instrument touched; the best prices are folded into a checksum that is
//! consumed with `black_box`, so the work cannot be removed by the compiler.
//!
//! Implementations: `decode` (decoder only), `l3` (zero-allocation book),
//! `ref` (BTreeMap/HashMap oracle). One unmeasured warm-up pass precedes the
//! measured epochs; the book is cleared, not rebuilt, between epochs, so the
//! allocation count of measured epochs shows whether the hot path allocates.
//!
//!   titan-replay [messages] [instruments] [epochs] [pmu]
//!   titan-replay file=PATH [limit=MESSAGES] [epochs=N] [pmu]
//!   titan-replay write=PATH [messages=N] [instruments=M]
//!
//! File mode replays a NASDAQ ITCH 5.0 binary file (decompressed). Before
//! timing, a verification pass applies every message to both the L3 book and
//! the reference book and compares the best bid and ask of the instrument
//! touched after each message; any mismatch aborts the run.

use std::hint::black_box;
use titan_itch::book::L3Book;
use titan_itch::decode::{Msg, Reader};
use titan_itch::gen::{feed, Mix};
use titan_itch::reference::RefBook;
use titan_itch::{Book, Side};
use titan_platform::alloc_count::{thread_allocations, CountingAlloc};
use titan_platform::clock;
use titan_platform::kperf::{events, Kperf, Session};
use titan_platform::mem::PinnedRegion;
use titan_platform::qos::{self, Qos};

#[global_allocator]
static A: CountingAlloc = CountingAlloc;

#[inline(always)]
fn locate_of(m: &Msg) -> u16 {
    match *m {
        Msg::Add { locate, .. }
        | Msg::Executed { locate, .. }
        | Msg::ExecutedWithPrice { locate, .. }
        | Msg::Cancel { locate, .. }
        | Msg::Delete { locate, .. }
        | Msg::Replace { locate, .. } => locate,
        Msg::Other { .. } => 0,
    }
}

#[inline(never)]
fn replay_book<B: Book>(buf: &[u8], book: &mut B) -> (u64, u64) {
    let mut r = Reader::new(buf);
    let mut chk = 0u64;
    let mut n = 0u64;
    while let Some(m) = r.next_msg() {
        let m = m.expect("well-formed feed");
        book.apply(&m);
        if let Some((p, s)) = book.best(locate_of(&m), Side::Buy) {
            chk = chk.rotate_left(5) ^ (p as u64) ^ s;
        }
        n += 1;
    }
    (n, black_box(chk))
}

#[inline(never)]
fn replay_decode(buf: &[u8]) -> (u64, u64) {
    let mut r = Reader::new(buf);
    let mut chk = 0u64;
    let mut n = 0u64;
    while let Some(m) = r.next_msg() {
        let m = m.expect("well-formed feed");
        chk = chk.rotate_left(5) ^ locate_of(&m) as u64;
        n += 1;
    }
    (n, black_box(chk))
}

struct Run<'a> {
    mix: &'static str,
    imp: &'static str,
    buf: &'a [u8],
    pmu: Option<&'a Session>,
}

fn measure(run: &Run, epoch: usize, f: &mut dyn FnMut(&[u8]) -> (u64, u64)) {
    let a0 = thread_allocations();
    let c0 = run.pmu.map(|s| s.read());
    let t0 = clock::counter();
    let (n, chk) = f(run.buf);
    let t1 = clock::counter();
    let c1 = run.pmu.map(|s| s.read());
    let allocs = thread_allocations() - a0;
    let secs = (t1 - t0) as f64 / clock::counter_frequency() as f64;
    let ns = secs * 1e9 / n as f64;
    let pmu = match (run.pmu, c0, c1) {
        (Some(s), Some(a), Some(b)) => {
            let d = s.delta(&a, &b);
            let per = |x: u64| x as f64 / n as f64;
            format!(
                " msg_cycles={:.2} msg_instructions={:.2} msg_l1d_miss={:.4} msg_br_mispred={:.4} core_ghz={:.3}",
                per(d[0]),
                per(d[1]),
                per(d[2]),
                per(d[3]),
                d[0] as f64 / secs / 1e9
            )
        }
        _ => String::new(),
    };
    println!(
        "rp impl={} mix={} epoch={epoch} messages={n} ns_per_msg={ns:.3} mmsg_per_s={:.2} allocations={allocs} checksum={chk:016x}{pmu}",
        run.imp,
        run.mix,
        1e3 / ns,
    );
}

fn kv<'a>(args: &'a [String], key: &str) -> Option<&'a str> {
    args.iter().find_map(|a| a.strip_prefix(key).and_then(|r| r.strip_prefix('=')))
}

/// Byte length of the first `limit` messages, maximum stock locate, count.
fn scan(buf: &[u8], limit: usize) -> (usize, u16, usize) {
    let mut r = Reader::new(buf);
    let mut max_locate = 0u16;
    let mut n = 0usize;
    let mut end = 0usize;
    while n < limit {
        match r.next_raw() {
            Some(Ok(b)) => {
                if b.len() >= 3 {
                    max_locate = max_locate.max(u16::from_be_bytes([b[1], b[2]]));
                }
                n += 1;
                end += 2 + b.len();
            }
            Some(Err(e)) => {
                eprintln!("malformed input after {n} messages: {e:?}");
                break;
            }
            None => break,
        }
    }
    (end, max_locate, n)
}

/// Apply every message to both books; compare top of book after each one.
fn verify(buf: &[u8], log2: u32, instruments: usize) -> (u64, u64, u64) {
    let mut fast = L3Book::new(log2, instruments);
    let mut slow = RefBook::default();
    let mut r = Reader::new(buf);
    let mut n = 0u64;
    let mut mismatches = 0u64;
    while let Some(m) = r.next_msg() {
        let m = m.expect("well-formed feed");
        fast.apply(&m);
        slow.apply(&m);
        let loc = locate_of(&m);
        if !matches!(m, Msg::Other { .. }) {
            for side in [Side::Buy, Side::Sell] {
                if fast.best(loc, side) != slow.best(loc, side) {
                    if mismatches < 5 {
                        eprintln!("mismatch at message {n}: {m:?} side {side:?} fast {:?} ref {:?}", fast.best(loc, side), slow.best(loc, side));
                    }
                    mismatches += 1;
                }
            }
        }
        n += 1;
    }
    assert_eq!(fast.live_orders(), slow.live_orders(), "live order counts differ");
    (n, mismatches, fast.unknown)
}

fn file_mode(args: &[String], path: &str, pmu: Option<&Session>) {
    let limit: usize = kv(args, "limit").and_then(|v| v.parse().ok()).unwrap_or(usize::MAX);
    let epochs: usize = kv(args, "epochs").and_then(|v| v.parse().ok()).unwrap_or(3);
    let raw = std::fs::read(path).unwrap_or_else(|e| panic!("cannot read {path}: {e}"));
    let (len, max_locate, n) = scan(&raw, limit);
    let mut region = PinnedRegion::new(len).expect("feed region");
    region.as_mut_slice()[..len].copy_from_slice(&raw[..len]);
    drop(raw);
    let buf = &region.as_mut_slice()[..len];
    let instruments = max_locate as usize + 1;
    let name = std::path::Path::new(path).file_name().and_then(|f| f.to_str()).unwrap_or("file").replace('.', "_");

    let mut kinds = [0u64; 256];
    let mut r = Reader::new(buf);
    let mut probe = RefBook::default();
    let mut peak = 0usize;
    while let Some(b) = r.next_raw() {
        let b = b.expect("well-formed feed");
        kinds[b[0] as usize] += 1;
        if let Ok(m) = titan_itch::decode::decode(b) {
            probe.apply(&m);
            peak = peak.max(probe.live_orders());
        }
    }
    drop(probe);
    let mix: Vec<String> = (0..256).filter(|&k| kinds[k] > 0).map(|k| format!("{}:{}", k as u8 as char, kinds[k])).collect();
    let mut log2 = 10u32;
    while ((1usize << log2) / 2) < peak {
        log2 += 1;
    }
    println!(
        "rp_feed mix={name} source=file messages={n} bytes={len} instruments={instruments} peak_live_orders={peak} order_table_slots={} types={}",
        1usize << log2,
        mix.join(",")
    );

    let (vn, mismatches, unknown) = verify(buf, log2, instruments);
    println!("rp_verify mix={name} messages={vn} top_of_book_mismatches={mismatches} unknown_order_refs={unknown}");
    assert_eq!(mismatches, 0, "L3 book disagrees with the reference book on real data");

    let mut l3 = L3Book::new(log2, instruments);
    replay_book(buf, &mut l3);
    let base = Run { mix: Box::leak(name.into_boxed_str()), imp: "l3", buf, pmu };
    for epoch in 0..epochs {
        let dec = Run { imp: "decode", ..base };
        measure(&dec, epoch, &mut |b| replay_decode(b));
        l3.clear();
        {
            let book = &mut l3;
            measure(&base, epoch, &mut |b| replay_book(b, book));
        }
        let mut rf = RefBook::default();
        {
            let book = &mut rf;
            let rr = Run { imp: "ref", ..base };
            measure(&rr, epoch, &mut |b| replay_book(b, book));
        }
        black_box(&rf);
    }
}

/// Write a synthetic feed in NASDAQ binary file format, with a system event
/// first and a non-displayed trade ('P', 44 bytes) after every 20th message,
/// so file mode also exercises message types the book ignores.
fn write_mode(args: &[String], path: &str) {
    let messages: usize = kv(args, "messages").and_then(|v| v.parse().ok()).unwrap_or(1_000_000);
    let instruments: u16 = kv(args, "instruments").and_then(|v| v.parse().ok()).unwrap_or(500);
    let book = feed(99, instruments, messages, Mix::default());
    let mut out = Vec::with_capacity(book.len() + book.len() / 10);
    let mut sys = vec![0u8; 12];
    sys[0] = b'S';
    sys[11] = b'O';
    out.extend_from_slice(&12u16.to_be_bytes());
    out.extend_from_slice(&sys);
    let mut trade = vec![0u8; 44];
    trade[0] = b'P';
    let mut r = Reader::new(&book);
    let mut i = 0usize;
    while let Some(b) = r.next_raw() {
        let b = b.expect("generator output");
        out.extend_from_slice(&(b.len() as u16).to_be_bytes());
        out.extend_from_slice(b);
        i += 1;
        if i.is_multiple_of(20) {
            out.extend_from_slice(&44u16.to_be_bytes());
            out.extend_from_slice(&trade);
        }
    }
    std::fs::write(path, &out).unwrap_or_else(|e| panic!("cannot write {path}: {e}"));
    println!("rp_write path={path} book_messages={messages} bytes={}", out.len());
}

fn main() {
    let args: Vec<String> = std::env::args().collect();
    if let Some(path) = kv(&args, "write") {
        write_mode(&args, path);
        return;
    }
    let pmu = if args.iter().any(|a| a == "pmu") {
        match Kperf::open().and_then(|k| {
            k.session(&[events::CYCLES, events::INSTRUCTIONS, events::L1D_LOAD_MISS, events::BRANCH_MISPREDICT])
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
    if let Some(path) = kv(&args, "file") {
        println!("rp_config source=file pmu={}", pmu.is_some());
        file_mode(&args, path, pmu.as_ref());
        return;
    }
    let messages: usize = args.get(1).and_then(|v| v.parse().ok()).unwrap_or(5_000_000);
    let instruments: u16 = args.get(2).and_then(|v| v.parse().ok()).unwrap_or(500);
    let epochs: usize = args.get(3).and_then(|v| v.parse().ok()).unwrap_or(3);

    let mixes: [(&'static str, Mix); 3] = [
        ("default", Mix::default()),
        ("add_heavy", Mix { add: 60, execute: 5, execute_price: 1, cancel: 8, delete: 20, replace: 6 }),
        ("replace_heavy", Mix { add: 35, execute: 10, execute_price: 2, cancel: 8, delete: 15, replace: 30 }),
    ];
    println!("rp_config messages={messages} instruments={instruments} epochs={epochs} pmu={}", pmu.is_some());

    for (name, mix) in mixes {
        let src = feed(42, instruments, messages, mix);
        let mut region = PinnedRegion::new(src.len()).expect("feed region");
        region.as_mut_slice()[..src.len()].copy_from_slice(&src);
        let buf = &region.as_mut_slice()[..src.len()];
        drop(src);

        // Size the order table from the reference replay's peak live count.
        let mut probe = RefBook::default();
        let mut peak = 0usize;
        let mut r = Reader::new(buf);
        while let Some(m) = r.next_msg() {
            probe.apply(&m.unwrap());
            peak = peak.max(probe.live_orders());
        }
        drop(probe);
        let mut log2 = 10u32;
        while ((1usize << log2) / 2) < peak {
            log2 += 1;
        }
        println!("rp_feed mix={name} bytes={} peak_live_orders={peak} order_table_slots={}", buf.len(), 1usize << log2);

        let mut l3 = L3Book::new(log2, instruments as usize + 1);
        let mut rf = RefBook::default();
        replay_book(buf, &mut l3); // warm-up: grows level vectors once
        let base = Run { mix: name, imp: "l3", buf, pmu: pmu.as_ref() };
        for epoch in 0..epochs {
            let dec = Run { imp: "decode", ..base };
            measure(&dec, epoch, &mut |b| replay_decode(b));

            l3.clear();
            {
                let book = &mut l3;
                measure(&base, epoch, &mut |b| replay_book(b, book));
            }

            rf = RefBook::default();
            {
                let book = &mut rf;
                let rr = Run { imp: "ref", ..base };
                measure(&rr, epoch, &mut |b| replay_book(b, book));
            }
        }
        black_box(&rf);
    }
}
