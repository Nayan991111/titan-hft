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
use titan_spsc::order::{AcqRel, Fenced, Order, SeqCst};
use titan_spsc::pad::{IndexCell, Pad128};
use titan_spsc::ring::{channel_slot, same_line_128, Slot, Slot128};
use titan_spsc::wait::{Spin, SpinHint, Wait, Wfe, Yield};
use titan_spsc::{Rx, Tx};

const CAPACITY: usize = 1024;

/// Measure one arm. Returns true when the consumer's sum was exact.
fn case<S: Slot + 'static, O: Order, P: IndexCell, W: Wait, const C: bool>(
    epoch: usize,
    n: usize,
    pmu: Option<&Session>,
    cap: usize,
) -> bool {
    let warm = n / 10 + 1000;
    let total = (warm + n) as u64;
    let (mut tx, mut rx) = channel_slot::<S, O, P, W, C>(cap);
    let (ha, ta) = tx.inner.index_addresses();
    let layout = format!(
        " head_off128={} tail_off128={} same_line={} slot_bytes={} slot_footprint={}",
        ha % 128,
        ta % 128,
        same_line_128(ha, ta) as u8,
        S::BYTES,
        cap * S::BYTES
    );
    let (sum, u0, c0, t0, t1, c1);
    if producer_side() {
        // The consumer is spawned and the producer keeps the measuring thread.
        let consumer = std::thread::spawn(move || {
            let _ = qos::set_current_thread(Qos::UserInteractive);
            for _ in 0..warm {
                std::hint::black_box(rx.recv());
            }
            let mut s = 0u64;
            for _ in 0..n {
                s = s.wrapping_add(rx.recv());
            }
            s
        });
        let _ = qos::set_current_thread(Qos::UserInteractive);
        for i in 0..warm as u64 {
            tx.send(i);
        }
        u0 = Usage::now();
        c0 = pmu.map(|s| s.read());
        t0 = clock::counter();
        for i in warm as u64..total {
            tx.send(i);
        }
        t1 = clock::counter();
        c1 = pmu.map(|s| s.read());
        sum = consumer.join().unwrap();
    } else {
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
        u0 = Usage::now();
        c0 = pmu.map(|s| s.read());
        t0 = clock::counter();
        let mut s = 0u64;
        for _ in 0..n {
            s = s.wrapping_add(rx.recv());
        }
        t1 = clock::counter();
        c1 = pmu.map(|s| s.read());
        sum = s;
        producer.join().unwrap();
    }
    let invol = Usage::now().since(&u0).involuntary_switches;

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
            // The field names follow the counters that were actually
            // programmed. Emitting store-probe data under msg_l1d_miss would
            // put a wrong name on a committed record, which is exactly the
            // class of defect this register keeps recording.
            let spec = probe_spec();
            let f = |v: f64, p: usize| format!("{:.*}", p, v);
            format!(
                " msg_cycles={:.3} {}={} {}={} {}={} core_ghz={:.3}",
                per(d[0]),
                spec.fields[0], f(per(d[1]), spec.decimals[0]),
                spec.fields[1], f(per(d[2]), spec.decimals[1]),
                spec.fields[2], f(per(d[3]), spec.decimals[2]),
                d[0] as f64 / secs / 1e9
            )
        }
        _ => String::new(),
    };
    println!(
        "bd impl=ring order={} pad={} cap={cap} cached={} wait={} epoch={epoch} messages={n} epoch_ms={:.3} ns_per_msg={ns_per_msg:.3} sum_ok={} invol_switches={invol}{layout}{pmu_txt}",
        O::NAME,
        P::ALIGN,
        C as u8,
        W::NAME,
        secs * 1e3,
        sum_ok as u8,
    );
    sum_ok
}

/// Whether the store-side counter pair was requested. Read once from the
/// command line and cached, so the record writer and the session builder
/// cannot disagree about which counters are in use.
/// Measure the producer instead of the consumer.
///
/// The counters are read by whichever thread calls `Session::read`, and
/// counting is per thread. Until now that thread ran the consumer loop, so
/// every committed counter field in this harness describes the consumer. The
/// arms under test differ in the producer's store instruction: the census has
/// `push_acqrel` emitting `stlr` and `pop_acqrel` emitting `stlur`. H78 part
/// (a) asked whether the faster arm takes fewer retired store misses, and
/// read it from the side that does not execute the instruction in question.
/// That is D22.
///
/// This mode spawns the consumer and runs the producer on the main thread, so
/// the counter reads and the timing window sit around the send loop. Nothing
/// about the ring, the arms, the epoch structure or the record format changes,
/// and the default path is untouched, so the earlier runs stay comparable to
/// each other. Whether they stay comparable to this one is a question for the
/// registration, not an assumption: H79 fixes a band on ns_per_msg before the
/// run.
fn producer_side() -> bool {
    use std::sync::OnceLock;
    static F: OnceLock<bool> = OnceLock::new();
    *F.get_or_init(|| std::env::args().any(|a| a == "prodside"))
}

