//! The trading decision shared by `titan-t2t` and `titan-pipeline`: a signal
//! on the touched instrument's best bid and ask, and pre-trade risk checks of
//! the kind SEC Rule 15c3-5 (the Market Access Rule) requires before an order
//! leaves a broker-dealer.
//!
//! Everything is fixed-point integer arithmetic on preallocated state. Prices
//! are ITCH price units (4 implied decimals), so a notional is
//! `price * shares` in units of $0.0001.

use crate::decode::Msg;
use crate::Top;
use std::sync::atomic::{AtomicBool, Ordering};

/// Order intent sent to the gateway.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
#[repr(C)]
pub struct Intent {
    pub price: u32,
    pub shares: u32,
    pub locate: u16,
    pub buy: bool,
}

#[inline(always)]
pub fn locate_of(m: &Msg) -> u16 {
    match *m {
        Msg::Add { locate, .. }
        | Msg::Executed { locate, .. }
        | Msg::ExecutedWithPrice { locate, .. }
        | Msg::Cancel { locate, .. }
        | Msg::Delete { locate, .. }
        | Msg::Replace { locate, .. } => locate,
        Msg::Other { .. } => 0,
    }
}

/// ITCH 5.0 timestamp (nanoseconds since midnight) from a message body.
#[inline(always)]
pub fn timestamp(body: &[u8]) -> u64 {
    if body.len() < 11 {
        return 0;
    }
    let b = &body[5..11];
    ((b[0] as u64) << 40)
        | ((b[1] as u64) << 32)
        | ((b[2] as u64) << 24)
        | ((b[3] as u64) << 16)
        | ((b[4] as u64) << 8)
        | b[5] as u64
}

/// Update the chained state from the book; return an intent if the signal
/// fires. The state is rotated and mixed with the best prices and sizes on
/// every call and order sizes are taken from it, so each message's work
/// depends on the previous one.
///
/// The signal fires when the spread is positive and at most `max_spread`
/// and one side shows more than twice the resting size of the other; the
/// intent takes the opposite best price.
#[inline(always)]
pub fn signal(state: &mut u64, locate: u16, bid: Top, ask: Top, max_spread: u32) -> Option<Intent> {
    let (Some((bp, bs)), Some((ap, asz))) = (bid, ask) else {
        *state = state.rotate_left(3) ^ locate as u64;
        return None;
    };
    *state = state.rotate_left(5) ^ bp as u64 ^ ((ap as u64) << 20) ^ bs ^ (asz << 7);
    if ap > bp && ap - bp <= max_spread && (bs > 2 * asz || asz > 2 * bs) {
        let buy = bs > asz;
        Some(Intent { price: if buy { ap } else { bp }, shares: 100 + (*state & 0xff) as u32, locate, buy })
    } else {
        None
    }
}

/// Reasons a pre-trade check can reject an intent, in the order checked.
pub const REJECT_NAMES: [&str; 8] =
    ["kill", "size", "collar", "notional", "position", "credit", "duplicate", "throttle"];

/// Limits for [`Risk`]. Defaults are illustrative, not a firm's settings.
#[derive(Clone, Copy, Debug)]
pub struct Limits {
    /// Largest order, shares.
    pub max_shares: u32,
    /// Largest distance of the order price from the mid, basis points.
    pub collar_bps: u64,
    /// Largest single-order notional, $0.0001 units.
    pub max_notional: u64,
    /// Largest absolute position per instrument, shares.
    pub max_position: i64,
    /// Largest open exposure, the sum over instruments of |position| times
    /// the last order price, $0.0001 units (credit limit).
    pub max_exposure: u64,
    /// Same instrument, side and price again within this many ns is a duplicate.
    pub duplicate_ns: u64,
    /// Order-rate throttle: tokens added per second, and bucket depth.
    pub rate_per_s: u64,
    pub burst: u64,
}

impl Default for Limits {
    fn default() -> Self {
        Limits {
            max_shares: 1_000,
            collar_bps: 50,
            max_notional: 500_000 * 10_000,
            max_position: 10_000,
            max_exposure: 1_000_000_000 * 10_000,
            duplicate_ns: 1_000_000,
            rate_per_s: 50_000,
            burst: 100,
        }
    }
}

