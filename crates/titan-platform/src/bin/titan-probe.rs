//! Self-test for the platform layer. Prints `key=value` lines.
//!
//!   titan-probe            portable checks (no root)
//!   sudo titan-probe pmu   also programs the PMU and validates the counters
//!
//! The PMU validation runs a loop whose instruction count is known exactly
//! and whose cycle count is bounded by a dependency chain, so the counters
//! and the derived core frequency can be checked against first principles.

use titan_platform::alloc_count::{thread_allocations, CountingAlloc};
use titan_platform::clock;
use titan_platform::kperf::{events, CountMode, Kperf};
use titan_platform::mem::{page_size, PinnedRegion};
use titan_platform::qos::{self, Qos};
use titan_platform::rusage::Usage;

#[global_allocator]
static A: CountingAlloc = CountingAlloc;

fn main() {
    let pmu = std::env::args().any(|a| a == "pmu");
    if std::env::args().any(|a| a == "events") {
        dump_events();
        return;
    }
    println!("== platform");
    println!("page_size={}", page_size());
    let (n, d) = clock::mach_timebase();
    println!("mach_timebase={n}/{d}");
    let f = clock::counter_frequency();
    println!("counter_hz={f}");
    println!("counter_ns_per_tick={:.4}", 1e9 / f as f64);
    println!("qos_user_interactive={:?}", qos::set_current_thread(Qos::UserInteractive));

    println!("== timer");
    let r = clock::resolution(10_000_000);
    let ns = |units: f64| units * 1e9 / f as f64;
    println!("counter_min_step_units={} ({:.2} ns)", r.min_step, ns(r.min_step as f64));
    println!("counter_modal_step_units={} ({:.2} ns)", r.modal_step, ns(r.modal_step as f64));
    println!("counter_zero_fraction={:.4}", r.zero_fraction);
    println!("counter_read_cost_ns_est={:.2}", ns(r.mean_delta));
    println!("counter_outliers_gt_8_steps={} per_million={:.2}", r.outliers, r.outliers as f64 * 1e6 / r.pairs as f64);
    let counts = clock::nonzero_delta_counts(10_000_000);
    let nz: u64 = counts.iter().map(|c| c.1).sum();
    let top: Vec<String> = counts.iter().take(8).map(|(d, c)| format!("{d}:{c}")).collect();
    println!("counter_nonzero_deltas={} top_values={}", nz, top.join(","));
    let below_modal: u64 = counts.iter().filter(|(d, _)| *d < r.modal_step.saturating_sub(1)).map(|c| c.1).sum();
    let small: Vec<String> = {
        let mut v: Vec<&(u64, u64)> = counts.iter().filter(|(d, _)| *d < r.modal_step.saturating_sub(1)).collect();
        v.sort_by_key(|x| x.0);
        v.iter().take(10).map(|(d, c)| format!("{d}:{c}")).collect()
    };
    println!("counter_deltas_below_one_step={} values={}", below_modal, small.join(","));

    println!("== memory");
    let mut r = PinnedRegion::new(64 << 20).expect("mmap");
    println!("region_bytes={} pages={} aligned={}", r.len(), r.pages(), (r.as_ptr() as usize).is_multiple_of(r.page_size()));
    println!("mlock_ok={} mlock_errno={:?}", r.is_locked(), r.lock_error());
    println!("resident_pages={:?}", r.resident_pages());
    let page = r.page_size();
    let before = Usage::now();
    {
        let s = r.as_mut_slice();
        for pass in 0..4u8 {
            for off in (0..s.len()).step_by(page) {
                unsafe { std::ptr::write_volatile(&mut s[off], pass) };
            }
        }
    }
    let du = Usage::now().since(&before);
    println!("hot_write_minor_faults={} major_faults={} invol_switches={}", du.minor_faults, du.major_faults, du.involuntary_switches);

    println!("== allocation");
    let a0 = thread_allocations();
    let mut acc = 0u64;
    {
        let s = r.as_mut_slice();
        for i in 0..10_000_000usize {
            let j = (i.wrapping_mul(64)) & (s.len() - 1);
            s[j] = s[j].wrapping_add(1);
            acc = acc.wrapping_add(s[j] as u64);
        }
    }
    std::hint::black_box(acc);
    println!("hot_loop_allocations={}", thread_allocations() - a0);

    println!("== pmu");
    let k = match Kperf::open() {
        Ok(k) => k,
        Err(e) => {
            println!("kperf_available=false reason=\"{e}\"");
            return;
        }
    };
    println!("kperf_available=true db={} cpu=\"{}\" pmu_version={}", k.database_name(), k.cpu_string(), k.pmu_version());
    match k.events() {
        Ok(evs) => {
            println!("event_count={}", evs.len());
            for e in &evs {
                println!("event name={} alias=\"{}\" desc=\"{}\"", e.name, e.alias, e.description);
            }
        }
        Err(e) => println!("event_list_error=\"{e}\""),
    }
    if !pmu {
        println!("pmu_validation=skipped (run: sudo target/release/titan-probe pmu)");
        return;
    }
    println!("pmu_permission={}", k.has_permission());
    drop(k);
    // Alternating order so run-order drift cannot masquerade as a mode effect.
    for mode in [CountMode::All, CountMode::UserOnly, CountMode::All, CountMode::UserOnly] {
        run_pmu(mode);
    }
    storemiss_test();
    storemiss_isolation();
    violation_check();
}