fn store_probe() -> bool {
    use std::sync::OnceLock;
    static F: OnceLock<bool> = OnceLock::new();
    *F.get_or_init(|| std::env::args().any(|a| a == "storeprobe"))
}

/// What a counter mode programs and what it calls the fields it emits.
///
/// The two were separate before, an array of events in one place and a pair of
/// field names in another, and they drifted: a diagnostic read the store miss
/// column at a fixed index that held the instruction counter under one of the
/// two event sets, and printed instruction counts under a store miss heading.
/// That is D21. Keeping them in one structure means a mode cannot name a field
/// it did not program, and `assert_programmed` then checks that the hardware
/// resolved the events this mode asked for, in the order it asked for them,
/// before a single record is written.
struct ProbeSpec {
    name: &'static str,
    events: [&'static [&'static str]; 4],
    fields: [&'static str; 3],
    decimals: [usize; 3],
}

/// Vary the wait strategy at a fixed arm instead of the arm at a fixed wait.
fn wait_sweep() -> bool {
    use std::sync::OnceLock;
    static F: OnceLock<bool> = OnceLock::new();
    *F.get_or_init(|| std::env::args().any(|a| a == "waitsweep"))
}

/// Sweep ordering strength rather than wait strategy or capacity.
///
/// The measured arms until now were AcqRel and FencedStore, which on this
/// part are `ldapr` acquire against `ldapr` acquire with `dmb ish` before a
/// plain store. Those are the two ends of a three-point spectrum and the
/// middle point was never run, although it has been implemented the whole
/// time: `SeqCst` lowers to `ldar`, the RCsc acquire, which orders against
/// earlier loads in a way `ldapr` does not and still costs no explicit
/// barrier.
///
/// Measuring it separates two accounts of the central result that the two-arm
/// comparison cannot tell apart: that the cost difference comes from the
/// acquire flavour, or that it comes from the explicit barrier. The arms are
/// otherwise identical, so the ring, the slot, the padding, the wait strategy
/// and the epoch structure are held fixed and only the index access policy
/// moves.
fn ord_sweep() -> bool {
    use std::sync::OnceLock;
    static F: OnceLock<bool> = OnceLock::new();
    *F.get_or_init(|| std::env::args().any(|a| a == "ordsweep"))
}

/// Vary the ring capacity at a fixed arm and wait strategy.
fn cap_sweep() -> bool {
    use std::sync::OnceLock;
    static F: OnceLock<bool> = OnceLock::new();
    *F.get_or_init(|| std::env::args().any(|a| a == "capsweep"))
}

/// Capacities in case order, so the record carries the one it ran under.
const CAP_SWEEP: [usize; 8] = [2, 16, 128, 1024, 2, 16, 128, 1024];

/// A finer sweep around the capacity at which the comparison changes sign.
///
/// H83 found the acquire-release arm faster at capacity 2, by 1.0888, and the
/// fenced-store arm faster by 2.1759 at 16, 2.6359 at 128 and 2.4444 at 1024.
/// The crossover therefore lies between 2 and 16 and nothing in this project
/// had looked there. These five capacities bracket it.
const CAP_FINE: [usize; 10] = [2, 4, 8, 16, 1024, 2, 4, 8, 16, 1024];

/// Vary the ring capacity finely, to locate where the comparison changes sign.
fn cap_fine() -> bool {
    use std::sync::OnceLock;
    static F: OnceLock<bool> = OnceLock::new();
    *F.get_or_init(|| std::env::args().any(|a| a == "capfine"))
}

