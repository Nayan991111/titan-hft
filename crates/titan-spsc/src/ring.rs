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

#[repr(C)]
struct Shared<T, P: IndexCell> {
    /// Consumer line: `head` plus the consumer's cache of `tail`.
    head: P,
    /// Producer line: `tail` plus the producer's cache of `head`.
    tail: P,
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
        head: P::default(),
        tail: P::default(),
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
        let tail = s.tail.index().load(Ordering::Relaxed);
        if CACHED {
            let cached = unsafe { *s.tail.cache() };
            if tail.wrapping_sub(cached) == s.capacity {
                let head = O::load(s.head.index());
                unsafe { *s.tail.cache() = head };
                if tail.wrapping_sub(head) == s.capacity {
                    return false;
                }
            }
        } else if tail.wrapping_sub(O::load(s.head.index())) == s.capacity {
            return false;
        }
        unsafe { s.slots.add(tail & s.mask).write(v) };
        O::store(s.tail.index(), tail.wrapping_add(1));
        true
    }

    pub fn capacity(&self) -> usize {
        self.s.capacity
    }
}

impl<T: Copy + Send, O: Order, P: IndexCell, const CACHED: bool> Consumer<T, O, P, CACHED> {
    /// Remove the oldest element, if any.
    #[inline(always)]
    pub fn pop(&mut self) -> Option<T> {
        let s = &*self.s;
        let head = s.head.index().load(Ordering::Relaxed);
        if CACHED {
            if head == unsafe { *s.head.cache() } {
                let tail = O::load(s.tail.index());
                unsafe { *s.head.cache() = tail };
                if head == tail {
                    return None;
                }
            }
        } else if head == O::load(s.tail.index()) {
            return None;
        }
        let v = unsafe { s.slots.add(head & s.mask).read() };
        O::store(s.head.index(), head.wrapping_add(1));
        Some(v)
    }
}

/// A ring half combined with a wait strategy, as a blocking channel end.
pub struct WaitingProducer<O: Order, P: IndexCell, W: Wait, const CACHED: bool> {
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
            W::wait(s.head.index(), s.head.index().load(Ordering::Relaxed));
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
            W::wait(s.tail.index(), s.head.index().load(Ordering::Relaxed));
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
    fn wraps_many_times() {
        let (mut p, mut c) = ring::<u64, AcqRel, Pad128, false>(4);
        for i in 0..10_000u64 {
            assert!(p.push(i));
            assert_eq!(c.pop(), Some(i));
        }
    }
}
