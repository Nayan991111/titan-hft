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
    println!("region_bytes={} pages={} aligned={}", r.len(), r.pages(), r.as_ptr() as usize % r.page_size() == 0);
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