/// Can `ST_MEM_ORDER_VIOL_LD_NONSPEC` read non-zero on this part.
///
/// H80 and H81 recorded exactly 0.0000 memory order violations on every arm of
/// 1800 records, on both threads, and the registered part that rested on it is
/// refuted on that basis. A refutation built on a counter that has never been
/// seen to move is the D19 mistake with a different event, so the zero is not
/// a result until the counter is shown to discriminate.
///
/// A violation happens when a load executes speculatively ahead of an older
/// store it aliases, and is detected when the store resolves, costing a flush
/// and replay. Two passes are run and their expected order is fixed here,
/// before the counter is read.
///
/// The aliasing pass makes the store's address depend on a pointer chase, so
/// it resolves late and the following load issues first, and picks the load's
/// address to collide with the store's on an unpredictable half of iterations,
/// so the memory disambiguation predictor cannot learn to hold the load back.
/// The control pass is the same loop with the collision removed: the load
/// always reads a different slot. Violations should be far more frequent in
/// the first than in the second.
///
/// If the aliasing pass counts and the control does not, the counter works and
/// discriminates, and the ring's zero is a measurement. If both read zero, the
/// event is inert here and every claim resting on it is unevaluable rather
/// than refuted.
fn violation_check() {
    const N: usize = 4_000_000;
    const SLOTS: usize = 8;

    println!("-- memory order violation counter, can it move at all");
    let k = match Kperf::open() {
        Ok(k) => k,
        Err(e) => {
            println!("violcheck_open_error=\"{e}\"");
            return;
        }
    };
    let s = match k.session_with_mode(
        &[events::CYCLES, events::MEM_ORDER_VIOL, events::L1D_WRITEBACK,
          events::LDST_WAIT_MISS],
        CountMode::All,
    ) {
        Ok(s) => s,
        Err(e) => {
            println!("violcheck_session_error=\"{e}\"");
            return;
        }
    };
    println!("violcheck_events {:?}", s.names());

    // A permutation long enough that each step is a dependent cache miss, so
    // the store address below resolves late.
    let n = 1 << 16;
    let mut chain: Vec<usize> = (0..n).collect();
    let mut x = 0x2545F4914F6CDD1Du64;
    for i in (1..n).rev() {
        x ^= x << 13;
        x ^= x >> 7;
        x ^= x << 17;
        let j = (x % (i as u64 + 1)) as usize;
        chain.swap(i, j);
    }
    let mut buf = vec![0u64; SLOTS];

    let mut run = |label: &str, alias: bool| {
        let mut kk = 0usize;
        let mut acc = 0u64;
        let c0 = s.read();
        for i in 0..N {
            kk = chain[kk];
            let slot = kk & (SLOTS - 1);
            unsafe { std::ptr::write_volatile(buf.as_mut_ptr().add(slot), i as u64) };
            let pick = if alias && ((i ^ kk) & 1) == 0 {
                slot
            } else {
                (slot + 1) & (SLOTS - 1)
            };
            acc = acc.wrapping_add(unsafe { std::ptr::read_volatile(buf.as_ptr().add(pick)) });
        }
        let d = s.delta(&c0, &s.read());
        std::hint::black_box(acc);
        let per = N as f64;
        println!("violcheck pass={label} ops={N} mem_order_viol_per_op={:.6} \
writeback_per_op={:.4} wait_cycles_per_op={:.4} cycles_per_op={:.2}",
                 d[1] as f64 / per,
                 d[2] as f64 / per,
                 d[3] as f64 / per,
                 d[0] as f64 / per);
        d[1]
    };

    let aliasing = run("aliasing_expect_violations", true);
    let control = run("control_expect_few", false);
    println!("violcheck_verdict aliasing_total={aliasing} control_total={control} \
counter_discriminates={}",
             if aliasing == 0 && control == 0 {
                 "no, both zero, the event is inert here"
             } else if aliasing > control.saturating_mul(2) {
                 "yes, the aliasing pass counts far more"
             } else {
                 "unclear, the two passes are too close to separate"
             });
    std::hint::black_box(&buf);
}

