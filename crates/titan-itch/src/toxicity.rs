//! Order-flow toxicity: VPIN in volume time, and trade classification
//! accuracy against exact signs.
//!
//! ITCH executions (`E`, `C`) name the resting order, whose side is in the
//! L3 book, so the aggressor side of every displayed execution is known
//! exactly: the opposite of the resting side. VPIN (Easley, Lopez de Prado
//! and O'Hara, Review of Financial Studies 25(5), 2012) is computed here
//! from those exact signs; bulk volume classification and the tick rule are
//! evaluated against them.

/// One signed execution.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Exec {
    pub locate: u16,
    /// Aggressor bought (the resting order was a sell).
    pub buy: bool,
    pub shares: u32,
    /// 4 implied decimals.
    pub price: u32,
    /// Nanoseconds since midnight.
    pub ts: u64,
}

/// Buckets in the VPIN window.
pub const VPIN_BUCKETS: usize = 50;

#[derive(Clone)]
struct State {
    /// Volume per bucket; 0 disables the instrument.
    size: u64,
    buy: u64,
    sell: u64,
    imb: [u64; VPIN_BUCKETS],
    pos: usize,
    full: usize,
    sum: u64,
}

/// Per-instrument VPIN over the last [`VPIN_BUCKETS`] full buckets.
pub struct Vpin {
    s: Vec<State>,
}

impl Vpin {
    /// `bucket[locate]` is the bucket volume of that instrument (0: ignore it).
    pub fn new(bucket: &[u64]) -> Self {
        let s = bucket
            .iter()
            .map(|&size| State { size, buy: 0, sell: 0, imb: [0; VPIN_BUCKETS], pos: 0, full: 0, sum: 0 })
            .collect();
        Vpin { s }
    }

    pub fn reset(&mut self) {
        for x in &mut self.s {
            x.buy = 0;
            x.sell = 0;
            x.imb = [0; VPIN_BUCKETS];
            x.pos = 0;
            x.full = 0;
            x.sum = 0;
        }
    }

    /// Add one signed execution; it may close several buckets.
    #[inline(always)]
    pub fn update(&mut self, locate: u16, buy: bool, shares: u32) {
        let Some(x) = self.s.get_mut(locate as usize) else { return };
        if x.size == 0 {
            return;
        }
        let mut left = shares as u64;
        while left > 0 {
            let room = x.size - x.buy - x.sell;
            let take = room.min(left);
            if buy {
                x.buy += take;
            } else {
                x.sell += take;
            }
            left -= take;
            if x.buy + x.sell == x.size {
                let d = x.buy.abs_diff(x.sell);
                x.sum = x.sum - x.imb[x.pos] + d;
                x.imb[x.pos] = d;
                x.pos = if x.pos + 1 == VPIN_BUCKETS { 0 } else { x.pos + 1 };
                x.full = (x.full + 1).min(VPIN_BUCKETS);
                x.buy = 0;
                x.sell = 0;
            }
        }
    }

    /// VPIN of the instrument once its window is full.
    pub fn value(&self, locate: u16) -> Option<f64> {
        let x = self.s.get(locate as usize)?;
        (x.size > 0 && x.full == VPIN_BUCKETS).then(|| x.sum as f64 / (VPIN_BUCKETS as u64 * x.size) as f64)
    }
}

/// Standard normal CDF; erfc from Abramowitz and Stegun 7.1.26 (absolute
/// error below 1.5e-7).
pub fn normal_cdf(z: f64) -> f64 {
    let x = z.abs() / std::f64::consts::SQRT_2;
    let t = 1.0 / (1.0 + 0.327_591_1 * x);
    let poly = t * (0.254_829_592 + t * (-0.284_496_736 + t * (1.421_413_741 + t * (-1.453_152_027 + t * 1.061_405_429))));
    let erfc = poly * (-x * x).exp();
    if z >= 0.0 {
        1.0 - 0.5 * erfc
    } else {
        0.5 * erfc
    }
}

/// Result of [`classification_accuracy`]; volumes in shares.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Accuracy {
    pub instruments: usize,
    /// Signed volume compared (each instrument's first bar excluded).
    pub volume: f64,
    /// Sum over bars of |estimated - true buy volume|, divided by volume.
    pub bvc_bar_error: f64,
    pub tick_bar_error: f64,
    /// Wrongly signed volume over volume, trade by trade (tick rule).
    pub tick_trade_error: f64,
    /// Executions with no prior non-zero tick (counted half buy).
    pub tick_unresolved: u64,
    /// Instruments where BVC's bar error is the larger one.
    pub bvc_worse: usize,
}

