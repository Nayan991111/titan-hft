//! One order-book price-level update carried in three inbound wire formats,
//! and decoders that normalise each back to the same record.
//!
//! The update is the depth-diff form exchanges publish for local books: the
//! new total quantity resting at one price on one side of one instrument,
//! 0 meaning the level is gone. Formats:
//!
//! * **binary**: a fixed 32-byte big-endian layout (type, side, instrument
//!   locate, sequence, time, price with 4 implied decimals, quantity), the
//!   style of ITCH and of SBE market data;
//! * **FIX 4.4** MarketDataIncrementalRefresh (35=X) with the required
//!   standard header (8, 9, 35, 49 SenderCompID, 56 TargetCompID,
//!   34 MsgSeqNum, 52 SendingTime) and one entry:
//!   279 MDUpdateAction (0 new, 1 change, 2 delete; must be first in the
//!   group), 269 MDEntryType (0 bid, 1 offer), 55 Symbol, 270 MDEntryPx,
//!   271 MDEntrySize (omitted on delete), with BodyLength and CheckSum;
//! * **JSON** in the shape of Binance's spot diff depth stream:
//!   `{"e":"depthUpdate","E":ms,"s":"SYM","U":first,"u":final,"b":[["px","qty"]],"a":[]}`,
//!   prices and quantities as strings, here one level per event.
//!
//! Decoders: binary; FIX validated (BeginString, BodyLength, CheckSum, 35 as
//! the third field, every header and entry field present once, SendingTime
//! format, header before body, any other tag rejected); JSON with
//! `serde_json` into a typed, borrowing struct; JSON with a scanner
//! specialised to the key order of the documented example (anything else is
//! rejected, so a reordered stream fails safe).
//! Every decoder maps the text symbol back to the instrument locate through
//! [`Symbols`], which is part of the cost of normalising text formats.

use crate::gateway::{price4, soh_scalar, symbol8, uint, FIX_MAX_FIELDS};

/// The normalised record: what every decoder must return.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub struct Level {
    pub seq: u64,
    pub locate: u16,
    pub buy: bool,
    /// Price with 4 implied decimals (ITCH units).
    pub price: u32,
    /// New total quantity at the level; 0: the level is gone.
    pub qty: u64,
}

/// Length of the binary record.
pub const BIN_LEN: usize = 32;

/// Instrument locate to symbol (8 bytes, space padded) and back.
pub struct Symbols {
    by_locate: Vec<[u8; 8]>,
    keys: Vec<u64>,
    vals: Vec<u16>,
    shift: u32,
}

#[inline(always)]
fn key8(s: &[u8; 8]) -> u64 {
    u64::from_le_bytes(*s)
}

impl Symbols {
    /// `by_locate[l]` is the symbol of locate `l`; empty symbols are
    /// skipped. Symbols must be unique.
    pub fn new(by_locate: Vec<[u8; 8]>) -> Symbols {
        let n = by_locate.len().max(1);
        let mut cap = 16usize;
        while cap < 2 * n {
            cap *= 2;
        }
        let shift = 64 - cap.trailing_zeros();
        let mut t = Symbols { by_locate, keys: vec![0; cap], vals: vec![0; cap], shift };
        for l in 0..t.by_locate.len() {
            let s = t.by_locate[l];
            if s == [b' '; 8] {
                continue;
            }
            let k = key8(&s);
            let mut i = t.slot(k);
            while t.keys[i] != 0 {
                assert_ne!(t.keys[i], k, "duplicate symbol");
                i = (i + 1) & (cap - 1);
            }
            t.keys[i] = k;
            t.vals[i] = l as u16;
        }
        t
    }

    #[inline(always)]
    fn slot(&self, k: u64) -> usize {
        (k.wrapping_mul(0x9e37_79b9_7f4a_7c15) >> self.shift) as usize
    }