/// Separate the slot array's footprint from the slot count and the run-ahead.
///
/// H84 found the acquire-release arm breaking by 2.716 in cost between
/// capacities 8 and 16 while the fenced-store arm moved by 0.935. In that
/// design an eight byte slot and a page aligned region meant capacity alone
/// set the footprint, which first fills a 128 byte line at exactly capacity
/// 16, and capacity alone set how far the producer may run ahead. Three
/// candidate causes moved together.
///
/// This is the two by two that separates them: each arm at capacity 8 and 16,
/// each with an eight byte slot and with a 128 byte padded slot. The padded
/// slot makes the footprint many cache lines at both capacities while leaving
/// the slot count and the run-ahead alone, so if the break survives there it
/// was never about the footprint. H85.
fn pad_sweep() -> bool {
    use std::sync::OnceLock;
    static F: OnceLock<bool> = OnceLock::new();
    *F.get_or_init(|| std::env::args().any(|a| a == "padsweep"))
}

/// Capacities in case order for the footprint cross.
const PAD_SWEEP: [usize; 8] = [8, 16, 8, 16, 8, 16, 8, 16];

fn probe_spec() -> &'static ProbeSpec {
    const DEFAULT: ProbeSpec = ProbeSpec {
        name: "default",
        events: [events::CYCLES, events::INSTRUCTIONS, events::L1D_LOAD_MISS,
                 events::BRANCH_MISPREDICT],
        fields: ["msg_instructions", "msg_l1d_miss", "msg_br_mispred"],
        decimals: [2, 4, 4],
    };
    const STORE: ProbeSpec = ProbeSpec {
        name: "storeprobe",
        events: [events::CYCLES, events::INSTRUCTIONS, events::L1D_STORE_MISS,
                 events::LDST_WAIT_MISS],
        fields: ["msg_instructions", "msg_l1d_store_miss", "msg_ldst_wait_cycles"],
        decimals: [2, 4, 4],
    };
    // The throttling question, on counters that can answer it. Writebacks are
    // the ownership transfer a contended cursor forces, and memory order
    // violations are a mechanism for the inversion rather than a correlate.
    const VIOL: ProbeSpec = ProbeSpec {
        name: "violprobe",
        events: [events::CYCLES, events::L1D_WRITEBACK, events::MEM_ORDER_VIOL,
                 events::LDST_WAIT_MISS],
        fields: ["msg_l1d_writeback", "msg_mem_order_viol", "msg_ldst_wait_cycles"],
        decimals: [4, 4, 4],
    };
    // ST_MEM_ORDER_VIOL_LD_NONSPEC read zero in H80, H81, H82 and in a
    // validation pass whose aliasing loop was built to trigger violations, so
    // it is not carried into a new mode. L1D_CACHE_REFILL is the inbound half
    // of the same coherence traffic and does count. See D27.
    const TRAFFIC: ProbeSpec = ProbeSpec {
        name: "trafficprobe",
        events: [events::CYCLES, events::L1D_WRITEBACK, events::L1D_REFILL,
                 events::LDST_WAIT_MISS],
        fields: ["msg_l1d_writeback", "msg_l1d_refill", "msg_ldst_wait_cycles"],
        decimals: [4, 4, 4],
    };
    // The one mechanism in the audit list that no run has tested: that a full
    // barrier is faster under contention because it suppresses the issue rate
    // and so prevents a coherence storm, rather than because it reorders less.
    // Issue-active cycles over total cycles is that quantity directly.
    const ISSUE: ProbeSpec = ProbeSpec {
        name: "issueprobe",
        events: [events::CYCLES, events::ISSUE_ACTIVE, events::STALL_BACKEND,
                 events::RETIRED_UOPS],
        fields: ["msg_issue_active", "msg_stall_backend", "msg_retired_uops"],
        decimals: [4, 4, 2],
    };
    // Where the 201 cycles per message go. H86 accounted for 7.77 percent of
    // the acquire-release arm's contended cost as issuing and 0.16 percent as
    // a backend stall, and left the rest unlocated. ARM_STALL is every cycle
    // with no operation sent for execution, and the frontend counter splits
    // it, so these three plus cycles either partition the time or show that
    // they cannot be made to.
    const STALLS: ProbeSpec = ProbeSpec {
        name: "stallprobe",
        events: [events::CYCLES, events::STALL_ANY, events::STALL_FRONTEND,
                 events::STALL_BACKEND],
        fields: ["msg_stall_any", "msg_stall_frontend", "msg_stall_backend"],
        decimals: [4, 4, 4],
    };
    if std::env::args().any(|a| a == "stallprobe") {
        &STALLS
    } else if std::env::args().any(|a| a == "issueprobe") {
        &ISSUE
    } else if std::env::args().any(|a| a == "trafficprobe") {
        &TRAFFIC
    } else if std::env::args().any(|a| a == "violprobe") {
        &VIOL
    } else if store_probe() {
        &STORE
    } else {
        &DEFAULT
    }
}