/// BVC and the tick rule against exact signs, per `bar_ns` bar, for
/// instruments with at least `min_execs` executions. `execs` must be in
/// feed order.
pub fn classification_accuracy(execs: &[Exec], bar_ns: u64, min_execs: usize) -> Accuracy {
    use std::collections::BTreeMap;
    let mut by: BTreeMap<u16, Vec<Exec>> = BTreeMap::new();
    for e in execs {
        by.entry(e.locate).or_default().push(*e);
    }
    let mut a = Accuracy::default();
    let (mut bvc_err, mut tick_err, mut tick_trade_err) = (0.0, 0.0, 0.0);
    for v in by.values().filter(|v| v.len() >= min_execs) {
        // Tick rule, trade by trade: buy weight of each execution (1, 0 or 0.5).
        let mut w = Vec::with_capacity(v.len());
        let (mut last_price, mut last_sign): (Option<u32>, Option<bool>) = (None, None);
        for e in v {
            if let Some(p) = last_price {
                if e.price > p {
                    last_sign = Some(true);
                } else if e.price < p {
                    last_sign = Some(false);
                }
            }
            w.push(match last_sign {
                Some(true) => 1.0,
                Some(false) => 0.0,
                None => 0.5,
            });
            last_price = Some(e.price);
        }
        // Bars: (bar id, volume, true buy volume, tick buy volume, last price, unresolved).
        let mut bars: Vec<(u64, f64, f64, f64, u32, u64, f64)> = Vec::new();
        for (e, &wt) in v.iter().zip(&w) {
            let id = e.ts / bar_ns;
            if bars.last().map(|b| b.0) != Some(id) {
                bars.push((id, 0.0, 0.0, 0.0, 0, 0, 0.0));
            }
            let b = bars.last_mut().unwrap();
            let q = e.shares as f64;
            b.1 += q;
            if e.buy {
                b.2 += q;
            }
            b.3 += wt * q;
            b.4 = e.price;
            if wt == 0.5 {
                b.5 += 1;
            }
            // Trade-level tick error: wrong sign counts fully, unresolved half.
            b.6 += if wt == 0.5 { 0.5 * q } else if (wt == 1.0) != e.buy { q } else { 0.0 };
        }
        if bars.len() < 2 {
            continue;
        }
        // BVC: price change from the previous non-empty bar.
        let dp: Vec<f64> = bars.windows(2).map(|p| p[1].4 as f64 - p[0].4 as f64).collect();
        let mean = dp.iter().sum::<f64>() / dp.len() as f64;
        let sd = (dp.iter().map(|d| (d - mean) * (d - mean)).sum::<f64>() / dp.len() as f64).sqrt();
        let (mut ib, mut it, mut vol) = (0.0, 0.0, 0.0);
        for (b, d) in bars[1..].iter().zip(&dp) {
            let frac = if sd > 0.0 { normal_cdf(d / sd) } else { 0.5 };
            ib += (frac * b.1 - b.2).abs();
            it += (b.3 - b.2).abs();
            tick_trade_err += b.6;
            a.tick_unresolved += b.5;
            vol += b.1;
        }
        a.instruments += 1;
        a.volume += vol;
        bvc_err += ib;
        tick_err += it;
        if ib > it {
            a.bvc_worse += 1;
        }
    }
    if a.volume > 0.0 {
        a.bvc_bar_error = bvc_err / a.volume;
        a.tick_bar_error = tick_err / a.volume;
        a.tick_trade_error = tick_trade_err / a.volume;
    }
    a
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn vpin_buckets_and_window() {
        let mut v = Vpin::new(&[0, 100]);
        // Instrument 0 is disabled.
        v.update(0, true, 1_000);
        assert_eq!(v.value(0), None);
        // 49 balanced buckets, then one all-buy bucket filled by a single
        // execution that also spills into the next bucket.
        for _ in 0..49 {
            v.update(1, true, 50);
            v.update(1, false, 50);
        }
        assert_eq!(v.value(1), None);
        v.update(1, true, 130);
        // Window: 49 buckets with imbalance 0 and one with 100.
        let got = v.value(1).unwrap();
        assert!((got - 100.0 / 5_000.0).abs() < 1e-12, "{got}");
        // The 30 spilled shares sit in the open bucket.
        v.update(1, false, 70);
        // That bucket closes with |30 - 70| = 40 and evicts the oldest (0).
        let got = v.value(1).unwrap();
        assert!((got - 140.0 / 5_000.0).abs() < 1e-12, "{got}");
        v.reset();
        assert_eq!(v.value(1), None);
    }

    #[test]
    fn normal_cdf_values() {
        for (z, want) in [(0.0, 0.5), (1.0, 0.841_344_746), (-1.96, 0.024_997_895), (3.0, 0.998_650_102)] {
            assert!((normal_cdf(z) - want).abs() < 2e-7, "{z}");
        }
    }

    #[test]
    fn accuracy_on_known_flow() {
        // Prices step up with buys and down with sells: the tick rule is
        // exact after the first trade.
        let mut e = Vec::new();
        let mut p = 1_000_000u32;
        for i in 0..400u64 {
            let buy = (i / 7) % 2 == 0;
            p = if buy { p + 100 } else { p - 100 };
            e.push(Exec { locate: 3, buy, shares: 100, price: p, ts: i * 1_000_000_000 });
        }
        let a = classification_accuracy(&e, 60_000_000_000, 100);
        assert_eq!(a.instruments, 1);
        assert!(a.tick_bar_error < 1e-12 && a.tick_trade_error < 1e-12, "{a:?}");
        assert!(a.bvc_bar_error > 0.0);
        // Too few executions: nothing compared.
        assert_eq!(classification_accuracy(&e[..50], 60_000_000_000, 100).instruments, 0);
    }
}
