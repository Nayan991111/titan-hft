//! Barrier-placement policies, for decomposing the Fenced inversion.
//!
//! [`crate::order::Fenced`] replaces both halves of an acquire-release pair at
//! once: the RCpc acquire load becomes a relaxed load followed by `dmb ish`,
//! and the release store becomes `dmb ish` followed by a relaxed store. In the
//! uncached streaming configuration that substitution is measured to cost
//! 0.4475 of AcqRel in cycles per message, and in the cached configuration it
//! costs 2.449 of it. Both figures are reproduced, and neither is explained.
//!
//! Because `Fenced` changes the load side and the store side together, the
//! measurement cannot say which side carries the effect. These policies cross
//! the two halves so it can:
//!
//! | policy | load | store |
//! |---|---|---|
//! | [`crate::order::AcqRel`] | `ldapr` | `stlr` |
//! | [`crate::order::Fenced`] | `ldr` + `dmb ish` | `dmb ish` + `str` |
//! | [`FencedLoad`] | `ldr` + `dmb ish` | `stlr` |
//! | [`FencedStore`] | `ldapr` | `dmb ish` + `str` |
//! | [`Unordered`] | `ldr` | `str` |
//!
//! The four ordered policies are each correct for an SPSC ring: every load of
//! the other side's index carries acquire semantics and every publishing store
//! carries release semantics, whether the semantics come from the instruction
//! or from an adjacent barrier. `FencedLoad` and `FencedStore` are therefore
//! substitutions a programmer could legitimately make, not synthetic arms.
//!
//! The lowering in that table is a claim about the compiler, so it is checked
//! rather than assumed: `crate::census` exposes an unmangled, non-inlined entry
//! point per policy and `scripts/census.sh` counts the mnemonics each one
//! actually emitted.

use crate::order::Order;
use std::sync::atomic::{fence, AtomicUsize, Ordering};

/// Barrier on the load side only: `ldr` + `dmb ish` to read, `stlr` to publish.
pub struct FencedLoad;

/// Barrier on the store side only: `ldapr` to read, `dmb ish` + `str` to publish.
pub struct FencedStore;

/// No ordering on either side.
///
/// **This is not a correct SPSC ring and must never be presented as one.** The
/// producer writes a slot and then publishes the index; with relaxed accesses
/// on both sides nothing stops the consumer from observing the published index
/// before the slot write that it guards, and reading a stale slot. It exists
/// only as a cost floor, so the other four policies can be reported as a
/// fraction of the total ordering cost in the loop rather than only against
/// each other.
///
/// The harness that runs this policy reports a correctness flag per arm
/// instead of asserting, so that a reordering observed here records itself as
/// data and does not abort a sweep that the other arms are halfway through.
pub struct Unordered;

impl Order for FencedLoad {
    const NAME: &'static str = "fencedload";
    #[inline(always)]
    fn load(a: &AtomicUsize) -> usize {
        let v = a.load(Ordering::Relaxed);
        fence(Ordering::SeqCst);
        v
    }
    #[inline(always)]
    fn store(a: &AtomicUsize, v: usize) {
        a.store(v, Ordering::Release)
    }
}

impl Order for FencedStore {
    const NAME: &'static str = "fencedstore";
    #[inline(always)]
    fn load(a: &AtomicUsize) -> usize {
        a.load(Ordering::Acquire)
    }
    #[inline(always)]
    fn store(a: &AtomicUsize, v: usize) {
        fence(Ordering::SeqCst);
        a.store(v, Ordering::Relaxed)
    }
}

impl Order for Unordered {
    const NAME: &'static str = "unordered";
    #[inline(always)]
    fn load(a: &AtomicUsize) -> usize {
        a.load(Ordering::Relaxed)
    }
    #[inline(always)]
    fn store(a: &AtomicUsize, v: usize) {
        a.store(v, Ordering::Relaxed)
    }
}