    pub fn symbol(&self, locate: u16) -> &[u8; 8] {
        &self.by_locate[locate as usize]
    }

    /// The locate of a symbol given as text (1 to 8 bytes, no padding).
    #[inline(always)]
    pub fn locate(&self, text: &[u8]) -> Option<u16> {
        let s = symbol8(text)?;
        let k = key8(&s);
        let mask = self.keys.len() - 1;
        let mut i = self.slot(k);
        loop {
            let ki = self.keys[i];
            if ki == k {
                return Some(self.vals[i]);
            }
            if ki == 0 {
                return None;
            }
            i = (i + 1) & mask;
        }
    }
}

fn trimmed(s: &[u8; 8]) -> &[u8] {
    let n = s.iter().rposition(|&c| c != b' ').map_or(0, |i| i + 1);
    &s[..n]
}

/// `price` (4 implied decimals) as text with exactly 4 decimals.
fn put_price(out: &mut Vec<u8>, price: u32) {
    let int = price / 10_000;
    let frac = price % 10_000;
    out.extend_from_slice(int.to_string().as_bytes());
    out.push(b'.');
    out.extend_from_slice(format!("{frac:04}").as_bytes());
}

// ---------------------------------------------------------------- binary

pub fn encode_bin(u: &Level, ts_ns: u64, out: &mut Vec<u8>) {
    out.push(b'L');
    out.push(if u.buy { b'B' } else { b'S' });
    out.extend_from_slice(&u.locate.to_be_bytes());
    out.extend_from_slice(&u.seq.to_be_bytes());
    out.extend_from_slice(&ts_ns.to_be_bytes());
    out.extend_from_slice(&u.price.to_be_bytes());
    out.extend_from_slice(&u.qty.to_be_bytes());
}

#[inline(always)]
pub fn decode_bin(b: &[u8]) -> Option<Level> {
    if b.len() != BIN_LEN || b[0] != b'L' {
        return None;
    }
    let buy = match b[1] {
        b'B' => true,
        b'S' => false,
        _ => return None,
    };
    Some(Level {
        buy,
        locate: u16::from_be_bytes([b[2], b[3]]),
        seq: u64::from_be_bytes(b[4..12].try_into().unwrap()),
        price: u32::from_be_bytes(b[20..24].try_into().unwrap()),
        qty: u64::from_be_bytes(b[24..32].try_into().unwrap()),
    })
}

// ---------------------------------------------------------------- FIX 4.4

/// FIX MDUpdateAction for an update, given whether the level existed before.
pub fn md_action(existed: bool, qty: u64) -> u8 {
    if qty == 0 {
        b'2'
    } else if existed {
        b'1'
    } else {
        b'0'
    }
}

/// Date part of SendingTime; the time of day comes from the update.
const SENDING_DATE: &[u8; 8] = b"20190730";

