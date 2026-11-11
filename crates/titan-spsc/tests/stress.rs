//! Cross-thread correctness: every message arrives exactly once, in order,
//! for every ordering policy, padding, caching mode and wait strategy.

use titan_spsc::order::{AcqRel, Fenced, Order, SeqCst};
use titan_spsc::pad::{IndexCell, Pad128, Pad64};
use titan_spsc::ring::channel;
use titan_spsc::wait::{Spin, SpinHint, Wait, Wfe, Yield};
use titan_spsc::{Rx, Tx};

const N: u64 = 2_000_000;

fn run<O: Order, P: IndexCell, W: Wait, const C: bool>() {
    // Small capacity forces frequent full and empty transitions.
    let (mut tx, mut rx) = channel::<O, P, W, C>(64);
    let t = std::thread::spawn(move || {
        for i in 0..N {
            tx.send(i.wrapping_mul(0x9E37_79B9_7F4A_7C15));
        }
    });
    for i in 0..N {
        assert_eq!(rx.recv(), i.wrapping_mul(0x9E37_79B9_7F4A_7C15), "message {i} lost or reordered");
    }
    t.join().unwrap();
}

#[test]
fn acqrel_variants() {
    run::<AcqRel, Pad128, SpinHint, true>();
    run::<AcqRel, Pad128, SpinHint, false>();
    run::<AcqRel, Pad64, SpinHint, true>();
    run::<AcqRel, Pad64, SpinHint, false>();
}

#[test]
fn seqcst_variants() {
    run::<SeqCst, Pad128, SpinHint, true>();
    run::<SeqCst, Pad64, SpinHint, false>();
}

#[test]
fn fenced_variants() {
    run::<Fenced, Pad128, SpinHint, true>();
    run::<Fenced, Pad64, SpinHint, false>();
}

#[test]
fn wait_strategies() {
    run::<AcqRel, Pad128, Spin, true>();
    run::<AcqRel, Pad128, Yield, true>();
    run::<AcqRel, Pad128, Wfe, true>();
}

#[test]
fn mutex_baseline() {
    let (mut tx, mut rx) = titan_spsc::baseline::channel(64);
    let t = std::thread::spawn(move || {
        for i in 0..200_000u64 {
            tx.send(i);
        }
    });
    for i in 0..200_000u64 {
        assert_eq!(rx.recv(), i);
    }
    t.join().unwrap();
}