/// Which of two causes explains the H78 zero.
///
/// `storemiss_test` proved the store miss event counts when it is programmed
/// alongside `L1D_CACHE_MISS_LD` and no fixed instruction counter. The barrier
/// harness asks for a different set, and reads its counters on the consumer
/// thread. Two causes fit, and they are not distinguishable from the
/// repository.
///
/// A. the event cannot be allocated alongside `FIXED_INSTRUCTIONS` and
///    `LDST_UNIT_OLD_L1D_CACHE_MISS`, and the failure is silent.
/// B. the consumer thread genuinely retires no store misses, in which case
///    H78 part (a) was measuring the wrong thread: the arms differ in the
///    producer's store instruction, and `push_acqrel` is the `stlr`.
///
/// This repeats the same three passes under the barrier's exact event set, on
/// one thread whose store misses are known in advance. If the counter reads
/// zero here, cause A holds and the event set is at fault. If it counts, A is
/// refuted and B stands, which means the harness must read the producer's
/// counters before part (a) can be evaluated at all.
fn storemiss_isolation() {
    println!("-- isolation, the barrier harness event set");
    storemiss_passes(
        "barrier_set",
        &[
            events::CYCLES,
            events::INSTRUCTIONS,
            events::L1D_STORE_MISS,
            events::LDST_WAIT_MISS,
        ],
    );
}

/// Does the retired-store-miss counter move at all on this part?
///
/// H78 part (a) could not be evaluated because `L1D_CACHE_MISS_ST_NONSPEC`
/// returned exactly 0.0000 in all 900 records of the H78 run: one distinct
/// value across every arm and both caching states. Three explanations fit
/// that observation equally well from the repository side. The event may be
/// inert on this part. The session may have programmed it into a class that
/// never counts. Or the ring may genuinely retire no store misses even while
/// a cache line is bounced between cores.
///
/// Three passes separate them, because the L1D behaviour of each is known
/// before the counter is read rather than inferred from it.
///
/// Pass one stores one byte per distinct 128-byte line across 64 MiB, which
/// is far larger than the 128 KiB L1D, so close to every store must miss.
/// Pass two stores the same number of times into 4 KiB, which fits, so close
/// to none should. Pass three reads and writes each line over the 64 MiB
/// buffer, so the same line both loads and stores and the two counters see
/// the same misses.
///
/// Pass three is the decisive one. If its load-miss counter moves and its
/// store-miss counter stays at zero, the store event is inert here and H78
/// (a) is unmeasurable on this machine rather than unmeasured. If both move,
/// the fault is in how the barrier session programmed the event and (a) can
/// be rerun. If neither moves, the session is not counting at all and
/// nothing about stores follows from the H78 zero.
fn storemiss_test() {
    storemiss_passes(
        "probe_set",
        &[
            events::CYCLES,
            events::L1D_STORE_MISS,
            events::L1D_LOAD_MISS,
            events::LDST_WAIT_MISS,
        ],
    );
}