/// One-entry MarketDataIncrementalRefresh. `action` from [`md_action`];
/// `ts_ns` (nanoseconds since midnight) gives SendingTime to the millisecond.
pub fn encode_fix(u: &Level, action: u8, ts_ns: u64, sym: &Symbols, out: &mut Vec<u8>) {
    let mut body = Vec::with_capacity(128);
    body.extend_from_slice(b"35=X\x0149=TITAN\x0156=CLIENT\x0134=");
    body.extend_from_slice(u.seq.to_string().as_bytes());
    let ms = (ts_ns / 1_000_000) % 86_400_000;
    body.extend_from_slice(b"\x0152=");
    body.extend_from_slice(SENDING_DATE);
    body.extend_from_slice(
        format!("-{:02}:{:02}:{:02}.{:03}", ms / 3_600_000, ms / 60_000 % 60, ms / 1000 % 60, ms % 1000).as_bytes(),
    );
    body.extend_from_slice(b"\x01268=1\x01279=");
    body.push(action);
    body.extend_from_slice(b"\x01269=");
    body.push(if u.buy { b'0' } else { b'1' });
    body.extend_from_slice(b"\x0155=");
    body.extend_from_slice(trimmed(sym.symbol(u.locate)));
    body.extend_from_slice(b"\x01270=");
    put_price(&mut body, u.price);
    body.push(1);
    if action != b'2' {
        body.extend_from_slice(b"271=");
        body.extend_from_slice(u.qty.to_string().as_bytes());
        body.push(1);
    }
    let start = out.len();
    out.extend_from_slice(b"8=FIX.4.4\x019=");
    out.extend_from_slice(body.len().to_string().as_bytes());
    out.push(1);
    out.extend_from_slice(&body);
    let sum = out[start..].iter().fold(0u32, |a, &c| a + c as u32) % 256;
    out.extend_from_slice(format!("10={sum:03}\x01").as_bytes());
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum WireError {
    Framing,
    BeginString,
    BodyLength,
    CheckSum,
    Fields,
    Symbol,
    /// FIX: 35 not the third field, a header field after the group, or an
    /// entry field before its 268 or out of order.
    Order,
    /// FIX: a field present twice.
    Duplicate,
    /// FIX: a tag this message type does not carry.
    UnknownTag,
    /// FIX: a required header field absent.
    Missing,
    /// FIX: a field with a value outside its form (MsgType, CompID, SendingTime).
    Value,
}

/// SendingTime in the millisecond form of UTCTimestamp, YYYYMMDD-HH:MM:SS.sss,
/// seconds 00 to 59 (a format check, not a calendar check; the form without
/// milliseconds and leap second 60, both allowed by FIX, are not produced
/// here and are rejected).
#[inline(always)]
fn utc_timestamp_ok(v: &[u8]) -> bool {
    if v.len() != 21 || v[8] != b'-' || v[11] != b':' || v[14] != b':' || v[17] != b'.' {
        return false;
    }
    let mut d = [0u8; 21];
    for (k, &c) in v.iter().enumerate() {
        if !matches!(k, 8 | 11 | 14 | 17) {
            if !c.is_ascii_digit() {
                return false;
            }
            d[k] = c - b'0';
        }
    }
    let two = |k: usize| d[k] * 10 + d[k + 1];
    (1..=12).contains(&two(4)) && (1..=31).contains(&two(6)) && two(9) < 24 && two(12) < 60 && two(15) < 60
}

/// Validated parse of a one-entry MarketDataIncrementalRefresh.
#[inline(always)]
pub fn decode_fix(m: &[u8], sym: &Symbols) -> Result<Level, WireError> {
    let mut soh = [0u16; FIX_MAX_FIELDS];
    let n = soh_scalar(m, &mut soh);
    if n == usize::MAX || n < 4 || soh[n - 1] as usize != m.len() - 1 {
        return Err(WireError::Framing);
    }
    let soh = &soh[..n];
    let mut f = [(0u32, 0u16, 0u16); FIX_MAX_FIELDS];
    let mut start = 0usize;
    for (k, &end) in soh.iter().enumerate() {
        let end = end as usize;
        let mut tag = 0u32;
        let mut j = start;
        while j < end && m[j] != b'=' {
            let d = m[j].wrapping_sub(b'0');
            if d > 9 || j - start == 9 {
                return Err(WireError::Framing);
            }
            tag = tag * 10 + d as u32;
            j += 1;
        }
        if j == end || j == start {
            return Err(WireError::Framing);
        }
        f[k] = (tag, (j + 1) as u16, end as u16);
        start = end + 1;
    }
    let val = |k: usize| &m[f[k].1 as usize..f[k].2 as usize];
    if f[0].0 != 8 || val(0) != b"FIX.4.4" {
        return Err(WireError::BeginString);
    }
    if f[1].0 != 9 || f[n - 1].0 != 10 {
        return Err(WireError::Framing);
    }
    let body_start = soh[1] as usize + 1;
    let trailer = soh[n - 2] as usize + 1;
    if uint(val(1)) != Some((trailer - body_start) as u64) {
        return Err(WireError::BodyLength);
    }
    let sum = m[..trailer].iter().fold(0u32, |a, &c| a + c as u32) % 256;
    if val(n - 1).len() != 3 || uint(val(n - 1)) != Some(sum as u64) {
        return Err(WireError::CheckSum);
    }
    if f[2].0 != 35 {
        return Err(WireError::Order);
    }
    let (mut ty, mut seq, mut entries, mut action, mut side, mut locate, mut price, mut qty) =
        (false, None, None, None, None, None, None, None);
    let (mut sender, mut target, mut sent) = (false, false, false);
    let mut first_in_group = false;
    let mut seen = 0u32;
    for (k, fk) in f.iter().enumerate().take(n - 1).skip(2) {
        let v = val(k);
        let bit = match fk.0 {
            35 => 0,
            49 => 1,
            56 => 2,
            34 => 3,
            52 => 4,
            268 => 5,
            279 => 6,
            269 => 7,
            55 => 8,
            270 => 9,
            271 => 10,
            _ => return Err(WireError::UnknownTag),
        };
        if seen & (1 << bit) != 0 {
            return Err(WireError::Duplicate);
        }
        if (bit <= 4 && seen & (1 << 5) != 0) || (bit > 5 && seen & (1 << 5) == 0) {
            return Err(WireError::Order);
        }
        seen |= 1 << bit;
        match fk.0 {
            35 => {
                if v != b"X" {
                    return Err(WireError::Value);
                }
                ty = true;
            }
            49 | 56 => {
                if v.is_empty() {
                    return Err(WireError::Value);
                }
                if fk.0 == 49 {
                    sender = true;
                } else {
                    target = true;
                }
            }
            52 => {
                if !utc_timestamp_ok(v) {
                    return Err(WireError::Value);
                }
                sent = true;
            }
            34 => seq = uint(v),
            268 => {
                entries = uint(v);
                first_in_group = true;
                continue;
            }
            279 => {
                if !first_in_group {
                    return Err(WireError::Order);
                }
                action = match v {
                    b"0" | b"1" | b"2" => Some(v[0]),
                    _ => None,
                }
            }
            269 => {
                side = match v {
                    b"0" => Some(true),
                    b"1" => Some(false),
                    _ => None,
                }
            }
            55 => locate = Some(sym.locate(v).ok_or(WireError::Symbol)?),
            270 => price = price4(v),
            271 => qty = uint(v),
            _ => unreachable!(),
        }
        first_in_group = false;
    }
    let qty = match (action, qty) {
        (Some(b'2'), None) => Some(0),
        (Some(b'0' | b'1'), Some(q)) if q > 0 => Some(q),
        _ => None,
    };
    if !(ty && sender && target && sent) {
        return Err(WireError::Missing);
    }
    match (ty, seq, entries, side, locate, price, qty) {
        (true, Some(seq), Some(1), Some(buy), Some(locate), Some(price), Some(qty)) => Ok(Level { seq, locate, buy, price, qty }),
        _ => Err(WireError::Fields),
    }
}

// ---------------------------------------------------------------- JSON

/// One event in the shape of Binance's spot diff depth stream, one level.
pub fn encode_json(u: &Level, ts_ms: u64, sym: &Symbols, out: &mut Vec<u8>) {
    out.extend_from_slice(b"{\"e\":\"depthUpdate\",\"E\":");
    out.extend_from_slice(ts_ms.to_string().as_bytes());
    out.extend_from_slice(b",\"s\":\"");
    out.extend_from_slice(trimmed(sym.symbol(u.locate)));
    out.extend_from_slice(b"\",\"U\":");
    out.extend_from_slice(u.seq.to_string().as_bytes());
    out.extend_from_slice(b",\"u\":");
    out.extend_from_slice(u.seq.to_string().as_bytes());
    let mut level = Vec::with_capacity(32);
    level.extend_from_slice(b"[[\"");
    put_price(&mut level, u.price);
    level.extend_from_slice(b"\",\"");
    level.extend_from_slice(u.qty.to_string().as_bytes());
    level.extend_from_slice(b"\"]]");
    if u.buy {
        out.extend_from_slice(b",\"b\":");
        out.extend_from_slice(&level);
        out.extend_from_slice(b",\"a\":[]}");
    } else {
        out.extend_from_slice(b",\"b\":[],\"a\":");
        out.extend_from_slice(&level);
        out.push(b'}');
    }
}

#[derive(serde::Deserialize)]
#[allow(non_snake_case)]
struct DepthEvent<'a> {
    e: &'a str,
    #[allow(dead_code)]
    E: u64,
    s: &'a str,
    U: u64,
    u: u64,
    #[serde(borrow)]
    b: Vec<[&'a str; 2]>,
    #[serde(borrow)]
    a: Vec<[&'a str; 2]>,
}

/// JSON through `serde_json` into a typed struct that borrows its strings.
#[inline(always)]
pub fn decode_json_serde(j: &[u8], sym: &Symbols) -> Result<Level, WireError> {
    let d: DepthEvent = serde_json::from_slice(j).map_err(|_| WireError::Framing)?;
    if d.e != "depthUpdate" || d.U != d.u {
        return Err(WireError::Fields);
    }
    let (buy, lv) = match (d.b.as_slice(), d.a.as_slice()) {
        ([l], []) => (true, l),
        ([], [l]) => (false, l),
        _ => return Err(WireError::Fields),
    };
    let locate = sym.locate(d.s.as_bytes()).ok_or(WireError::Symbol)?;
    let price = price4(lv[0].as_bytes()).ok_or(WireError::Fields)?;
    let qty = uint(lv[1].as_bytes()).ok_or(WireError::Fields)?;
    Ok(Level { seq: d.u, locate, buy, price, qty })
}

struct Cursor<'a> {
    b: &'a [u8],
    i: usize,
}

impl Cursor<'_> {
    #[inline(always)]
    fn lit(&mut self, s: &[u8]) -> Result<(), WireError> {
        if self.b.len() >= self.i + s.len() && &self.b[self.i..self.i + s.len()] == s {
            self.i += s.len();
            Ok(())
        } else {
            Err(WireError::Framing)
        }
    }

    /// Bytes up to (not including) the next `stop`.
    #[inline(always)]
    fn until(&mut self, stop: u8) -> Result<&[u8], WireError> {
        let start = self.i;
        while self.i < self.b.len() && self.b[self.i] != stop {
            self.i += 1;
        }
        if self.i == self.b.len() {
            return Err(WireError::Framing);
        }
        Ok(&self.b[start..self.i])
    }

    #[inline(always)]
    fn level(&mut self) -> Result<Option<(u32, u64)>, WireError> {
        self.lit(b"[")?;
        if self.b.get(self.i) == Some(&b']') {
            self.i += 1;
            return Ok(None);
        }
        self.lit(b"[\"")?;
        let p = price4(self.until(b'"')?).ok_or(WireError::Fields)?;
        self.lit(b"\",\"")?;
        let q = uint(self.until(b'"')?).ok_or(WireError::Fields)?;
        self.lit(b"\"]]")?;
        Ok(Some((p, q)))
    }
}