impl Limits {
    /// Limits no order can breach, so every check runs to the end and every
    /// order is accepted: isolates the cost of the checks themselves.
    pub fn open() -> Self {
        Limits {
            max_shares: u32::MAX,
            collar_bps: u64::MAX / (u32::MAX as u64 * 10_000),
            max_notional: u64::MAX,
            max_position: i64::MAX / 2,
            max_exposure: u64::MAX / 2,
            duplicate_ns: 0,
            rate_per_s: 1_000_000_000,
            burst: u64::MAX / 2,
        }
    }
}

#[derive(Clone, Copy, Default)]
struct Last {
    ts: u64,
    price: u32,
    buy: bool,
    valid: bool,
}

/// Pre-trade risk state. Accepted orders are assumed filled in full, which
/// moves position and gross notional as fast as possible and so exercises
/// the limits; this is a cost model, not a fill simulation.
pub struct Risk<'a> {
    pub limits: Limits,
    kill: &'a AtomicBool,
    position: Vec<i64>,
    exposure: Vec<u64>,
    last: Vec<Last>,
    open: u64,
    tokens: u64,
    token_ts: u64,
    pub accepted: u64,
    pub rejected: [u64; 8],
}

impl<'a> Risk<'a> {
    pub fn new(instruments: usize, limits: Limits, kill: &'a AtomicBool) -> Self {
        Risk {
            limits,
            kill,
            position: vec![0; instruments],
            exposure: vec![0; instruments],
            last: vec![Last::default(); instruments],
            open: 0,
            tokens: limits.burst,
            token_ts: 0,
            accepted: 0,
            rejected: [0; 8],
        }
    }

    pub fn reset(&mut self) {
        self.position.iter_mut().for_each(|p| *p = 0);
        self.exposure.iter_mut().for_each(|e| *e = 0);
        self.last.iter_mut().for_each(|l| *l = Last::default());
        self.open = 0;
        self.tokens = self.limits.burst;
        self.token_ts = 0;
        self.accepted = 0;
        self.rejected = [0; 8];
    }

    /// Run every check on `it`, quoted against `mid` at feed time `ts` (ns).
    /// Returns `true` if the order may be sent; state changes only on accept.
    #[inline(always)]
    pub fn check(&mut self, it: &Intent, mid: u64, ts: u64) -> bool {
        let r = self.verdict(it, mid, ts);
        if r == 0 {
            true
        } else {
            self.rejected[(r - 1) as usize] += 1;
            false
        }
    }