/// Refuse to measure if the hardware did not give this mode the events it
/// asked for, in order. A counter that silently resolved to something else, or
/// to nothing, is how nine hundred records came to carry a field that was
/// never programmed. Failing here costs one run; not failing here cost two.
fn assert_programmed(s: &Session) {
    let spec = probe_spec();
    let got = s.names();
    if got.len() != 4 {
        eprintln!("pmu programmed {} counters, expected 4, mode {}", got.len(), spec.name);
        std::process::exit(3);
    }
    for (i, cands) in spec.events.iter().enumerate() {
        if !cands.contains(&got[i].as_str()) {
            eprintln!("pmu slot {i} resolved to {} which is not among {:?}, mode {}",
                      got[i], cands, spec.name);
            std::process::exit(3);
        }
    }
}

type Case = fn(usize, usize, Option<&Session>, usize) -> bool;

fn main() {
    let args: Vec<String> = std::env::args().collect();
    let n: usize = args.get(1).and_then(|v| v.parse().ok()).unwrap_or(10_000_000);
    let epochs: usize = args.get(2).and_then(|v| v.parse().ok()).unwrap_or(3);
    let pmu = if args.iter().any(|a| a == "pmu") {
        match Kperf::open().and_then(|k| {
            // The mode decides the events and the field names together, so
            // the default stays exactly as it was and every earlier results
            // file remains comparable. See probe_spec and D21.
            k.session(&probe_spec().events)
        }) {
            Ok(s) => {
                assert_programmed(&s);
                Some(s)
            }
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
    // Holding the arm fixed and varying only how it waits separates coherence
    // traffic caused by the ordering instruction from traffic caused by the
    // time spent polling. Writebacks per message correlate with cycles per
    // message at r above 0.98 across the H80 and H81 cells, which is what a
    // spin wait produces mechanically: a slower arm polls a remote cursor
    // more times and moves the line more often. A per message rate therefore
    // cannot tell a cause from a consequence, which is D28. Two arms across
    // four wait strategies, contended only, puts both on the same duration
    // axis without changing the instruction under test.
    // The wait strategy moved acqrel's cost by 1.3295 and fencedstore's by only
    // 1.1137, because fencedstore is dominated by the barrier and not by
    // waiting, so H82's duration axis existed for one arm and not the other and
    // its regression across them was unevaluable. That is D29. Ring capacity is
    // a lever that acts on both: a smaller ring forces the producer to wait on
    // the consumer's cursor more often whatever the ordering costs.
    let cases: Vec<(Case, bool)> = if pad_sweep() {
        vec![
            (case::<u64, AcqRel, Pad128, SpinHint, false>, true),
            (case::<u64, AcqRel, Pad128, SpinHint, false>, true),
            (case::<Slot128, AcqRel, Pad128, SpinHint, false>, true),
            (case::<Slot128, AcqRel, Pad128, SpinHint, false>, true),
            (case::<u64, FencedStore, Pad128, SpinHint, false>, true),
            (case::<u64, FencedStore, Pad128, SpinHint, false>, true),
            (case::<Slot128, FencedStore, Pad128, SpinHint, false>, true),
            (case::<Slot128, FencedStore, Pad128, SpinHint, false>, true),
        ]
    } else if ord_sweep() {
        // Three ordering policies, each contended and each resident. The
        // contended cells decide H90; the resident cells are the control,
        // because an ordering spread that survives line residency is not
        // about coherence traffic.
        vec![
            (case::<u64, AcqRel, Pad128, SpinHint, false>, true),
            (case::<u64, SeqCst, Pad128, SpinHint, false>, true),
            (case::<u64, FencedStore, Pad128, SpinHint, false>, true),
            (case::<u64, AcqRel, Pad128, SpinHint, true>, true),
            (case::<u64, SeqCst, Pad128, SpinHint, true>, true),
            (case::<u64, FencedStore, Pad128, SpinHint, true>, true),
        ]
    } else if cap_fine() {
        vec![
            (case::<u64, AcqRel, Pad128, SpinHint, false>, true),
            (case::<u64, AcqRel, Pad128, SpinHint, false>, true),
            (case::<u64, AcqRel, Pad128, SpinHint, false>, true),
            (case::<u64, AcqRel, Pad128, SpinHint, false>, true),
            (case::<u64, AcqRel, Pad128, SpinHint, false>, true),
            (case::<u64, FencedStore, Pad128, SpinHint, false>, true),
            (case::<u64, FencedStore, Pad128, SpinHint, false>, true),
            (case::<u64, FencedStore, Pad128, SpinHint, false>, true),
            (case::<u64, FencedStore, Pad128, SpinHint, false>, true),
            (case::<u64, FencedStore, Pad128, SpinHint, false>, true),
        ]
    } else if cap_sweep() {
        vec![
            (case::<u64, AcqRel, Pad128, SpinHint, false>, true),
            (case::<u64, AcqRel, Pad128, SpinHint, false>, true),
            (case::<u64, AcqRel, Pad128, SpinHint, false>, true),
            (case::<u64, AcqRel, Pad128, SpinHint, false>, true),
            (case::<u64, FencedStore, Pad128, SpinHint, false>, true),
            (case::<u64, FencedStore, Pad128, SpinHint, false>, true),
            (case::<u64, FencedStore, Pad128, SpinHint, false>, true),
            (case::<u64, FencedStore, Pad128, SpinHint, false>, true),
        ]
    } else if wait_sweep() {
        vec![
            (case::<u64, AcqRel, Pad128, Spin, false>, true),
            (case::<u64, AcqRel, Pad128, SpinHint, false>, true),
            (case::<u64, AcqRel, Pad128, Yield, false>, true),
            (case::<u64, AcqRel, Pad128, Wfe, false>, true),
            (case::<u64, FencedStore, Pad128, Spin, false>, true),
            (case::<u64, FencedStore, Pad128, SpinHint, false>, true),
            (case::<u64, FencedStore, Pad128, Yield, false>, true),
            (case::<u64, FencedStore, Pad128, Wfe, false>, true),
        ]
    } else {
        vec![
        (case::<u64, AcqRel, Pad128, SpinHint, false>, true),
        (case::<u64, FencedLoad, Pad128, SpinHint, false>, true),
        (case::<u64, FencedStore, Pad128, SpinHint, false>, true),
        (case::<u64, Fenced, Pad128, SpinHint, false>, true),
        (case::<u64, Unordered, Pad128, SpinHint, false>, false),
        (case::<u64, AcqRel, Pad128, SpinHint, true>, true),
        (case::<u64, FencedLoad, Pad128, SpinHint, true>, true),
        (case::<u64, FencedStore, Pad128, SpinHint, true>, true),
        (case::<u64, Fenced, Pad128, SpinHint, true>, true),
        (case::<u64, Unordered, Pad128, SpinHint, true>, false),
        ]
    };
    // One capacity per case, in case order, so a record carries the ring it ran
    // under rather than a constant that stopped being true.
    let caps: Vec<usize> = if pad_sweep() {
        PAD_SWEEP.to_vec()
    } else if cap_fine() {
        CAP_FINE.to_vec()
    } else if cap_sweep() {
        CAP_SWEEP.to_vec()
    } else {
        vec![CAPACITY; cases.len()]
    };
    assert_eq!(caps.len(), cases.len(), "one capacity per case");

    println!(
        "bd_config messages={n} epochs={epochs} cases={} capacity={} pmu={} measured_side={} counters={} arms={}",
        cases.len(),
        if cap_sweep() || cap_fine() || pad_sweep() { "swept".to_string() } else { CAPACITY.to_string() },
        pmu.is_some(),
        if producer_side() { "producer" } else { "consumer" },
        probe_spec().name,
        if pad_sweep() {
            "padsweep"
        } else if ord_sweep() {
            "ordsweep"
        } else if cap_fine() {
            "capfine"
        } else if cap_sweep() {
            "capsweep"
        } else if wait_sweep() {
            "waitsweep"
        } else {
            "orders"
        }
    );
    // Arms rotate by a stride coprime with the arm count so that position in
    // the epoch is not fixed per arm; 3 and 10 are coprime.
    let mut broken: Vec<String> = Vec::new();
    for epoch in 0..epochs {
        for k in 0..cases.len() {
            let idx = (k + epoch * 3) % cases.len();
            let (f, must_be_exact) = cases[idx];
            let ok = f(epoch, n, pmu.as_ref(), caps[idx]);
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
