//! NASDAQ TotalView-ITCH 5.0 order-book reconstruction.
//!
//! * [`decode`]: zero-copy decoder over the length-prefixed binary format
//!   used by NASDAQ's published ITCH files (2-byte big-endian length, then
//!   the message).
//! * [`book`]: L3 book. Orders live in an open-addressed table keyed by order
//!   reference (Fibonacci hashing, linear probing, backward-shift deletion);
//!   price levels per instrument and side live in a sorted array with the
//!   best level last. Prices are ITCH fixed point (4 implied decimals), never
//!   floating point.
//! * [`reference`]: straightforward `BTreeMap`/`HashMap` book used only as a
//!   correctness oracle.
//! * [`gateway`]: order-entry encoders (OUCH 4.2 Enter Order, FIX 4.2
//!   NewOrderSingle) into caller-owned buffers.
//! * [`toxicity`]: VPIN from exact ITCH trade signs; bulk volume
//!   classification and tick rule accuracy against them.
//! * [`mold`]: MoldUDP64 packet framing and sequence-gap tracking.
//! * [`arb`]: A/B line arbitration and MoldUDP64 gap recovery.
//! * [`combine`]: consolidated top of book across venues, locked and crossed.
//! * [`heapbook`]: the same book as a heap of orders, for comparison.
//! * [`fanout`]: top of book shared with several reader threads (per-reader
//!   ring copies, or one seqlock snapshot per instrument).
//! * [`wire`]: one price-level update in binary, FIX 4.4 and JSON, with
//!   decoders that normalise each to the same record.
//! * [`gen`]: deterministic synthetic ITCH generator for tests and benchmarks.

pub mod book;
pub mod decode;
pub mod fanout;
pub mod gateway;
pub mod gen;
pub mod arb;
pub mod combine;
pub mod heapbook;
pub mod mold;
pub mod path;
pub mod reference;
pub mod toxicity;
pub mod wire;

/// Side of an order.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Side {
    Buy,
    Sell,
}

/// Best price and aggregate shares on one side, if the side is not empty.
pub type Top = Option<(u32, u64)>;

/// Common interface of the fast book and the reference book.
pub trait Book {
    fn apply(&mut self, m: &decode::Msg);
    fn best(&self, locate: u16, side: Side) -> Top;
    /// Full depth of one side, best level first, as (price, shares, orders).
    fn depth(&self, locate: u16, side: Side) -> Vec<(u32, u64, u32)>;
    fn live_orders(&self) -> usize;
}