    #[inline(always)]
    fn verdict(&mut self, it: &Intent, mid: u64, ts: u64) -> u8 {
        let l = &self.limits;
        // 1. Kill switch: another thread may set it at any time.
        if self.kill.load(Ordering::Relaxed) {
            return 1;
        }
        // 2. Maximum order size.
        if it.shares > l.max_shares {
            return 2;
        }
        // 3. Price collar against the mid, without division.
        let price = it.price as u64;
        let dist = price.abs_diff(mid);
        if dist * 10_000 > l.collar_bps * mid {
            return 3;
        }
        // 4. Maximum single-order notional.
        let notional = price * it.shares as u64;
        if notional > l.max_notional {
            return 4;
        }
        // 5. Position limit per instrument (assumes a full fill).
        let i = it.locate as usize;
        let Some(&pos) = self.position.get(i) else {
            return 5;
        };
        let next = if it.buy { pos + it.shares as i64 } else { pos - it.shares as i64 };
        if next.abs() > l.max_position {
            return 5;
        }
        // 6. Open exposure (credit) limit after this order.
        let new_exposure = next.unsigned_abs() * price;
        let open = self.open - self.exposure[i] + new_exposure;
        if open > l.max_exposure {
            return 6;
        }
        // 7. Duplicate: same instrument, side and price within the window.
        let last = self.last[i];
        if last.valid && last.buy == it.buy && last.price == it.price && ts.wrapping_sub(last.ts) < l.duplicate_ns {
            return 7;
        }
        // 8. Order-rate throttle: token bucket refilled from feed time.
        let elapsed = ts.saturating_sub(self.token_ts);
        let refill = elapsed.saturating_mul(l.rate_per_s) / 1_000_000_000;
        let mut tokens = self.tokens;
        if refill > 0 {
            tokens = (tokens + refill).min(l.burst);
            self.token_ts = ts;
        }
        if tokens == 0 {
            self.tokens = 0;
            return 8;
        }
        // Accept.
        self.tokens = tokens - 1;
        self.position[i] = next;
        self.exposure[i] = new_exposure;
        self.open = open;
        self.last[i] = Last { ts, price: it.price, buy: it.buy, valid: true };
        self.accepted += 1;
        0
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn intent(locate: u16, buy: bool, price: u32, shares: u32) -> Intent {
        Intent { price, shares, locate, buy }
    }

    #[test]
    fn each_check_rejects_its_own_case() {
        let kill = AtomicBool::new(false);
        let mid = 1_000_000u64; // $100.0000
        let mut r = Risk::new(4, Limits::default(), &kill);
        let mut ts = 10_000_000_000u64;
        assert!(r.check(&intent(1, true, 1_000_000, 100), mid, ts));
        ts += 2_000_000;
        assert!(!r.check(&intent(1, true, 1_000_000, 5_000), mid, ts)); // size
        assert!(!r.check(&intent(1, true, 1_010_000, 100), mid, ts)); // collar: 100 bps away
        assert!(!r.check(&intent(9, true, 1_000_000, 100), mid, ts)); // unknown instrument
        kill.store(true, Ordering::Relaxed);
        assert!(!r.check(&intent(1, true, 1_000_000, 100), mid, ts)); // kill
        kill.store(false, Ordering::Relaxed);
        assert!(r.check(&intent(1, true, 1_000_100, 100), mid, ts));
        assert!(!r.check(&intent(1, true, 1_000_100, 100), mid, ts + 10)); // duplicate within 1 ms
        let tight = Limits { max_notional: 50 * 10_000 * 100, ..Limits::default() }; // $5,000
        let mut r2 = Risk::new(4, tight, &kill);
        assert!(!r2.check(&intent(1, true, 1_000_000, 100), mid, ts)); // $10,000 order
        assert_eq!(r.rejected[..3], [1, 1, 1]);
        assert_eq!(r.rejected[4], 1); // unknown instrument counted as a position reject
        assert_eq!(r.rejected[6], 1);
        assert_eq!(r2.rejected[3], 1);
    }

    #[test]
    fn position_credit_and_throttle_limits() {
        let kill = AtomicBool::new(false);
        let mid = 100_000u64; // $10.0000
        let l = Limits { max_position: 250, burst: 3, rate_per_s: 1_000, ..Limits::default() }; // one token per ms
        let mut r = Risk::new(2, l, &kill);
        let ts0 = 1_000_000_000u64;
        assert!(r.check(&intent(0, true, 100_000, 200), mid, ts0));
        assert!(!r.check(&intent(0, true, 100_010, 100), mid, ts0 + 1)); // 300 > 250
        assert!(r.check(&intent(0, false, 100_010, 400), mid, ts0 + 2)); // 200 - 400 = -200
        assert!(r.check(&intent(1, true, 100_000, 100), mid, ts0 + 3));
        assert!(!r.check(&intent(1, false, 100_020, 100), mid, ts0 + 4)); // bucket empty
        assert!(r.check(&intent(1, false, 100_030, 100), mid, ts0 + 2_000_000)); // refilled
        assert_eq!(r.rejected[4], 1);
        assert_eq!(r.rejected[7], 1);
        let c = Limits { max_exposure: 100_000 * 300, ..Limits::default() }; // 300 shares at $10
        let mut r3 = Risk::new(1, c, &kill);
        assert!(r3.check(&intent(0, true, 100_000, 200), mid, ts0));
        assert!(!r3.check(&intent(0, true, 100_010, 200), mid, ts0 + 2_000_000)); // 400 shares open
        assert!(r3.check(&intent(0, false, 100_020, 200), mid, ts0 + 4_000_000)); // back to flat
        assert!(r3.check(&intent(0, false, 100_030, 250), mid, ts0 + 6_000_000)); // short 250
        assert_eq!(r3.rejected[5], 1);
        r3.reset();
        assert_eq!(r3.accepted, 0);
        assert!(r3.check(&intent(0, false, 100_010, 200), mid, ts0));
    }

    #[test]
    fn timestamp_is_big_endian_48_bit() {
        let mut b = [0u8; 19];
        b[5..11].copy_from_slice(&[0x00, 0x1F, 0x2E, 0x3D, 0x4C, 0x5B]);
        assert_eq!(timestamp(&b), 0x001F_2E3D_4C5B);
        assert_eq!(timestamp(&b[..10]), 0);
    }
}
