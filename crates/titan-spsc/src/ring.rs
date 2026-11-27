//! The ring.
//!
//! Indices are free-running counters (they never wrap in practice); the slot
//! for index `i` is `i & mask`. The ring is empty when `head == tail` and full
//! when `tail - head == capacity`. The producer owns `tail`, the consumer owns
//! `head`. Slot storage lives in a pinned, pre-faulted region.

use crate::order::Order;
use crate::pad::IndexCell;
use crate::wait::Wait;
use crate::{Rx, Tx};
use std::marker::PhantomData;
use std::sync::atomic::Ordering;
use std::sync::Arc;
use titan_platform::mem::PinnedRegion;

/// Both index cells in one 128-byte-aligned block, so their placement
/// relative to Apple silicon's 128-byte lines is fixed rather than left to the
/// allocator: with `Pad64` head and tail always share one line, with `Pad128`
/// they never do.
#[repr(C, align(128))]
struct Indices<P: IndexCell> {
    /// Consumer cell: `head` plus the consumer's cache of `tail`.
    head: P,
    /// Producer cell: `tail` plus the producer's cache of `head`.
    tail: P,
}

#[repr(C)]
struct Shared<T, P: IndexCell> {
    ix: Indices<P>,
    slots: *mut T,
    mask: usize,
    capacity: usize,
    _region: PinnedRegion,
}

unsafe impl<T: Send, P: IndexCell> Send for Shared<T, P> {}
unsafe impl<T: Send, P: IndexCell> Sync for Shared<T, P> {}

pub struct Producer<T, O: Order, P: IndexCell, const CACHED: bool> {
    s: Arc<Shared<T, P>>,
    _o: PhantomData<O>,
}

pub struct Consumer<T, O: Order, P: IndexCell, const CACHED: bool> {
    s: Arc<Shared<T, P>>,
    _o: PhantomData<O>,
}

/// Create a ring of `capacity` slots (a power of two).
pub fn ring<T: Copy + Send, O: Order, P: IndexCell, const CACHED: bool>(
    capacity: usize,
) -> (Producer<T, O, P, CACHED>, Consumer<T, O, P, CACHED>) {
    assert!(capacity.is_power_of_two(), "capacity must be a power of two");
    let bytes = capacity * std::mem::size_of::<T>().max(1);
    let region = PinnedRegion::new(bytes).expect("ring storage");
    assert!(std::mem::align_of::<T>() <= region.page_size());
    let s = Arc::new(Shared {
        ix: Indices { head: P::default(), tail: P::default() },
        slots: region.as_ptr() as *mut T,
        mask: capacity - 1,
        capacity,
        _region: region,
    });
    (Producer { s: s.clone(), _o: PhantomData }, Consumer { s, _o: PhantomData })
}

impl<T: Copy + Send, O: Order, P: IndexCell, const CACHED: bool> Producer<T, O, P, CACHED> {
    /// Append `v`; false if the ring is full.
    #[inline(always)]
    pub fn push(&mut self, v: T) -> bool {
        let s = &*self.s;
        let tail = s.ix.tail.index().load(Ordering::Relaxed);
        if CACHED {
            let cached = unsafe { *s.ix.tail.cache() };
            if tail.wrapping_sub(cached) == s.capacity {
                let head = O::load(s.ix.head.index());
                unsafe { *s.ix.tail.cache() = head };
                if tail.wrapping_sub(head) == s.capacity {
                    return false;
                }
            }
        } else if tail.wrapping_sub(O::load(s.ix.head.index())) == s.capacity {
            return false;
        }
        unsafe { s.slots.add(tail & s.mask).write(v) };
        O::store(s.ix.tail.index(), tail.wrapping_add(1));
        true
    }

    pub fn capacity(&self) -> usize {
        self.s.capacity
    }

    /// Placement of the index cells: (head address, tail address).
    pub fn index_addresses(&self) -> (usize, usize) {
        (self.s.ix.head.index() as *const _ as usize, self.s.ix.tail.index() as *const _ as usize)
    }
}