/// JSON with a scanner specialised to the key order of the documented example and to
/// compact output (no whitespace); anything else is rejected, not guessed.
#[inline(always)]
pub fn decode_json_fixed(j: &[u8], sym: &Symbols) -> Result<Level, WireError> {
    let mut c = Cursor { b: j, i: 0 };
    c.lit(b"{\"e\":\"depthUpdate\",\"E\":")?;
    uint(c.until(b',')?).ok_or(WireError::Fields)?;
    c.lit(b",\"s\":\"")?;
    let locate = sym.locate(c.until(b'"')?).ok_or(WireError::Symbol)?;
    c.lit(b"\",\"U\":")?;
    let first = uint(c.until(b',')?).ok_or(WireError::Fields)?;
    c.lit(b",\"u\":")?;
    let last = uint(c.until(b',')?).ok_or(WireError::Fields)?;
    c.lit(b",\"b\":")?;
    let bid = c.level()?;
    c.lit(b",\"a\":")?;
    let ask = c.level()?;
    c.lit(b"}")?;
    if c.i != j.len() || first != last {
        return Err(WireError::Fields);
    }
    let (buy, (price, qty)) = match (bid, ask) {
        (Some(l), None) => (true, l),
        (None, Some(l)) => (false, l),
        _ => return Err(WireError::Fields),
    };
    Ok(Level { seq: last, locate, buy, price, qty })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn syms() -> Symbols {
        let mut v = vec![[b' '; 8]; 4];
        v[1] = *b"SPY     ";
        v[2] = *b"BRK.A   ";
        v[3] = *b"ZVZZT   ";
        Symbols::new(v)
    }

    fn cases() -> Vec<(Level, bool)> {
        vec![
            (Level { seq: 1, locate: 1, buy: true, price: 2_981_200, qty: 300 }, false),
            (Level { seq: 2, locate: 2, buy: false, price: 1, qty: 1 }, true),
            (Level { seq: 3, locate: 3, buy: true, price: 0, qty: 0 }, true),
            (Level { seq: u64::MAX / 3, locate: 1, buy: false, price: u32::MAX, qty: u64::MAX / 7 }, true),
        ]
    }

    #[test]
    fn every_format_round_trips_to_the_same_record() {
        let s = syms();
        for (u, existed) in cases() {
            let (mut b, mut f, mut j) = (Vec::new(), Vec::new(), Vec::new());
            encode_bin(&u, 7, &mut b);
            encode_fix(&u, md_action(existed, u.qty), 34_200_000_000_123, &s, &mut f);
            encode_json(&u, 7, &s, &mut j);
            assert_eq!(b.len(), BIN_LEN);
            assert_eq!(decode_bin(&b), Some(u));
            assert_eq!(decode_fix(&f, &s), Ok(u), "{}", String::from_utf8_lossy(&f));
            assert_eq!(decode_json_serde(&j, &s), Ok(u), "{}", String::from_utf8_lossy(&j));
            assert_eq!(decode_json_fixed(&j, &s), Ok(u), "{}", String::from_utf8_lossy(&j));
        }
    }

    #[test]
    fn json_matches_the_documented_shape() {
        let s = syms();
        let mut j = Vec::new();
        encode_json(&Level { seq: 160, locate: 1, buy: false, price: 26_000, qty: 100 }, 1_672_515_782_136, &s, &mut j);
        assert_eq!(std::str::from_utf8(&j).unwrap(), r#"{"e":"depthUpdate","E":1672515782136,"s":"SPY","U":160,"u":160,"b":[],"a":[["2.6000","100"]]}"#);
    }

    #[test]
    fn fix_delete_omits_size_and_validates() {
        let s = syms();
        let u = Level { seq: 9, locate: 2, buy: true, price: 123_400, qty: 0 };
        let mut f = Vec::new();
        encode_fix(&u, md_action(true, 0), 57_600_999_000_000, &s, &mut f);
        let text = String::from_utf8_lossy(&f).replace('\x01', "|");
        assert!(text.starts_with("8=FIX.4.4|9="), "{text}");
        assert!(text.contains("|35=X|49=TITAN|56=CLIENT|34=9|52=20190730-16:00:00.999|268=1|"), "{text}");
        assert!(text.contains("|279=2|269=0|55=BRK.A|270=12.3400|10="), "{text}");
        assert!(!text.contains("271="));
        assert_eq!(decode_fix(&f, &s), Ok(u));
    }

    /// Frames a body with a correct BodyLength and CheckSum.
    fn frame(body: &str) -> Vec<u8> {
        let body = body.replace('|', "\x01");
        let mut m = format!("8=FIX.4.4\x019={}\x01{body}", body.len()).into_bytes();
        let sum = m.iter().fold(0u32, |a, &c| a + c as u32) % 256;
        m.extend_from_slice(format!("10={sum:03}\x01").as_bytes());
        m
    }

    #[test]
    fn fix_structure_is_checked_beyond_the_checksum() {
        let s = syms();
        let head = "35=X|49=TITAN|56=CLIENT|34=5|52=20190730-09:30:00.000|";
        let entry = "268=1|279=0|269=0|55=SPY|270=298.1200|271=300|";
        let u = Level { seq: 5, locate: 1, buy: true, price: 2_981_200, qty: 300 };
        assert_eq!(decode_fix(&frame(&format!("{head}{entry}")), &s), Ok(u));
        use WireError::*;
        let bad = [
            (format!("49=TITAN|35=X|56=CLIENT|34=5|52=20190730-09:30:00.000|{entry}"), Order),
            (format!("35=X|56=CLIENT|34=5|52=20190730-09:30:00.000|{entry}"), Missing),
            (format!("35=X|49=TITAN|56=CLIENT|34=5|{entry}"), Missing),
            (format!("35=X|49=TITAN|56=CLIENT|34=5|52=20190730-25:30:00.000|{entry}"), Value),
            (format!("35=X|49=TITAN|56=CLIENT|34=5|52=20190730-09:30:00|{entry}"), Value),
            (format!("35=X|49=|56=CLIENT|34=5|52=20190730-09:30:00.000|{entry}"), Value),
            (format!("{head}{entry}271=300|"), Duplicate),
            (format!("{head}{entry}58=hello|"), UnknownTag),
            ("35=X|49=TITAN|56=CLIENT|268=1|34=5|52=20190730-09:30:00.000|279=0|269=0|55=SPY|270=298.1200|271=300|".to_string(), Order),
            (format!("{head}268=1|269=0|279=0|55=SPY|270=298.1200|271=300|"), Order),
            (format!("35=W|49=TITAN|56=CLIENT|34=5|52=20190730-09:30:00.000|{entry}"), Value),
        ];
        for (b, why) in &bad {
            assert_eq!(decode_fix(&frame(b), &s), Err(*why), "{b}");
        }
    }

    #[test]
    fn damage_is_rejected_by_every_text_decoder() {
        let s = syms();
        let u = Level { seq: 5, locate: 1, buy: true, price: 2_981_200, qty: 300 };
        let (mut f, mut j) = (Vec::new(), Vec::new());
        encode_fix(&u, b'0', 0, &s, &mut f);
        encode_json(&u, 5, &s, &mut j);
        for i in 0..f.len() {
            let mut g = f.clone();
            g[i] ^= 0x01;
            assert_ne!(decode_fix(&g, &s), Ok(u), "FIX byte {i} flipped was accepted");
        }
        let mut unknown = j.clone();
        let at = unknown.windows(3).position(|w| w == b"SPY").unwrap();
        unknown[at] = b'Q';
        assert_eq!(decode_json_fixed(&unknown, &s), Err(WireError::Symbol));
        assert_eq!(decode_json_serde(&unknown, &s), Err(WireError::Symbol));
        let both = String::from_utf8(j.clone()).unwrap().replace(r#""a":[]"#, r#""a":[["1.0000","1"]]"#);
        assert_eq!(decode_json_fixed(both.as_bytes(), &s), Err(WireError::Fields));
        assert_eq!(decode_json_serde(both.as_bytes(), &s), Err(WireError::Fields));
        assert!(decode_bin(&[0u8; BIN_LEN]).is_none());
    }
}
