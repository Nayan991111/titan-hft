//! Order-entry encoders: what the gateway writes for one order intent.
//!
//! * [`ouch42_enter`]: NASDAQ OUCH 4.2 Enter Order message, fixed 49 bytes,
//!   integers unsigned big-endian, prices with 4 implied decimals (the same
//!   fixed point as ITCH).
//! * [`fix42_new_order`]: FIX 4.2 NewOrderSingle (`35=D`) in tag=value form
//!   with SOH separators, BodyLength (tag 9) and CheckSum (tag 10, byte sum
//!   modulo 256 as three digits).
//!
//! Both write into a caller-owned buffer and allocate nothing. Numbers are
//! formatted with a small digit loop, not `core::fmt`.

use crate::path::Intent;

pub const OUCH42_ENTER_LEN: usize = 49;
/// Upper bound on a NewOrderSingle produced by [`fix42_new_order`].
pub const FIX_MAX: usize = 256;
const SOH: u8 = 0x01;

/// Fixed fields of the session, set once.
#[derive(Clone, Copy)]
pub struct Session {
    /// OUCH firm identifier and FIX SenderCompID.
    pub firm: [u8; 4],
    /// FIX TargetCompID.
    pub target: [u8; 4],
    /// FIX date part of SendingTime and TransactTime, `YYYYMMDD`.
    pub date: [u8; 8],
}

impl Default for Session {
    fn default() -> Self {
        Session { firm: *b"TITN", target: *b"XNAS", date: *b"20190730" }
    }
}

/// Decimal token, left-justified and space-padded like an OUCH Alpha field.
#[inline(always)]
fn token(dst: &mut [u8], mut v: u64) {
    let mut tmp = [b' '; 20];
    let mut n = 0;
    loop {
        tmp[19 - n] = b'0' + (v % 10) as u8;
        v /= 10;
        n += 1;
        if v == 0 {
            break;
        }
    }
    let k = n.min(dst.len());
    dst[..k].copy_from_slice(&tmp[20 - k..]);
    dst[k..].fill(b' ');
}

/// OUCH 4.2 Enter Order. `seq` makes the day-unique order token.
#[inline(always)]
pub fn ouch42_enter(buf: &mut [u8; OUCH42_ENTER_LEN], it: &Intent, symbol: &[u8; 8], seq: u64, s: &Session) {
    buf[0] = b'O';
    token(&mut buf[1..15], seq);
    buf[15] = if it.buy { b'B' } else { b'S' };
    buf[16..20].copy_from_slice(&it.shares.to_be_bytes());
    buf[20..28].copy_from_slice(symbol);
    buf[28..32].copy_from_slice(&it.price.to_be_bytes());
    buf[32..36].copy_from_slice(&0u32.to_be_bytes()); // time in force 0: immediate or cancel
    buf[36..40].copy_from_slice(&s.firm);
    buf[40] = b'Y'; // display
    buf[41] = b'P'; // capacity: principal
    buf[42] = b'N'; // not an intermarket sweep
    buf[43..47].copy_from_slice(&0u32.to_be_bytes()); // minimum quantity
    buf[47] = b'N'; // cross type: continuous market
    buf[48] = b'R'; // customer type: retail designated (field required, value illustrative)
}

#[inline(always)]
fn put(buf: &mut [u8], at: &mut usize, s: &[u8]) {
    buf[*at..*at + s.len()].copy_from_slice(s);
    *at += s.len();
}

#[inline(always)]
fn put_u64(buf: &mut [u8], at: &mut usize, mut v: u64) {
    let mut tmp = [0u8; 20];
    let mut n = 0;
    loop {
        tmp[19 - n] = b'0' + (v % 10) as u8;
        v /= 10;
        n += 1;
        if v == 0 {
            break;
        }
    }
    put(buf, at, &tmp[20 - n..]);
}

/// `n` digits, zero-padded.
#[inline(always)]
fn put_fixed(buf: &mut [u8], at: &mut usize, mut v: u64, n: usize) {
    for i in (0..n).rev() {
        buf[*at + i] = b'0' + (v % 10) as u8;
        v /= 10;
    }
    *at += n;
}

/// `YYYYMMDD-HH:MM:SS.sss` from nanoseconds since midnight.
#[inline(always)]
fn put_time(buf: &mut [u8], at: &mut usize, date: &[u8; 8], ns: u64) {
    put(buf, at, date);
    put(buf, at, b"-");
    let ms = ns / 1_000_000;
    let (h, m, sec, milli) = (ms / 3_600_000, ms / 60_000 % 60, ms / 1_000 % 60, ms % 1_000);
    put_fixed(buf, at, h, 2);
    put(buf, at, b":");
    put_fixed(buf, at, m, 2);
    put(buf, at, b":");
    put_fixed(buf, at, sec, 2);
    put(buf, at, b".");
    put_fixed(buf, at, milli, 3);
}

/// Symbol without the space padding ITCH and OUCH use.
#[inline(always)]
fn trimmed(symbol: &[u8; 8]) -> &[u8] {
    let n = symbol.iter().position(|&c| c == b' ').unwrap_or(8);
    &symbol[..n.max(1)]
}