/// Whether two addresses fall in the same 128-byte line.
pub fn same_line_128(a: usize, b: usize) -> bool {
    a / 128 == b / 128
}

impl<T: Copy + Send, O: Order, P: IndexCell, const CACHED: bool> Consumer<T, O, P, CACHED> {
    /// Remove the oldest element, if any.
    #[inline(always)]
    pub fn pop(&mut self) -> Option<T> {
        let s = &*self.s;
        let head = s.ix.head.index().load(Ordering::Relaxed);
        if CACHED {
            if head == unsafe { *s.ix.head.cache() } {
                let tail = O::load(s.ix.tail.index());
                unsafe { *s.ix.head.cache() = tail };
                if head == tail {
                    return None;
                }
            }
        } else if head == O::load(s.ix.tail.index()) {
            return None;
        }
        let v = unsafe { s.slots.add(head & s.mask).read() };
        O::store(s.ix.head.index(), head.wrapping_add(1));
        Some(v)
    }
}

/// A ring half combined with a wait strategy, as a blocking channel end.
pub struct WaitingProducer<O: Order, P: IndexCell, W: Wait, const CACHED: bool> {
    /// The underlying producer; exposed for layout inspection.
    pub inner: Producer<u64, O, P, CACHED>,
    _w: PhantomData<W>,
}

pub struct WaitingConsumer<O: Order, P: IndexCell, W: Wait, const CACHED: bool> {
    pub inner: Consumer<u64, O, P, CACHED>,
    _w: PhantomData<W>,
}

pub fn channel<O: Order, P: IndexCell, W: Wait, const CACHED: bool>(
    capacity: usize,
) -> (WaitingProducer<O, P, W, CACHED>, WaitingConsumer<O, P, W, CACHED>) {
    let (p, c) = ring::<u64, O, P, CACHED>(capacity);
    (WaitingProducer { inner: p, _w: PhantomData }, WaitingConsumer { inner: c, _w: PhantomData })
}

impl<O: Order, P: IndexCell, W: Wait, const CACHED: bool> Tx for WaitingProducer<O, P, W, CACHED> {
    #[inline(always)]
    fn send(&mut self, v: u64) {
        while !self.inner.push(v) {
            let s = &*self.inner.s;
            W::wait(s.ix.head.index(), s.ix.head.index().load(Ordering::Relaxed));
        }
    }
}

