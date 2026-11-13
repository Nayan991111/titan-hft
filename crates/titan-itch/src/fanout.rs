//! Sharing the top of book of one L3 book with several strategy threads.
//!
//! One writer (the thread that owns the book) publishes the best bid and ask
//! of an instrument whenever they change. Two ways of handing that to N
//! readers are provided:
//!
//! * a private copy per reader: every update is pushed into one ring per
//!   reader as a 32-byte [`Quote`] (lossless and ordered, writer work grows
//!   with N);
//! * one shared snapshot per instrument: a seqlock [`Slot`] that the writer
//!   overwrites and readers poll (writer work independent of N, but a reader
//!   sees only the latest version: intermediate versions can be skipped).
//!
//! The seqlock follows the fence-based form in H.-J. Boehm, "Can seqlocks get
//! along with programming language memory models?", MSPC 2012, pp. 12-20,
//! doi:10.1145/2247684.2247688: the protected fields are atomics accessed
//! with relaxed ordering, so a reader racing with the writer reads stale or
//! mixed values but never has undefined behaviour, and the sequence check
//! discards them.

use crate::Top;
use std::sync::atomic::{fence, AtomicU64, Ordering};

/// Best price and size of one side in one word: price in the high 32 bits,
/// size saturated to 32 bits in the low 32; 0 for an empty side (price 0 is
/// not a valid ITCH price, so 0 is unambiguous).
#[inline(always)]
pub fn pack(t: Top) -> u64 {
    match t {
        Some((p, s)) => ((p as u64) << 32) | s.min(u32::MAX as u64),
        None => 0,
    }
}

/// One top-of-book update as carried by a per-reader ring.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
#[repr(C)]
pub struct Quote {
    /// Generic counter value when the writer published the update.
    pub stamp: u64,
    pub bid: u64,
    pub ask: u64,
    /// Version of this instrument's top of book: 1 for its first update, so
    /// version 0 is free to mark the end of a stream.
    pub ver: u32,
    /// Index of the instrument among the subscribed ones.
    pub slot: u16,
    pub locate: u16,
}

/// A consistent read of a [`Slot`].
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Snapshot {
    /// Sequence number: twice the number of completed writes.
    pub seq: u64,
    pub bid: u64,
    pub ask: u64,
    pub stamp: u64,
}

/// Seqlock-protected top of book of one instrument, alone in a 128-byte
/// line (the line size of Apple silicon), so slots of different instruments
/// never share a line.
#[repr(C, align(128))]
#[derive(Default)]
pub struct Slot {
    seq: AtomicU64,
    bid: AtomicU64,
    ask: AtomicU64,
    stamp: AtomicU64,
}

impl Slot {
    /// Publish a new version. Single writer only.
    #[inline(always)]
    pub fn write(&self, bid: u64, ask: u64, stamp: u64) {
        let s = self.seq.load(Ordering::Relaxed);
        self.seq.store(s.wrapping_add(1), Ordering::Relaxed);
        // Orders the odd sequence number before the data stores.
        fence(Ordering::Release);
        self.bid.store(bid, Ordering::Relaxed);
        self.ask.store(ask, Ordering::Relaxed);
        self.stamp.store(stamp, Ordering::Relaxed);
        self.seq.store(s.wrapping_add(2), Ordering::Release);
    }

    /// Current sequence number, as a cheap change check before [`Slot::read`].
    #[inline(always)]
    pub fn seq(&self) -> u64 {
        self.seq.load(Ordering::Relaxed)
    }

    /// A consistent snapshot, or `None` if a write was in progress or
    /// completed during the read (the caller retries later).
    #[inline(always)]
    pub fn read(&self) -> Option<Snapshot> {
        let s0 = self.seq.load(Ordering::Acquire);
        if s0 & 1 != 0 {
            return None;
        }
        let bid = self.bid.load(Ordering::Relaxed);
        let ask = self.ask.load(Ordering::Relaxed);
        let stamp = self.stamp.load(Ordering::Relaxed);
        // Orders the data loads before the second sequence load.
        fence(Ordering::Acquire);
        let s1 = self.seq.load(Ordering::Relaxed);
        if s0 != s1 {
            return None;
        }
        Some(Snapshot { seq: s0, bid, ask, stamp })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::AtomicBool;

    #[test]
    fn quote_is_32_bytes_and_slot_one_line() {
        assert_eq!(std::mem::size_of::<Quote>(), 32);
        assert_eq!(std::mem::size_of::<Slot>(), 128);
        assert_eq!(std::mem::align_of::<Slot>(), 128);
    }

    #[test]
    fn pack_keeps_price_and_saturates_size() {
        assert_eq!(pack(None), 0);
        assert_eq!(pack(Some((1_000_000, 300))), (1_000_000u64 << 32) | 300);
        assert_eq!(pack(Some((1, u64::MAX))), (1u64 << 32) | u32::MAX as u64);
    }

    #[test]
    fn sequential_write_then_read() {
        let s = Slot::default();
        assert_eq!(s.read(), Some(Snapshot { seq: 0, bid: 0, ask: 0, stamp: 0 }));
        s.write(5, 6, 7);
        s.write(8, 9, 10);
        assert_eq!(s.read(), Some(Snapshot { seq: 4, bid: 8, ask: 9, stamp: 10 }));
    }

    /// Readers racing a writer never see a torn snapshot: every write stores
    /// three fields derived from one value, and every successful read must
    /// show three fields from the same write, with a sequence number that
    /// matches the write count and never goes backwards.
    #[test]
    fn concurrent_reads_are_never_torn() {
        const WRITES: u64 = 200_000;
        let slot = Slot::default();
        let done = AtomicBool::new(false);
        std::thread::scope(|s| {
            for _ in 0..3 {
                s.spawn(|| {
                    let (mut last, mut ok) = (0u64, 0u64);
                    loop {
                        let fin = done.load(Ordering::Acquire);
                        if let Some(r) = slot.read() {
                            let v = r.seq / 2;
                            let want = if v == 0 { (0, 0, 0) } else { (v, v.wrapping_mul(3), v ^ 0x5555) };
                            assert_eq!((r.bid, r.ask, r.stamp), want, "torn read");
                            assert!(r.seq >= last, "sequence went backwards");
                            last = r.seq;
                            ok += 1;
                        }
                        if fin {
                            break;
                        }
                    }
                    assert_eq!(last, 2 * WRITES, "final version not seen");
                    assert!(ok > 0);
                });
            }
            for v in 1..=WRITES {
                slot.write(v, v.wrapping_mul(3), v ^ 0x5555);
            }
            done.store(true, Ordering::Release);
        });
    }
}