/// FIX 4.2 NewOrderSingle for `it`, limit order, sent at feed time `ts`
/// (ns since midnight). Returns the message length.
#[inline(always)]
pub fn fix42_new_order(buf: &mut [u8; FIX_MAX], it: &Intent, symbol: &[u8; 8], seq: u64, ts: u64, s: &Session) -> usize {
    // Body first, after room for the header prefix "8=FIX.4.2|9=NNN|".
    const PREFIX: usize = 16;
    let mut at = PREFIX;
    put(buf, &mut at, b"35=D\x0149=");
    put(buf, &mut at, &s.firm);
    put(buf, &mut at, b"\x0156=");
    put(buf, &mut at, &s.target);
    put(buf, &mut at, b"\x0134=");
    put_u64(buf, &mut at, seq);
    put(buf, &mut at, b"\x0152=");
    put_time(buf, &mut at, &s.date, ts);
    put(buf, &mut at, b"\x0111=");
    put_u64(buf, &mut at, seq);
    put(buf, &mut at, b"\x0121=1\x0155=");
    put(buf, &mut at, trimmed(symbol));
    put(buf, &mut at, if it.buy { b"\x0154=1\x0160=" } else { b"\x0154=2\x0160=" });
    put_time(buf, &mut at, &s.date, ts);
    put(buf, &mut at, b"\x0138=");
    put_u64(buf, &mut at, it.shares as u64);
    put(buf, &mut at, b"\x0140=2\x0144=");
    put_u64(buf, &mut at, it.price as u64 / 10_000);
    put(buf, &mut at, b".");
    put_fixed(buf, &mut at, it.price as u64 % 10_000, 4);
    put(buf, &mut at, &[SOH]);
    let body = at - PREFIX;
    debug_assert!(body < 1000);
    // Header prefix, right-aligned against the body: "8=FIX.4.2|9=" + digits + "|".
    let digits = if body >= 100 { 3 } else if body >= 10 { 2 } else { 1 };
    let start = PREFIX - (13 + digits);
    let mut h = start;
    put(buf, &mut h, b"8=FIX.4.2\x019=");
    put_fixed(buf, &mut h, body as u64, digits);
    put(buf, &mut h, &[SOH]);
    debug_assert_eq!(h, PREFIX);
    // Trailer.
    let sum = buf[start..at].iter().fold(0u32, |a, &c| a + c as u32) % 256;
    put(buf, &mut at, b"10=");
    put_fixed(buf, &mut at, sum as u64, 3);
    put(buf, &mut at, &[SOH]);
    // Move to the front so the message starts at offset 0.
    buf.copy_within(start..at, 0);
    at - start
}

#[cfg(test)]
mod tests {
    use super::*;

    fn intent(buy: bool) -> Intent {
        Intent { price: 1_234_500, shares: 250, locate: 7, buy }
    }

    #[test]
    fn ouch_layout() {
        let mut b = [0u8; OUCH42_ENTER_LEN];
        ouch42_enter(&mut b, &intent(true), b"AAPL    ", 42, &Session::default());
        assert_eq!(b[0], b'O');
        assert_eq!(&b[1..15], b"42            ");
        assert_eq!(b[15], b'B');
        assert_eq!(u32::from_be_bytes(b[16..20].try_into().unwrap()), 250);
        assert_eq!(&b[20..28], b"AAPL    ");
        assert_eq!(u32::from_be_bytes(b[28..32].try_into().unwrap()), 1_234_500);
        assert_eq!(&b[36..40], b"TITN");
        assert_eq!(b[48], b'R');
    }

    fn fields(m: &[u8]) -> Vec<(String, String)> {
        m.split(|&c| c == SOH)
            .filter(|f| !f.is_empty())
            .map(|f| {
                let s = std::str::from_utf8(f).unwrap();
                let (k, v) = s.split_once('=').unwrap();
                (k.to_string(), v.to_string())
            })
            .collect()
    }

    #[test]
    fn fix_body_length_and_checksum() {
        let mut b = [0u8; FIX_MAX];
        // 09:30:01.234 after midnight.
        let ts = ((9 * 3600 + 30 * 60 + 1) * 1_000 + 234) * 1_000_000u64;
        for (seq, buy) in [(1u64, true), (987_654_321, false)] {
            let n = fix42_new_order(&mut b, &intent(buy), b"MSFT    ", seq, ts, &Session::default());
            let m = &b[..n];
            let f = fields(m);
            assert_eq!(f[0], ("8".into(), "FIX.4.2".into()));
            assert_eq!(f[1].0, "9");
            assert_eq!(f[2], ("35".into(), "D".into()));
            // BodyLength: bytes after the SOH that ends tag 9, up to and
            // including the SOH before tag 10.
            let body_start = m.iter().enumerate().filter(|(_, &c)| c == SOH).nth(1).unwrap().0 + 1;
            let trailer = n - 7;
            assert_eq!(&m[trailer..trailer + 3], b"10=");
            assert_eq!(f[1].1.parse::<usize>().unwrap(), trailer - body_start);
            let sum = m[..trailer].iter().map(|&c| c as u32).sum::<u32>() % 256;
            assert_eq!(f.last().unwrap(), &("10".into(), format!("{sum:03}")));
            let get = |k: &str| f.iter().find(|(t, _)| t == k).map(|(_, v)| v.clone()).unwrap();
            assert_eq!(get("55"), "MSFT");
            assert_eq!(get("54"), if buy { "1" } else { "2" });
            assert_eq!(get("38"), "250");
            assert_eq!(get("44"), "123.4500");
            assert_eq!(get("34"), seq.to_string());
            assert_eq!(get("60"), "20190730-09:30:01.234");
        }
    }
}
