//! Streaming throughput harness.
//!
//! One producer pushes `n` sequence numbers as fast as the ring accepts them;
//! the consumer (main thread) pops and sums them and checks the sum, so a lost
//! or duplicated message fails the run. Unlike ping-pong, many messages are in
//! flight, so each side repeatedly stores its own index and then loads the
//! other's: the pattern where RCsc (`ldar`) must wait for an earlier `stlr`,
//! and where indices sharing a cache line cause false sharing.
//!
//! With `pmu` (run under sudo) the consumer reports cycles, instructions,
//! L1D load misses and branch mispredicts per message from its own counters.
//!
//!   titan-stream [messages] [epochs] [pmu]

use std::time::Duration;
use titan_platform::clock;
use titan_platform::kperf::{events, Kperf, Session};
use titan_platform::qos::{self, Qos};
use titan_platform::rusage::Usage;
use titan_spsc::order::{AcqRel, Fenced, Order, SeqCst};
use titan_spsc::pad::{IndexCell, Pad128, Pad64};
use titan_spsc::ring::channel;
use titan_spsc::wait::{SpinHint, Wait, Wfe};
use titan_spsc::{Rx, Tx};

const CAPACITY: usize = 1024;

fn case<O: Order, P: IndexCell, W: Wait, const C: bool>(epoch: usize, n: usize, pmu: Option<&Session>) {
    let warm = n / 10 + 1000;
    let total = (warm + n) as u64;
    let (mut tx, mut rx) = channel::<O, P, W, C>(CAPACITY);
    let producer = std::thread::spawn(move || {
        let _ = qos::set_current_thread(Qos::UserInteractive);
        for i in 0..total {
            tx.send(i);
        }
    });
    let _ = qos::set_current_thread(Qos::UserInteractive);
    for _ in 0..warm {
        std::hint::black_box(rx.recv());
    }
    let u0 = Usage::now();
    let c0 = pmu.map(|s| s.read());
    let t0 = clock::counter();
    let mut sum = 0u64;
    for _ in 0..n {
        sum = sum.wrapping_add(rx.recv());
    }
    let t1 = clock::counter();
    let c1 = pmu.map(|s| s.read());
    let invol = Usage::now().since(&u0).involuntary_switches;
    producer.join().unwrap();

    let first = warm as u64;
    let last = total - 1;
    let expected = (first..=last).fold(0u64, |a, i| a.wrapping_add(i));
    assert_eq!(sum, expected, "message lost or duplicated");

    let f = clock::counter_frequency() as f64;
    let secs = (t1 - t0) as f64 / f;
    let ns_per_msg = secs * 1e9 / n as f64;
    let pmu_txt = match (pmu, c0, c1) {
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
        "st impl=ring order={} pad={} cached={} wait={} epoch={epoch} messages={n} ns_per_msg={ns_per_msg:.3} mmsg_per_s={:.2} invol_switches={invol}{pmu_txt}",
        O::NAME,
        P::ALIGN,
        C as u8,
        W::NAME,
        1e3 / ns_per_msg,
    );
}

type Case = fn(usize, usize, Option<&Session>);

fn main() {
    let args: Vec<String> = std::env::args().collect();
    let n: usize = args.get(1).and_then(|v| v.parse().ok()).unwrap_or(10_000_000);
    let epochs: usize = args.get(2).and_then(|v| v.parse().ok()).unwrap_or(3);
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
    let cases: Vec<Case> = vec![
        case::<AcqRel, Pad128, SpinHint, true>,
        case::<AcqRel, Pad128, SpinHint, false>,
        case::<AcqRel, Pad64, SpinHint, true>,
        case::<AcqRel, Pad64, SpinHint, false>,
        case::<SeqCst, Pad128, SpinHint, true>,
        case::<SeqCst, Pad128, SpinHint, false>,
        case::<SeqCst, Pad64, SpinHint, true>,
        case::<SeqCst, Pad64, SpinHint, false>,
        case::<Fenced, Pad128, SpinHint, true>,
        case::<Fenced, Pad128, SpinHint, false>,
        case::<Fenced, Pad64, SpinHint, true>,
        case::<Fenced, Pad64, SpinHint, false>,
        case::<AcqRel, Pad128, Wfe, true>,
    ];
    println!(
        "st_config messages={n} epochs={epochs} cases={} capacity={CAPACITY} pmu={}",
        cases.len(),
        pmu.is_some()
    );
    for epoch in 0..epochs {
        for k in 0..cases.len() {
            let idx = (k + epoch * 5) % cases.len();
            cases[idx](epoch, n, pmu.as_ref());
            std::thread::sleep(Duration::from_millis(50));
        }
    }
}