/// Run the three known passes under `evs` and print what each counter said.
///
/// Every column is labelled with the event name the session reports, not with
/// a position. The first version of this took the store miss column as index
/// one for both event sets, which is right for the probe's set and wrong for
/// the barrier's, where index one is the fixed instruction counter. It
/// printed instruction counts under a store miss heading, and the only reason
/// the reading was caught is that the 4 KiB pass, which must barely miss at
/// all, came out higher than the 64 MiB pass, which must miss on nearly every
/// store. A column that cannot be checked against a prediction is a column
/// that can lie, so the names now come from the data.
fn storemiss_passes(tag: &str, evs: &[&[&str]]) {
    const LINE: usize = 128;
    const BIG: usize = 64 << 20;
    const SMALL: usize = 4 << 10;
    const STORES: usize = BIG / LINE;

    let k = match Kperf::open() {
        Ok(k) => k,
        Err(e) => {
            println!("storemiss_open_error tag={tag} err=\"{e}\"");
            return;
        }
    };
    let s = match k.session_with_mode(evs, CountMode::All) {
        Ok(s) => s,
        Err(e) => {
            println!("storemiss_session_error tag={tag} err=\"{e}\"");
            return;
        }
    };
    println!("storemiss_events tag={tag} {:?}", s.names());
    println!("storemiss_add_errs tag={tag} {:?}", s.add_errs());

    let mut big = vec![0u8; BIG];
    let mut small = vec![0u8; SMALL];
    // Fault the pages in first, so first-touch page faults are not counted as
    // part of any pass.
    for i in 0..STORES {
        unsafe { std::ptr::write_volatile(big.as_mut_ptr().add(i * LINE), 1u8) }
    }

    let report = |label: &str, n: usize, c0: &titan_platform::kperf::Snapshot| {
        let d = s.delta(c0, &s.read());
        let n = n as f64;
        let per: Vec<String> = s
            .names()
            .iter()
            .enumerate()
            .map(|(i, nm)| format!("{nm}_per_op={:.4}", d[i] as f64 / n))
            .collect();
        println!("storemiss tag={tag} pass={label} ops={n:.0} {}", per.join(" "));
    };

    let c0 = s.read();
    for i in 0..STORES {
        unsafe { std::ptr::write_volatile(big.as_mut_ptr().add(i * LINE), (i & 0xff) as u8) }
    }
    report("store_64mib_expect_miss", STORES, &c0);

    let c0 = s.read();
    for i in 0..STORES {
        let off = (i * LINE) % SMALL;
        unsafe { std::ptr::write_volatile(small.as_mut_ptr().add(off), (i & 0xff) as u8) }
    }
    report("store_4kib_expect_hit", STORES, &c0);

    let c0 = s.read();
    for i in 0..STORES {
        unsafe {
            let p = big.as_mut_ptr().add(i * LINE);
            let v = std::ptr::read_volatile(p);
            std::ptr::write_volatile(p, v.wrapping_add(1));
        }
    }
    report("rmw_64mib_expect_both", STORES, &c0);

    std::hint::black_box((&big, &small));
}

/// Every event the performance database on this machine offers.
///
/// Written because a registered threshold was built on an event that cannot
/// answer it. `L1D_CACHE_MISS_ST_NONSPEC` counts retired stores that missed in
/// L1D, and the ring's stores land in lines that are already present, so the
/// quantity is zero on both threads and no rerun changes that. Choosing a
/// replacement from memory is how the first one was chosen. This prints the
/// database so the choice is made against what the hardware actually exposes,
/// and the output is committed so the choice can be audited.
fn dump_events() {
    let k = match Kperf::open() {
        Ok(k) => k,
        Err(e) => {
            println!("events_open_error=\"{e}\"");
            return;
        }
    };
    match k.events() {
        Ok(evs) => {
            println!("event_count={}", evs.len());
            for e in evs {
                println!("event name={} alias={} desc={}",
                         e.name,
                         if e.alias.is_empty() { "-" } else { &e.alias },
                         if e.description.is_empty() { "-" } else { &e.description });
            }
        }
        Err(e) => println!("events_error=\"{e}\""),
    }
}

