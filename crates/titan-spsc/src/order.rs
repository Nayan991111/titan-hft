//! Index access policies.
//!
//! On arm64 targets with FEAT_LRCPC (all Apple silicon), LLVM lowers:
//!
//! | policy | load | store |
//! |---|---|---|
//! | [`AcqRel`] | `ldapr` (RCpc acquire) | `stlr` |
//! | [`SeqCst`] | `ldar` (RCsc acquire) | `stlr` |
//! | [`Fenced`] | `ldr` + `dmb ish` | `dmb ish` + `str` |
//!
//! All three are correct for an SPSC ring: each load of the other side's
//! index has acquire semantics and each publishing store has release
//! semantics. They differ only in how much ordering the hardware must enforce.

use std::sync::atomic::{fence, AtomicUsize, Ordering};

pub trait Order: Send + Sync + 'static {
    const NAME: &'static str;
    /// Load the other side's index with (at least) acquire semantics.
    fn load(a: &AtomicUsize) -> usize;
    /// Publish this side's index with (at least) release semantics.
    fn store(a: &AtomicUsize, v: usize);
}

pub struct AcqRel;
pub struct SeqCst;
pub struct Fenced;

impl Order for AcqRel {
    const NAME: &'static str = "acqrel";
    #[inline(always)]
    fn load(a: &AtomicUsize) -> usize {
        a.load(Ordering::Acquire)
    }
    #[inline(always)]
    fn store(a: &AtomicUsize, v: usize) {
        a.store(v, Ordering::Release)
    }
}

impl Order for SeqCst {
    const NAME: &'static str = "seqcst";
    #[inline(always)]
    fn load(a: &AtomicUsize) -> usize {
        a.load(Ordering::SeqCst)
    }
    #[inline(always)]
    fn store(a: &AtomicUsize, v: usize) {
        a.store(v, Ordering::SeqCst)
    }
}

impl Order for Fenced {
    const NAME: &'static str = "fenced";
    #[inline(always)]
    fn load(a: &AtomicUsize) -> usize {
        let v = a.load(Ordering::Relaxed);
        fence(Ordering::SeqCst);
        v
    }
    #[inline(always)]
    fn store(a: &AtomicUsize, v: usize) {
        fence(Ordering::SeqCst);
        a.store(v, Ordering::Relaxed)
    }
}
