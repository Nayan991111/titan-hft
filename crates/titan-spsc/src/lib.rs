//! Single-producer single-consumer rings built for controlled experiments.
//!
//! Every property that affects cross-core handoff cost is a type parameter,
//! so each combination compiles to its own specialised code:
//!
//! * [`order`]: how index loads and stores are ordered
//!   (`ldapr`/`stlr`, `ldar`/`stlr`, or relaxed accesses with `dmb ish`);
//! * [`pad`]: whether the producer and consumer indices are padded to 64 or
//!   128 bytes;
//! * `CACHED`: whether each side keeps a private copy of the other side's
//!   index and refreshes it only when the ring looks full or empty;
//! * [`wait`]: what a blocked side executes while it waits.
//!
//! [`baseline::MutexChannel`] is a `Mutex<VecDeque>` reference point.

pub mod baseline;
pub mod census;
pub mod order;
pub mod pad;
pub mod ring;
pub mod wait;

/// Blocking send side of a channel.
pub trait Tx: Send {
    fn send(&mut self, v: u64);
}

/// Blocking receive side of a channel.
pub trait Rx: Send {
    fn recv(&mut self) -> u64;
}
