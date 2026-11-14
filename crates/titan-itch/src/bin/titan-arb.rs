//! What A/B line arbitration and gap recovery cost per delivered message.
//!
//! Untimed: the first `messages` ITCH messages of the feed are framed into
//! MoldUDP64 downstream packets of at most 1472 bytes of UDP payload. Line B
//! is a byte copy of line A at a different address, a fixed `lag` packets
//! behind A; each line loses packets independently (seeded, reproducible).
//! A dry pass records every Request Packet the arbiter sends and builds each
//! answer beforehand, and the arbiter's containers are created before the
//! timed region, so the timed pass allocates nothing; the dry pass also
//! yields the arbitration statistics and checks exact delivery. The answer
//! to a request is injected 64 events after it (event time, not waiting),
//! so the conditions measure processing cost, not recovery latency.
//!
//! Timed, per condition, one pass over the whole event schedule: `floor`
//! walks every packet of line A and folds its first byte, last byte and
//! length; `one0` one lossless line; `two0` two lossless lines; `two_a`
//! loss on A only, filled from B; `two_ab` loss on both lines, the doubly
//! lost messages recovered from the re-request server. Every condition must
//! deliver every message exactly once and in order: the delivered fold must
//! equal the store's, asserted in every pass. Values are per delivered
//! message: generic counter time and, with `pmu`, core cycles, instructions,
//! L1D load misses and branch mispredicts of the whole pass; allocations of
//! the pass per message. Condition order rotates between epochs.
//!
//!   titan-arb [messages=N] [instruments=M] [epochs=E] [loss_ppm=L] [pmu]
//!   titan-arb file=PATH [messages=N] [epochs=E] [loss_ppm=L] [pmu]

use std::collections::VecDeque;
use std::hint::black_box;
use titan_itch::arb::{downstream, header_only, Arbiter, Packets, Request, Store, MAX_PAYLOAD};
use titan_itch::decode::Reader;
use titan_itch::gen::{feed, Mix, Rng};
use titan_itch::mold::{END_OF_SESSION, HEADER};
use titan_platform::alloc_count::{thread_allocations, CountingAlloc};
use titan_platform::clock;
use titan_platform::kperf::{events, Kperf, Session};
use titan_platform::qos::{self, Qos};

#[global_allocator]
static A: CountingAlloc = CountingAlloc;

const SESSION: &[u8; 10] = b"TITANARB01";
const CONDITIONS: [&str; 5] = ["floor", "one0", "two0", "two_a", "two_ab"];
const LAG: usize = 4;
const RTT: usize = 64;

#[inline(always)]
fn fold(state: u64, m: &[u8]) -> u64 {
    state.rotate_left(7) ^ m[0] as u64 ^ m[m.len() - 1] as u64 ^ m.len() as u64
}

/// One event of the schedule: which line, and which packet of that line
/// (`u32::MAX` is that line's end-of-session packet).
type Ev = (u8, u32);

/// The arrival schedule of a condition: per-line loss, B `LAG` packets
/// behind A, in the time order `2 * (index + line * LAG) + line`.
fn schedule(packets: usize, lines: usize, loss_ppm: [u64; 2], seed: u64) -> Vec<Ev> {
    let mut rng = Rng::new(seed);
    let mut ev: Vec<(usize, Ev)> = Vec::new();
    for i in 0..packets {
        for (l, ppm) in loss_ppm.iter().enumerate().take(lines) {
            if rng.below(1_000_000) >= *ppm {
                ev.push((2 * (i + l * LAG) + l, (l as u8, i as u32)));
            }
        }
    }
    for l in 0..lines {
        ev.push((2 * (packets + l * LAG) + l, (l as u8, u32::MAX)));
    }
    ev.sort_unstable();
    ev.into_iter().map(|(_, e)| e).collect()
}

