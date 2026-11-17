//! Barrier-placement decomposition harness (H69).
//!
//! `titan-stream` measures AcqRel against Fenced, which substitutes the load
//! side and the store side at the same time, so it cannot say which side the
//! measured inversion comes from. This harness crosses the two halves:
//!
//!   acqrel       ldapr            stlr
//!   fencedload   ldr + dmb ish    stlr
//!   fencedstore  ldapr            dmb ish + str
//!   fenced       ldr + dmb ish    dmb ish + str
//!   unordered    ldr              str          (cost floor, not a correct ring)
//!
//! Each arm runs both cached and uncached at pad=128 on performance cores, the
//! configuration in which the inversion was measured at 0.4475 uncached and
//! 2.449 cached.
//!
//! Two differences from `titan-stream`, both deliberate:
//!
//! * The sum check is **reported** as `sum_ok=0|1`, not asserted. The
//!   `unordered` arm is permitted to lose ordering by construction, and an
//!   abort there would destroy a sweep the other nine arms are halfway
//!   through. A failure in any ordered arm still fails the process, at the end,
//!   once every arm has been measured and printed.
//! * Every line carries `epoch_ms` and `core_ghz`. The clock on this part
//!   tracks how long the core has been busy rather than how much work it did,
//!   so a clock figure is only interpretable next to the duration that
//!   produced it. A registration that thresholds the clock without the
//!   duration is not checkable, which is how H68 (d) came to fail against a
//!   correct run.
//!
//!   titan-barrier [messages] [epochs] [pmu]

use std::time::Duration;
use titan_platform::clock;
use titan_platform::kperf::{events, Kperf, Session};
use titan_platform::qos::{self, Qos};
use titan_platform::rusage::Usage;
use titan_spsc::barrier::{FencedLoad, FencedStore, Unordered};
use titan_spsc::order::{AcqRel, Fenced, Order};
use titan_spsc::pad::{IndexCell, Pad128};
use titan_spsc::ring::{channel, same_line_128};
use titan_spsc::wait::{SpinHint, Wait};
use titan_spsc::{Rx, Tx};

const CAPACITY: usize = 1024;

/// Measure one arm. Returns true when the consumer's sum was exact.
fn case<O: Order, P: IndexCell, W: Wait, const C: bool>(
    epoch: usize,
    n: usize,
    pmu: Option<&Session>,
) -> bool {
    let warm = n / 10 + 1000;
    let total = (warm + n) as u64;
    let (mut tx, mut rx) = channel::<O, P, W, C>(CAPACITY);
    let (ha, ta) = tx.inner.index_addresses();
    let layout = format!(
        " head_off128={} tail_off128={} same_line={}",
        ha % 128,
        ta % 128,
        same_line_128(ha, ta) as u8
    );
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
    let sum_ok = sum == expected;

    let f = clock::counter_frequency() as f64;
    let secs = (t1 - t0) as f64 / f;
    let ns_per_msg = secs * 1e9 / n as f64;
    let pmu_txt = match (pmu, c0, c1) {
        (Some(s), Some(a), Some(b)) => {
            let d = s.delta(&a, &b);
            let per = |x: u64| x as f64 / n as f64;
            format!(
                " msg_cycles={:.3} msg_instructions={:.2} msg_l1d_miss={:.4} msg_br_mispred={:.4} core_ghz={:.3}",
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
        "bd impl=ring order={} pad={} cached={} wait={} epoch={epoch} messages={n} epoch_ms={:.3} ns_per_msg={ns_per_msg:.3} sum_ok={} invol_switches={invol}{layout}{pmu_txt}",
        O::NAME,
        P::ALIGN,
        C as u8,
        W::NAME,
        secs * 1e3,
        sum_ok as u8,
    );
    sum_ok
}

type Case = fn(usize, usize, Option<&Session>) -> bool;

fn main() {
    let args: Vec<String> = std::env::args().collect();
    let n: usize = args.get(1).and_then(|v| v.parse().ok()).unwrap_or(10_000_000);
    let epochs: usize = args.get(2).and_then(|v| v.parse().ok()).unwrap_or(3);
    let pmu = if args.iter().any(|a| a == "pmu") {
        match Kperf::open().and_then(|k| {
            k.session(&[
                events::CYCLES,
                events::INSTRUCTIONS,
                events::L1D_LOAD_MISS,
                events::BRANCH_MISPREDICT,
            ])
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

    // Index 9 is `unordered`, the cost floor, which is permitted to lose
    // ordering. Every other arm is a correct SPSC ring and a failed sum there
    // fails the process. The pairing is held in one place so the two lists
    // cannot drift apart.
    let cases: Vec<(Case, bool)> = vec![
        (case::<AcqRel, Pad128, SpinHint, false>, true),
        (case::<FencedLoad, Pad128, SpinHint, false>, true),
        (case::<FencedStore, Pad128, SpinHint, false>, true),
        (case::<Fenced, Pad128, SpinHint, false>, true),
        (case::<Unordered, Pad128, SpinHint, false>, false),
        (case::<AcqRel, Pad128, SpinHint, true>, true),
        (case::<FencedLoad, Pad128, SpinHint, true>, true),
        (case::<FencedStore, Pad128, SpinHint, true>, true),
        (case::<Fenced, Pad128, SpinHint, true>, true),
        (case::<Unordered, Pad128, SpinHint, true>, false),
    ];
    println!(
        "bd_config messages={n} epochs={epochs} cases={} capacity={CAPACITY} pmu={}",
        cases.len(),
        pmu.is_some()
    );
    // Arms rotate by a stride coprime with the arm count so that position in
    // the epoch is not fixed per arm; 3 and 10 are coprime.
    let mut broken: Vec<String> = Vec::new();
    for epoch in 0..epochs {
        for k in 0..cases.len() {
            let idx = (k + epoch * 3) % cases.len();
            let (f, must_be_exact) = cases[idx];
            let ok = f(epoch, n, pmu.as_ref());
            if must_be_exact && !ok {
                broken.push(format!("case{idx}/epoch{epoch}"));
            }
            std::thread::sleep(Duration::from_millis(50));
        }
    }
    if !broken.is_empty() {
        eprintln!("ordered arm lost or duplicated a message: {}", broken.join(" "));
        std::process::exit(3);
    }
}