fn run_pmu(mode: CountMode) {
    println!("-- mode={mode:?}");
    let k = match Kperf::open() {
        Ok(k) => k,
        Err(e) => {
            println!("pmu_open_error=\"{e}\"");
            return;
        }
    };
    let s = match k.session_with_mode(&[events::CYCLES, events::INSTRUCTIONS, events::L1D_LOAD_MISS, events::BRANCH_MISPREDICT], mode) {
        Ok(s) => s,
        Err(e) => {
            println!("pmu_session_error=\"{e}\"");
            return;
        }
    };
    println!("pmu_events={:?}", s.names());
    println!("pmu_add_errs={:?}", s.add_errs());
    let mut oh: Vec<Vec<u64>> = s.read_overhead(10_000);
    oh.sort_by_key(|row| row[0]);
    println!("read_overhead_cycles min={} p50={} p99={}", oh[0][0], oh[oh.len() / 2][0], oh[oh.len() * 99 / 100][0]);
    oh.sort_by_key(|row| row[1]);
    println!("read_overhead_instructions min={} p50={}", oh[0][1], oh[oh.len() / 2][1]);
    validate_chain(&s);
    syscall_test(&s);
}

/// Instructions and cycles per `getppid` system call. With kernel counting
/// each call costs hundreds of instructions or more; a filter that truly
/// excludes kernel mode leaves only the user-side trap stub (a handful).
fn syscall_test(s: &titan_platform::kperf::Session) {
    const CALLS: u64 = 100_000;
    for trial in 0..3 {
        let c0 = s.read();
        let mut acc = 0i64;
        for _ in 0..CALLS {
            acc = acc.wrapping_add(unsafe { libc::getppid() } as i64);
        }
        let c1 = s.read();
        std::hint::black_box(acc);
        let d = s.delta(&c0, &c1);
        println!(
            "syscall trial={trial} calls={CALLS} instructions_per_call={:.1} cycles_per_call={:.1}",
            d[1] as f64 / CALLS as f64,
            d[0] as f64 / CALLS as f64
        );
    }
}

/// Dependent chain of 100 adds per iteration plus `subs` and `b.ne`:
/// exactly 102 instructions per iteration, and at least 100 cycles per
/// iteration because each add needs the previous result.
#[cfg(target_arch = "aarch64")]
fn validate_chain(s: &titan_platform::kperf::Session) {
    const ITERS: u64 = 10_000_000;
    for trial in 0..5 {
        let t0 = clock::counter();
        let c0 = s.read();
        let mut a: u64 = 0;
        let mut n: u64 = ITERS;
        unsafe {
            core::arch::asm!(
                "2:",
                ".rept 100",
                "add {a}, {a}, #1",
                ".endr",
                "subs {n}, {n}, #1",
                "b.ne 2b",
                a = inout(reg) a,
                n = inout(reg) n,
                options(nostack),
            );
        }
        let c1 = s.read();
        let t1 = clock::counter();
        std::hint::black_box((a, n));
        let d = s.delta(&c0, &c1);
        let secs = (t1 - t0) as f64 / clock::counter_frequency() as f64;
        let expected_inst = ITERS * 102;
        println!(
            "chain trial={trial} cycles={} instructions={} expected_instructions={} inst_error_pct={:.4} cycles_per_add={:.4} ipc={:.3} core_ghz={:.3} l1d_miss={} br_mispred={}",
            d[0],
            d[1],
            expected_inst,
            (d[1] as f64 - expected_inst as f64) / expected_inst as f64 * 100.0,
            d[0] as f64 / (ITERS * 100) as f64,
            d[1] as f64 / d[0] as f64,
            d[0] as f64 / secs / 1e9,
            d[2],
            d[3],
        );
    }
}

#[cfg(not(target_arch = "aarch64"))]
fn validate_chain(_s: &titan_platform::kperf::Session) {
    println!("chain=skipped (aarch64 only)");
}