impl<O: Order, P: IndexCell, W: Wait, const CACHED: bool> Rx for WaitingConsumer<O, P, W, CACHED> {
    #[inline(always)]
    fn recv(&mut self) -> u64 {
        loop {
            if let Some(v) = self.inner.pop() {
                return v;
            }
            let s = &*self.inner.s;
            W::wait(s.ix.tail.index(), s.ix.head.index().load(Ordering::Relaxed));
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::order::AcqRel;
    use crate::pad::Pad128;

    #[test]
    fn fill_and_drain_single_thread() {
        let (mut p, mut c) = ring::<u64, AcqRel, Pad128, true>(8);
        for i in 0..8 {
            assert!(p.push(i));
        }
        assert!(!p.push(99), "ring must report full at capacity");
        for i in 0..8 {
            assert_eq!(c.pop(), Some(i));
        }
        assert_eq!(c.pop(), None);
    }

    #[test]
    fn index_placement_is_controlled() {
        use crate::pad::Pad64;
        for _ in 0..64 {
            let (p64, _c) = ring::<u64, AcqRel, Pad64, true>(8);
            let (h, t) = p64.index_addresses();
            assert_eq!(h % 128, 0);
            assert!(same_line_128(h, t), "Pad64 cells must share a 128-byte line");
            let (p128, _c) = ring::<u64, AcqRel, Pad128, true>(8);
            let (h, t) = p128.index_addresses();
            assert_eq!(h % 128, 0);
            assert!(!same_line_128(h, t), "Pad128 cells must not share a line");
        }
    }

    #[test]
    fn wraps_many_times() {
        let (mut p, mut c) = ring::<u64, AcqRel, Pad128, false>(4);
        for i in 0..10_000u64 {
            assert!(p.push(i));
            assert_eq!(c.pop(), Some(i));
        }
    }
}

/// A ring slot carrying a `u64` payload in a chosen footprint.
///
/// H84 found the acquire-release arm breaking by 2.716 in cost between ring
/// capacities 8 and 16 while the fenced-store arm did not move. In that design
/// the slot was eight bytes and the region page aligned, so capacity alone set
/// the slot array's footprint, the number of slots and how far the producer
/// could run ahead. Three candidate causes moved together and the experiment
/// could not name one. Widening the slot separates the footprint from the other
/// two: at a fixed capacity a 128 byte slot occupies sixteen times the bytes of
/// an eight byte slot while the slot count and the run-ahead are unchanged.
///
/// The payload is what the checksum reads. The padding exists only to move the
/// footprint and is never read.
pub trait Slot: Copy + Send + Default {
    /// Bytes this slot occupies, used to report the manipulation in the record
    /// rather than inferring it from the flag that requested it.
    const BYTES: usize;
    fn pack(v: u64) -> Self;
    fn unpack(self) -> u64;
}

impl Slot for u64 {
    const BYTES: usize = 8;
    #[inline(always)]
    fn pack(v: u64) -> Self {
        v
    }
    #[inline(always)]
    fn unpack(self) -> u64 {
        self
    }
}

/// A slot padded to a full 128 byte coherence granule on this part.
#[repr(C, align(128))]
#[derive(Clone, Copy)]
pub struct Slot128 {
    v: u64,
    _pad: [u64; 15],
}

impl Default for Slot128 {
    #[inline(always)]
    fn default() -> Self {
        Self { v: 0, _pad: [0; 15] }
    }
}

impl Slot for Slot128 {
    const BYTES: usize = 128;
    #[inline(always)]
    fn pack(v: u64) -> Self {
        Self { v, _pad: [0; 15] }
    }
    #[inline(always)]
    fn unpack(self) -> u64 {
        self.v
    }
}

/// Producer over a ring whose slot type is chosen at the call site.
///
/// Kept separate from `WaitingProducer` rather than adding a parameter to it,
/// so that every existing call site compiles unchanged. The two differ only in
/// the slot type and the pack and unpack at the boundary.
pub struct SlotProducer<S: Slot, O: Order, P: IndexCell, W: Wait, const CACHED: bool> {
    pub inner: Producer<S, O, P, CACHED>,
    _w: PhantomData<W>,
}

/// Consumer counterpart to [`SlotProducer`].
pub struct SlotConsumer<S: Slot, O: Order, P: IndexCell, W: Wait, const CACHED: bool> {
    pub inner: Consumer<S, O, P, CACHED>,
    _w: PhantomData<W>,
}

/// A ring of `capacity` slots of type `S`.
pub fn channel_slot<S: Slot, O: Order, P: IndexCell, W: Wait, const CACHED: bool>(
    capacity: usize,
) -> (SlotProducer<S, O, P, W, CACHED>, SlotConsumer<S, O, P, W, CACHED>) {
    let (p, c) = ring::<S, O, P, CACHED>(capacity);
    (SlotProducer { inner: p, _w: PhantomData }, SlotConsumer { inner: c, _w: PhantomData })
}

impl<S: Slot, O: Order, P: IndexCell, W: Wait, const CACHED: bool> Tx
    for SlotProducer<S, O, P, W, CACHED>
{
    #[inline(always)]
    fn send(&mut self, v: u64) {
        let s = S::pack(v);
        while !self.inner.push(s) {
            let sh = &*self.inner.s;
            W::wait(sh.ix.head.index(), sh.ix.head.index().load(Ordering::Relaxed));
        }
    }
}

impl<S: Slot, O: Order, P: IndexCell, W: Wait, const CACHED: bool> Rx
    for SlotConsumer<S, O, P, W, CACHED>
{
    #[inline(always)]
    fn recv(&mut self) -> u64 {
        loop {
            if let Some(v) = self.inner.pop() {
                return v.unpack();
            }
            let sh = &*self.inner.s;
            W::wait(sh.ix.tail.index(), sh.ix.head.index().load(Ordering::Relaxed));
        }
    }
}