/// Answers to the arbiter's requests: the dry pass records and builds them,
/// the timed pass replays them.
struct Answers {
    requests: Vec<Request>,
    bytes: Vec<u8>,
    off: Vec<u32>,
}

impl Answers {
    fn get(&self, i: usize) -> &[u8] {
        &self.bytes[self.off[i] as usize..self.off[i + 1] as usize]
    }
}

/// Everything a pass reads: the store, the two lines and the end-of-session
/// packet, all owned by the caller so the arbiter can borrow them.
struct Setup<'a> {
    store: &'a Store,
    a: &'a Packets,
    b: &'a Packets,
    eos: &'a [u8; HEADER],
}

/// Runs the arbiter over a schedule. `answers` is `None` in the dry pass
/// (requests are answered from the store and recorded) and `Some` in the
/// timed pass (requests are asserted equal and answered from the record).
/// The arbiter and the pending queue come from the caller, created before
/// the timed region. Returns the delivered fold, the delivered count and
/// the dry-pass record.
fn run_schedule<'a>(
    su: &Setup<'a>,
    ev: &[Ev],
    answers: Option<&Answers>,
    arb: &mut Arbiter<'a>,
    pending: &mut VecDeque<(usize, usize)>,
) -> (u64, u64, Option<Answers>) {
    let (mut sum, mut count) = (0u64, 0u64);
    let mut deliver = |m: &[u8]| {
        sum = fold(sum, m);
        count += 1;
    };
    let mut rec = answers.is_none().then(|| Answers { requests: Vec::new(), bytes: Vec::new(), off: vec![0] });
    let mut ri = 0usize;
    let mut on_req = |r: Option<Request>, t: usize, pending: &mut VecDeque<(usize, usize)>, rec: &mut Option<Answers>| {
        let Some(r) = r else { return };
        match rec {
            Some(rec) => {
                rec.requests.push(r);
                downstream(su.store, SESSION, r.seq, r.count, MAX_PAYLOAD, &mut rec.bytes);
                rec.off.push(rec.bytes.len() as u32);
            }
            None => assert_eq!(answers.unwrap().requests[ri], r, "request {ri} differs between passes"),
        }
        // The 20-byte Request Packet a receiver would send, part of the cost.
        black_box(r.packet(SESSION));
        pending.push_back((t + RTT, ri));
        ri += 1;
    };
    let mut t = 0usize;
    let mut k = 0usize;
    while k < ev.len() || !pending.is_empty() {
        while pending.front().is_some_and(|&(due, _)| due <= t) {
            let (_, i) = pending.pop_front().unwrap();
            let resp = match (&rec, answers) {
                (Some(rec), _) => rec.get(i),
                (None, Some(ans)) => ans.get(i),
                (None, None) => unreachable!(),
            };
            let r = arb.on_resend(resp, &mut deliver);
            on_req(r, t, pending, &mut rec);
        }
        if k < ev.len() {
            let (l, i) = ev[k];
            let p = match (l, i) {
                (_, u32::MAX) => &su.eos[..],
                (0, i) => su.a.get(i as usize),
                (_, i) => su.b.get(i as usize),
            };
            let r = arb.on_line(l as usize, p, &mut deliver);
            on_req(r, t, pending, &mut rec);
            k += 1;
        }
        t += 1;
    }
    (sum, count, rec)
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
    let want: usize = num(&args, "messages", 2_000_000);
    let loss_ppm: u64 = num(&args, "loss_ppm", 30_000);
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
            let instruments: u16 = num(&args, "instruments", 500);
            (feed(42, instruments, want, Mix::default()), "synthetic_default".to_string())
        }
    };

    // Untimed: the message store, the two lines, the schedules, the answers.
    let mut store = Store::new();
    let mut r = Reader::new(&src);
    while store.len() < want {
        let Some(Ok(b)) = r.next_raw() else { break };
        store.push(b);
    }
    drop(src);
    let n = store.len() as u64;
    assert!(n > 0, "no messages");
    let mut want_sum = 0u64;
    for seq in 1..=n {
        want_sum = fold(want_sum, store.get(seq));
    }
    let a = Packets::of(&store, SESSION, MAX_PAYLOAD);
    let b = a.copy();
    println!(
        "arb_config feed={name} messages={n} packets={} payload={MAX_PAYLOAD} lag={LAG} rtt={RTT} loss_ppm={loss_ppm} epochs={epochs} pmu={}",
        a.len(),
        pmu.is_some(),
    );

    // Dry pass per condition: schedule, answers, stats, exact delivery.
    let eos = header_only(SESSION, n + 1, END_OF_SESSION);
    let su = Setup { store: &store, a: &a, b: &b, eos: &eos };
    let mut plans: Vec<(&str, Vec<Ev>, usize, Option<Answers>)> = Vec::new();
    for cond in &CONDITIONS[1..] {
        let (lines, loss) = match *cond {
            "one0" => (1, [0, 0]),
            "two0" => (2, [0, 0]),
            "two_a" => (2, [loss_ppm, 0]),
            _ => (2, [loss_ppm, loss_ppm]),
        };
        let ev = schedule(a.len(), lines, loss, 42);
        let mut arb = Arbiter::new(*SESSION, lines, 4096);
        let mut pending = VecDeque::with_capacity(64);
        let (sum, count, rec) = run_schedule(&su, &ev, None, &mut arb, &mut pending);
        let st = arb.stats;
        drop(arb);
        println!(
            "arb_cond cond={cond} feed={name} lines={lines} loss_a={} loss_b={} events={} packets={} duplicate={} held={} max_held={} requests={} resent={} partial={} foreign={}",
            loss[0], loss[1], ev.len(), st.packets, st.duplicate, st.held, st.max_held, st.requests, st.resent, st.partial, st.foreign
        );
        assert_eq!((sum, count), (want_sum, n), "{cond}: delivery is not exact");
        let rec = rec.unwrap();
        assert_eq!(st.requests as usize, rec.requests.len());
        if *cond == "two_ab" {
            assert!(!rec.requests.is_empty(), "two_ab sent no requests; raise loss_ppm");
        }
        plans.push((cond, ev, lines, Some(rec)));
    }

    let freq = clock::counter_frequency() as f64;
    let run = |cond: &str, pmu: Option<&Session>| {
        let plan = (cond != "floor").then(|| plans.iter().find(|p| p.0 == cond).unwrap());
        let mut arb = Arbiter::new(*SESSION, plan.map_or(1, |p| p.2), 4096);
        let mut pending = VecDeque::with_capacity(64);
        let a0 = thread_allocations();
        let p0 = pmu.map(|s| s.read());
        let t0 = clock::counter();
        let (sum, count) = match plan {
            None => {
                let mut s = 0u64;
                for i in 0..a.len() {
                    s = fold(s, a.get(i));
                }
                (s, n)
            }
            Some((_, ev, _, ans)) => {
                let (sum, count, _) = run_schedule(&su, ev, ans.as_ref(), &mut arb, &mut pending);
                (sum, count)
            }
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
        if cond != "floor" {
            assert_eq!((sum, count), (want_sum, n), "{cond}: delivery is not exact");
        }
        (black_box(sum), t1 - t0, d, allocs)
    };
    for cond in CONDITIONS {
        run(cond, pmu.as_ref());
    }
    for epoch in 0..epochs {
        for j in 0..CONDITIONS.len() {
            let cond = CONDITIONS[(j + epoch) % CONDITIONS.len()];
            let (sum, ticks, d, allocs) = run(cond, pmu.as_ref());
            let secs = ticks as f64 / freq;
            let mut line = format!(
                "arb feed={name} cond={cond} epoch={epoch} messages={n} ns_per_msg={:.3} allocations_pass={allocs} allocations_per_msg={:.4} checksum={sum:016x}",
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
