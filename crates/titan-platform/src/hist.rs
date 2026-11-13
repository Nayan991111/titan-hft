//! Fixed-memory latency histogram with bounded relative error.
//!
//! Values below 2^12 are counted exactly. Above that, each power-of-two range
//! [2^k, 2^(k+1)) is split into 2^11 equal buckets, so a recorded value is
//! represented by the lower edge of a bucket no wider than 1/2048 of the value
//! (relative error below 0.05%). The layout follows the log-linear scheme of
//! HdrHistogram. Recording is a few integer operations and one increment, with
//! no allocation after construction; the largest value is also kept exactly.

const EXACT_BITS: u32 = 12;
const SUB_BITS: u32 = 11;
const MAX_BITS: u32 = 48;
const BUCKETS: usize = (1 << EXACT_BITS) + ((MAX_BITS - EXACT_BITS) as usize) * (1 << SUB_BITS);

pub struct Histogram {
    counts: Box<[u64]>,
    total: u64,
    max: u64,
    sum: u128,
}

impl Default for Histogram {
    fn default() -> Self {
        Self::new()
    }
}

impl Histogram {
    pub fn new() -> Self {
        Histogram { counts: vec![0u64; BUCKETS].into_boxed_slice(), total: 0, max: 0, sum: 0 }
    }

    #[inline(always)]
    fn index(v: u64) -> usize {
        if v < (1 << EXACT_BITS) {
            return v as usize;
        }
        let v = v.min((1u64 << MAX_BITS) - 1);
        let k = 63 - v.leading_zeros(); // floor(log2 v), >= EXACT_BITS
        let shift = k - SUB_BITS;
        let sub = (v >> shift) as usize - (1 << SUB_BITS); // in [0, 2^SUB_BITS)
        (1 << EXACT_BITS) + ((k - EXACT_BITS) as usize) * (1 << SUB_BITS) + sub
    }

    /// Lower edge of the bucket with index `i`.
    fn lower(i: usize) -> u64 {
        if i < (1 << EXACT_BITS) {
            return i as u64;
        }
        let j = i - (1 << EXACT_BITS);
        let k = EXACT_BITS + (j >> SUB_BITS) as u32;
        let sub = (j & ((1 << SUB_BITS) - 1)) as u64 + (1 << SUB_BITS);
        sub << (k - SUB_BITS)
    }

    #[inline(always)]
    pub fn record(&mut self, v: u64) {
        self.counts[Self::index(v)] += 1;
        self.total += 1;
        self.sum += v as u128;
        if v > self.max {
            self.max = v;
        }
    }

    pub fn reset(&mut self) {
        self.counts.iter_mut().for_each(|c| *c = 0);
        self.total = 0;
        self.max = 0;
        self.sum = 0;
    }

    /// Touch every page of the counters, so recording never takes a first-touch
    /// page fault. The counters come zeroed from the allocator, and plain
    /// stores of zero would be removed by the compiler; volatile writes of
    /// each counter's current value are kept and change nothing.
    pub fn prefault(&mut self) {
        const STEP: usize = 4096 / std::mem::size_of::<u64>();
        let p = self.counts.as_mut_ptr();
        let mut i = 0;
        while i < self.counts.len() {
            unsafe { std::ptr::write_volatile(p.add(i), std::ptr::read_volatile(p.add(i))) };
            i += STEP;
        }
    }

    /// Add every value recorded in `other`.
    pub fn merge(&mut self, other: &Histogram) {
        for (a, &b) in self.counts.iter_mut().zip(other.counts.iter()) {
            *a += b;
        }
        self.total += other.total;
        self.sum += other.sum;
        self.max = self.max.max(other.max);
    }

    pub fn count(&self) -> u64 {
        self.total
    }

    pub fn max(&self) -> u64 {
        self.max
    }

    pub fn mean(&self) -> f64 {
        if self.total == 0 { f64::NAN } else { self.sum as f64 / self.total as f64 }
    }

    /// Smallest recorded bucket edge `x` such that at least `q` of all values
    /// are <= the bucket containing `x` (q in [0, 1]). `q = 1` returns the
    /// exact maximum.
    pub fn quantile(&self, q: f64) -> u64 {
        if self.total == 0 {
            return 0;
        }
        if q >= 1.0 {
            return self.max;
        }
        let rank = ((q * self.total as f64).ceil() as u64).max(1);
        let mut seen = 0u64;
        for (i, &c) in self.counts.iter().enumerate() {
            seen += c;
            if seen >= rank {
                return Self::lower(i).min(self.max);
            }
        }
        self.max
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn exact_range_and_bucket_edges() {
        for v in [0u64, 1, 41, 42, 4095] {
            assert_eq!(Histogram::lower(Histogram::index(v)), v);
        }
        // Every value maps to a bucket whose lower edge is <= v and within 1/2048.
        let mut x = 4096u64;
        while x < (1u64 << 40) {
            for v in [x, x + 1, x * 3 / 2, 2 * x - 1] {
                let lo = Histogram::lower(Histogram::index(v));
                assert!(lo <= v, "lower {lo} > {v}");
                assert!((v - lo) as f64 <= v as f64 / 2048.0, "error too large at {v}");
            }
            x *= 2;
        }
    }

    #[test]
    fn prefault_keeps_counts() {
        let mut h = Histogram::new();
        for v in [3u64, 5000, 1 << 30] {
            h.record(v);
        }
        h.prefault();
        assert_eq!((h.count(), h.max(), h.quantile(0.5)), (3, 1 << 30, Histogram::lower(Histogram::index(5000))));
    }

    #[test]
    fn merge_equals_recording_into_one() {
        let (mut a, mut b, mut both) = (Histogram::new(), Histogram::new(), Histogram::new());
        for v in 0..20_000u64 {
            let x = v * v % 1_000_003;
            if v % 3 == 0 { a.record(x) } else { b.record(x) }
            both.record(x);
        }
        a.merge(&b);
        assert_eq!((a.count(), a.max(), a.mean()), (both.count(), both.max(), both.mean()));
        for q in [0.0, 0.5, 0.9, 0.99, 0.999, 1.0] {
            assert_eq!(a.quantile(q), both.quantile(q));
        }
    }

    #[test]
    fn indices_are_monotonic_and_in_range() {
        let mut prev = 0usize;
        let mut v = 0u64;
        while v < (1u64 << 47) {
            let i = Histogram::index(v);
            assert!(i >= prev && i < BUCKETS);
            prev = i;
            v = v * 17 / 16 + 1;
        }
        assert!(Histogram::index(u64::MAX) < BUCKETS);
    }

    #[test]
    fn quantiles_of_a_known_distribution() {
        let mut h = Histogram::new();
        for v in 1..=10_000u64 {
            h.record(v);
        }
        assert_eq!(h.count(), 10_000);
        assert_eq!(h.quantile(0.5), 5_000);
        let p50 = h.quantile(0.5) as f64;
        assert!((p50 - 5000.0).abs() <= 5000.0 / 2048.0 + 1.0);
        assert!((h.quantile(0.99) as f64 - 9900.0).abs() <= 9900.0 / 2048.0 + 1.0);
        assert_eq!(h.quantile(1.0), 10_000);
        assert_eq!(h.quantile(0.0001), 1);
        assert!((h.mean() - 5000.5).abs() < 1e-9);
        h.reset();
        assert_eq!(h.count(), 0);
        assert_eq!(h.quantile(0.5), 0);
    }
}
