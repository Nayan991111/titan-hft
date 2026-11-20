//! Interarrival statistics of the schedules the pipeline replays.
//!
//! Kingman's heavy traffic approximation consumes the squared coefficient of
//! variation of the interarrival times. This project has only ever measured
//! the Fano factor, which is a property of event COUNTS in a window and is a
//! different quantity: both equal one for a Poisson process, and they diverge
//! for everything else. Evaluating the approximation against our own
//! measurements therefore needs the coefficient of variation computed
//! directly from the gaps between arrivals, which is what this prints.
//!
//! Usage:
//!   titan-arrivals [messages=N] [rate=R] [beta=B] [seed=S]

use titan_itch::arrivals::{poisson, Hawkes};

const FREQ: u64 = 1_000_000_000;

fn kv(args: &[String], key: &str) -> Option<String> {
    args.iter()
        .find(|a| a.starts_with(&format!("{key}=")))
        .map(|a| a[key.len() + 1..].to_string())
}

/// Mean, standard deviation and coefficient of variation of the gaps.
fn gaps(times: &[u64]) -> (f64, f64, f64, usize) {
    if times.len() < 3 {
        return (0.0, 0.0, 0.0, 0);
    }
    let d: Vec<f64> = times.windows(2).map(|w| (w[1] - w[0]) as f64).collect();
    let n = d.len() as f64;
    let mean = d.iter().sum::<f64>() / n;
    // Sample variance, divisor n-1, which is what a coefficient of variation
    // reported from a finite sample should use.
    let var = d.iter().map(|x| (x - mean) * (x - mean)).sum::<f64>() / (n - 1.0);
    let sd = var.sqrt();
    let cv = if mean > 0.0 { sd / mean } else { 0.0 };
    (mean, sd, cv, d.len())
}

/// Fano factor of the event COUNTS, over equal windows spanning the schedule.
/// Printed beside the coefficient of variation so the two can be compared on
/// the same schedule rather than across runs.
fn fano(times: &[u64], windows: usize) -> f64 {
    if times.len() < 2 || windows < 2 {
        return 0.0;
    }
    let (lo, hi) = (times[0] as f64, times[times.len() - 1] as f64);
    let width = (hi - lo) / windows as f64;
    if width <= 0.0 {
        return 0.0;
    }
    let mut counts = vec![0.0_f64; windows];
    for &t in times {
        let mut k = ((t as f64 - lo) / width) as usize;
        if k >= windows {
            k = windows - 1;
        }
        counts[k] += 1.0;
    }
    let n = windows as f64;
    let mean = counts.iter().sum::<f64>() / n;
    if mean <= 0.0 {
        return 0.0;
    }
    let var = counts.iter().map(|c| (c - mean) * (c - mean)).sum::<f64>() / (n - 1.0);
    var / mean
}

fn periodic(n: usize, rate: f64, freq: u64) -> Vec<u64> {
    let step = freq as f64 / rate;
    (0..n).map(|i| (i as f64 * step).round() as u64).collect()
}

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let n: usize = kv(&args, "messages").and_then(|v| v.parse().ok()).unwrap_or(10_000_000);
    let rate: f64 = kv(&args, "rate").and_then(|v| v.parse().ok()).unwrap_or(5_000_000.0);
    let beta: f64 = kv(&args, "beta").and_then(|v| v.parse().ok()).unwrap_or(1e4);
    let seed: u64 = kv(&args, "seed").and_then(|v| v.parse().ok()).unwrap_or(7);

    println!(
        "arr_config messages={n} rate={rate:.0} beta={beta:.0} seed={seed} freq={FREQ}"
    );

    let mut rows: Vec<(String, Vec<u64>)> = Vec::new();
    rows.push((format!("rate_{}", rate as u64), periodic(n, rate, FREQ)));
    rows.push(("poisson".to_string(), poisson(n, rate, FREQ, seed)));
    for b in [0.0_f64, 0.3, 0.5, 0.8, 0.9] {
        rows.push((
            format!("hawkes_{b}"),
            Hawkes::with_mean_rate(rate, b, beta).schedule(n, FREQ, seed),
        ));
    }

    for (name, t) in &rows {
        let (mean, sd, cv, k) = gaps(t);
        let realised = if t.len() > 1 {
            (t.len() - 1) as f64 * FREQ as f64 / (t[t.len() - 1] - t[0]) as f64
        } else {
            0.0
        };
        let f = fano(t, 2000);
        println!(
            "arr load={name} gaps={k} mean_gap_ns={mean:.4} sd_gap_ns={sd:.4} \
             cv={cv:.6} cv2={:.6} fano={f:.4} fano_over_cv2={:.4} \
             realised_mps={:.6} offered_mps={:.6}",
            cv * cv,
            if cv > 0.0 { f / (cv * cv) } else { 0.0 },
            realised / 1e6,
            rate / 1e6
        );
    }
}
