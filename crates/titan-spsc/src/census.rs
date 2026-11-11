//! Unmangled, non-inlined entry points so the instructions each ordering
//! policy compiles to can be read from the assembly by symbol name.

use crate::order::{AcqRel, Fenced, SeqCst};
use crate::pad::Pad128;
use crate::ring::{Consumer, Producer};
use crate::wait::{Wait, Wfe};
use std::sync::atomic::AtomicUsize;

macro_rules! census {
    ($push:ident, $pop:ident, $o:ty) => {
        #[no_mangle]
        #[inline(never)]
        pub fn $push(p: &mut Producer<u64, $o, Pad128, true>, v: u64) -> bool {
            p.push(v)
        }
        #[no_mangle]
        #[inline(never)]
        pub fn $pop(c: &mut Consumer<u64, $o, Pad128, true>) -> Option<u64> {
            c.pop()
        }
    };
}

census!(titan_census_push_acqrel, titan_census_pop_acqrel, AcqRel);
census!(titan_census_push_seqcst, titan_census_pop_seqcst, SeqCst);
census!(titan_census_push_fenced, titan_census_pop_fenced, Fenced);

#[no_mangle]
#[inline(never)]
pub fn titan_census_wait_wfe(watch: &AtomicUsize, seen: usize) {
    Wfe::wait(watch, seen)
}
