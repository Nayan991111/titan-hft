//! Cross-core round-trip harness.
//!
//! Thread A sends a sequence number to thread B over ring 1; B echoes it back
//! over ring 2. Both threads request QoS USER_INTERACTIVE (P-core preference;
//! Apple silicon has no hard affinity). Two measurements per epoch:
//!
//! * `batch`: one counter span around `n` round trips, no timer reads inside
//!   the loop. Mean round trip and one-way estimate (RTT / 2) come from this.
//! * `per_rt`: a counter read around every round trip. Gives the shape of the
//!   distribution at the counter's resolution; includes the cost of the reads.
//!
//! Closed loop: one message in flight, so there is no coordinated-omission
//! effect to correct. Output is one `key=value` line per epoch.
//!
//!   titan-pingpong [round_trips] [epochs]

use std::time::Duration;
use titan_platform::clock;
use titan_platform::qos::{self, Qos};
use titan_platform::rusage::Usage;
use titan_spsc::order::{AcqRel, Fenced, Order, SeqCst};
use titan_spsc::pad::{IndexCell, Pad128, Pad64};
use titan_spsc::ring::channel;
use titan_spsc::wait::{Spin, SpinHint, Wait, Wfe, Yield};
use titan_spsc::{Rx, Tx};

const STOP: u64 = u64::MAX;
const CAPACITY: usize = 1024;

struct Stats {
    batch_mean_ns: f64,
    p50: f64,
    p90: f64,
    p99: f64,
    p999: f64,
    max: f64,
    invol: i64,
}

fn pct(sorted: &[u64], q: f64) -> u64 {
    let i = ((sorted.len() as f64 - 1.0) * q).round() as usize;
    sorted[i]
}

fn measure<T: Tx + 'static, R: Rx + 'static>(
    mut a_tx: T,
    mut a_rx: R,
    mut b_tx: impl Tx + 'static,
    mut b_rx: impl Rx + 'static,
    n: usize,
) -> Stats {
    let echo = std::thread::spawn(move || {
        let _ = qos::set_current_thread(Qos::UserInteractive);
        loop {
            let v = b_rx.recv();
            b_tx.send(v);
            if v == STOP {
                break;
            }
        }
    });
    let _ = qos::set_current_thread(Qos::UserInteractive);
    let f = clock::counter_frequency() as f64;
    let warm = n / 10 + 1000;
    for i in 0..warm as u64 {
        a_tx.send(i);
        assert_eq!(a_rx.recv(), i);
    }

    let u0 = Usage::now();
    let t0 = clock::counter();
    for i in 0..n as u64 {
        a_tx.send(i);
        let r = a_rx.recv();
        debug_assert_eq!(r, i);
        std::hint::black_box(r);
    }
    let t1 = clock::counter();

    let mut d: Vec<u64> = vec![0; n];
    for (i, slot) in d.iter_mut().enumerate() {
        let s = clock::counter();
        a_tx.send(i as u64);
        let r = a_rx.recv();
        let e = clock::counter();
        std::hint::black_box(r);
        *slot = e - s;
    }
    let invol = Usage::now().since(&u0).involuntary_switches;

    a_tx.send(STOP);
    assert_eq!(a_rx.recv(), STOP);
    echo.join().unwrap();

    d.sort_unstable();
    let ns = |u: u64| u as f64 * 1e9 / f;
    Stats {
        batch_mean_ns: (t1 - t0) as f64 * 1e9 / f / n as f64,
        p50: ns(pct(&d, 0.50)),
        p90: ns(pct(&d, 0.90)),
        p99: ns(pct(&d, 0.99)),
        p999: ns(pct(&d, 0.999)),
        max: ns(*d.last().unwrap()),
        invol,
    }
}

fn report(label: &str, epoch: usize, n: usize, s: &Stats) {
    println!(
        "pp {label} epoch={epoch} round_trips={n} batch_rtt_ns={:.1} oneway_ns={:.1} per_rt_p50_ns={:.1} p90_ns={:.1} p99_ns={:.1} p999_ns={:.1} max_ns={:.1} invol_switches={}",
        s.batch_mean_ns, s.batch_mean_ns / 2.0, s.p50, s.p90, s.p99, s.p999, s.max, s.invol
    );
}

fn ring_case<O: Order, P: IndexCell, W: Wait, const C: bool>(epoch: usize, n: usize) {
    let (a_tx, b_rx) = channel::<O, P, W, C>(CAPACITY);
    let (b_tx, a_rx) = channel::<O, P, W, C>(CAPACITY);
    let s = measure(a_tx, a_rx, b_tx, b_rx, n);
    let label = format!("impl=ring order={} pad={} cached={} wait={}", O::NAME, P::ALIGN, C as u8, W::NAME);
    report(&label, epoch, n, &s);
}

fn mutex_case(epoch: usize, n: usize) {
    let (a_tx, b_rx) = titan_spsc::baseline::channel(CAPACITY);
    let (b_tx, a_rx) = titan_spsc::baseline::channel(CAPACITY);
    let s = measure(a_tx, a_rx, b_tx, b_rx, n);
    report("impl=mutex order=na pad=na cached=na wait=isb", epoch, n, &s);
}

type Case = fn(usize, usize);

fn main() {
    let args: Vec<String> = std::env::args().collect();
    let n: usize = args.get(1).and_then(|v| v.parse().ok()).unwrap_or(200_000);
    let epochs: usize = args.get(2).and_then(|v| v.parse().ok()).unwrap_or(3);

    let cases: Vec<Case> = vec![
        ring_case::<AcqRel, Pad128, SpinHint, true>,
        ring_case::<AcqRel, Pad128, SpinHint, false>,
        ring_case::<AcqRel, Pad64, SpinHint, true>,
        ring_case::<AcqRel, Pad64, SpinHint, false>,
        ring_case::<SeqCst, Pad128, SpinHint, true>,
        ring_case::<SeqCst, Pad128, SpinHint, false>,
        ring_case::<SeqCst, Pad64, SpinHint, true>,
        ring_case::<SeqCst, Pad64, SpinHint, false>,
        ring_case::<Fenced, Pad128, SpinHint, true>,
        ring_case::<Fenced, Pad128, SpinHint, false>,
        ring_case::<Fenced, Pad64, SpinHint, true>,
        ring_case::<Fenced, Pad64, SpinHint, false>,
        ring_case::<AcqRel, Pad128, Spin, true>,
        ring_case::<AcqRel, Pad128, Yield, true>,
        ring_case::<AcqRel, Pad128, Wfe, true>,
        mutex_case,
    ];

    println!(
        "pp_config round_trips={n} epochs={epochs} cases={} counter_hz={} capacity={CAPACITY}",
        cases.len(),
        clock::counter_frequency()
    );
    // Rotate the starting case each epoch so slow drift (thermal, background
    // activity) does not systematically favour cases that run first.
    for epoch in 0..epochs {
        for k in 0..cases.len() {
            let idx = (k + epoch * 5) % cases.len();
            cases[idx](epoch, n);
            std::thread::sleep(Duration::from_millis(50));
        }
    }
}
