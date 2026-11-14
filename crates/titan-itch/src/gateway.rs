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

/// The five order fields read back from a NewOrderSingle.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct FixOrder {
    pub buy: bool,
    pub shares: u32,
    /// 4 implied decimals, as ITCH and OUCH.
    pub price: u32,
    /// Space padded, as ITCH.
    pub symbol: [u8; 8],
}

/// Why a full parse rejected a message.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum FixError {
    /// No trailing SOH, a field without '=', a non-numeric tag or too many fields.
    Framing,
    BeginString,
    BodyLength,
    CheckSum,
    /// 35 is not D, or one of 55, 54, 38, 44 is missing or malformed.
    Fields,
}

/// Most fields a message may have for the full parse.
pub const FIX_MAX_FIELDS: usize = 32;

/// The symbol as FIX carries it (padding removed), padded again: what a
/// parse of [`fix42_new_order`] output returns for `symbol`.
pub fn fix_symbol(symbol: &[u8; 8]) -> [u8; 8] {
    let mut out = [b' '; 8];
    let t = trimmed(symbol);
    out[..t.len()].copy_from_slice(t);
    out
}

#[inline(always)]
pub(crate) fn uint(v: &[u8]) -> Option<u64> {
    if v.is_empty() || v.len() > 19 {
        return None;
    }
    let mut x = 0u64;
    for &c in v {
        let d = c.wrapping_sub(b'0');
        if d > 9 {
            return None;
        }
        x = x * 10 + d as u64;
    }
    Some(x)
}

/// "123.4500" to 1,234,500 (up to 4 decimals).
#[inline(always)]
pub(crate) fn price4(v: &[u8]) -> Option<u32> {
    let dot = v.iter().position(|&c| c == b'.').unwrap_or(v.len());
    let int = uint(&v[..dot])?;
    let mut frac = 0u64;
    let mut scale = 10_000u64;
    if dot < v.len() {
        let f = &v[dot + 1..];
        if f.len() > 4 {
            return None;
        }
        frac = if f.is_empty() { 0 } else { uint(f)? };
        for _ in 0..f.len() {
            scale /= 10;
        }
    }
    u32::try_from(int.checked_mul(10_000)?.checked_add(frac * scale)?).ok()
}

#[inline(always)]
pub(crate) fn symbol8(v: &[u8]) -> Option<[u8; 8]> {
    if v.is_empty() || v.len() > 8 {
        return None;
    }
    let mut s = [b' '; 8];
    s[..v.len()].copy_from_slice(v);
    Some(s)
}

/// Positions of every SOH in `m`, one byte at a time. Returns the count, or
/// `usize::MAX` if there are more than [`FIX_MAX_FIELDS`].
#[inline(always)]
pub fn soh_scalar(m: &[u8], out: &mut [u16; FIX_MAX_FIELDS]) -> usize {
    let mut n = 0;
    for (i, &c) in m.iter().enumerate() {
        if c == SOH {
            if n == FIX_MAX_FIELDS {
                return usize::MAX;
            }
            out[n] = i as u16;
            n += 1;
        }
    }
    n
}

/// As [`soh_scalar`], 16 bytes per step with NEON compares (aarch64); the
/// scalar loop elsewhere and for the tail.
#[inline(always)]
pub fn soh_neon(m: &[u8], out: &mut [u16; FIX_MAX_FIELDS]) -> usize {
    #[cfg(target_arch = "aarch64")]
    {
        use core::arch::aarch64::*;
        let mut n = 0usize;
        let mut i = 0usize;
        // SAFETY: every load reads 16 bytes at m[i..i + 16] with i + 16 <= m.len().
        unsafe {
            let soh = vdupq_n_u8(SOH);
            while i + 16 <= m.len() {
                let eq = vceqq_u8(vld1q_u8(m.as_ptr().add(i)), soh);
                // One nibble per input byte: nibble k is 0xF when byte k matched.
                let mut mask = vget_lane_u64::<0>(vreinterpret_u64_u8(vshrn_n_u16::<4>(vreinterpretq_u16_u8(eq))));
                while mask != 0 {
                    let b = (mask.trailing_zeros() / 4) as usize;
                    if n == FIX_MAX_FIELDS {
                        return usize::MAX;
                    }
                    out[n] = (i + b) as u16;
                    n += 1;
                    mask &= !(0xFu64 << (b * 4));
                }
                i += 16;
            }
        }
        for (j, &c) in m[i..].iter().enumerate() {
            if c == SOH {
                if n == FIX_MAX_FIELDS {
                    return usize::MAX;
                }
                out[n] = (i + j) as u16;
                n += 1;
            }
        }
        n
    }
    #[cfg(not(target_arch = "aarch64"))]
    {
        soh_scalar(m, out)
    }
}

/// Full parse given the SOH positions: index every field, validate
/// BeginString, BodyLength and CheckSum, then read the five order fields.
#[inline(always)]
pub fn fix42_parse_indexed(m: &[u8], soh: &[u16]) -> Result<FixOrder, FixError> {
    let n = soh.len();
    if n < 4 || soh[n - 1] as usize != m.len() - 1 {
        return Err(FixError::Framing);
    }
    // (tag, value start, value end) for every field.
    let mut f = [(0u32, 0u16, 0u16); FIX_MAX_FIELDS];
    let mut start = 0usize;
    for (k, &end) in soh.iter().enumerate() {
        let end = end as usize;
        let mut tag = 0u32;
        let mut j = start;
        while j < end && m[j] != b'=' {
            let d = m[j].wrapping_sub(b'0');
            if d > 9 || j - start == 9 {
                return Err(FixError::Framing);
            }
            tag = tag * 10 + d as u32;
            j += 1;
        }
        if j == end || j == start {
            return Err(FixError::Framing);
        }
        f[k] = (tag, (j + 1) as u16, end as u16);
        start = end + 1;
    }
    let val = |k: usize| &m[f[k].1 as usize..f[k].2 as usize];
    if f[0].0 != 8 || val(0) != b"FIX.4.2" {
        return Err(FixError::BeginString);
    }
    if f[1].0 != 9 || f[n - 1].0 != 10 {
        return Err(FixError::Framing);
    }
    let body_start = soh[1] as usize + 1;
    let trailer = soh[n - 2] as usize + 1;
    if uint(val(1)) != Some((trailer - body_start) as u64) {
        return Err(FixError::BodyLength);
    }
    let sum = m[..trailer].iter().fold(0u32, |a, &c| a + c as u32) % 256;
    if val(n - 1).len() != 3 || uint(val(n - 1)) != Some(sum as u64) {
        return Err(FixError::CheckSum);
    }
    let (mut msg_type, mut side, mut shares, mut price, mut symbol) = (None, None, None, None, None);
    for (k, fk) in f.iter().enumerate().take(n - 1).skip(2) {
        let v = val(k);
        match fk.0 {
            35 => msg_type = Some(v == b"D"),
            54 => side = Some(v),
            38 => shares = uint(v),
            44 => price = price4(v),
            55 => symbol = symbol8(v),
            _ => {}
        }
    }
    match (msg_type, side, shares, price, symbol) {
        (Some(true), Some(b"1"), Some(q), Some(p), Some(s)) | (Some(true), Some(b"2"), Some(q), Some(p), Some(s))
            if q <= u32::MAX as u64 =>
        {
            Ok(FixOrder { buy: side == Some(b"1"), shares: q as u32, price: p, symbol: s })
        }
        _ => Err(FixError::Fields),
    }
}

/// Full parse, delimiters found one byte at a time.
#[inline(always)]
pub fn fix42_parse_full(m: &[u8]) -> Result<FixOrder, FixError> {
    let mut soh = [0u16; FIX_MAX_FIELDS];
    let n = soh_scalar(m, &mut soh);
    if n == usize::MAX {
        return Err(FixError::Framing);
    }
    fix42_parse_indexed(m, &soh[..n])
}

/// Full parse, delimiters found with NEON (see [`soh_neon`]).
#[inline(always)]
pub fn fix42_parse_full_neon(m: &[u8]) -> Result<FixOrder, FixError> {
    let mut soh = [0u16; FIX_MAX_FIELDS];
    let n = soh_neon(m, &mut soh);
    if n == usize::MAX {
        return Err(FixError::Framing);
    }
    fix42_parse_indexed(m, &soh[..n])
}

/// Selective parse: walk the fields once, read only 35, 55, 54, 38 and 44,
/// stop when all five are seen. No BodyLength or CheckSum validation.
#[inline(always)]
pub fn fix42_parse_selective(m: &[u8]) -> Option<FixOrder> {
    let (mut seen, mut buy, mut shares, mut price, mut symbol) = (0u8, false, 0u32, 0u32, [b' '; 8]);
    let mut i = 0usize;
    while i < m.len() && seen != 0b11111 {
        let (mut tag, t0) = (0u32, i);
        while i < m.len() && m[i] != b'=' {
            let d = m[i].wrapping_sub(b'0');
            if d > 9 || i - t0 == 9 {
                return None;
            }
            tag = tag * 10 + d as u32;
            i += 1;
        }
        let vs = i + 1;
        let mut ve = vs;
        while ve < m.len() && m[ve] != SOH {
            ve += 1;
        }
        let v = m.get(vs..ve)?;
        match tag {
            35 => {
                if v != b"D" {
                    return None;
                }
                seen |= 1;
            }
            54 => {
                buy = v == b"1";
                seen |= 2;
            }
            38 => {
                shares = u32::try_from(uint(v)?).ok()?;
                seen |= 4;
            }
            44 => {
                price = price4(v)?;
                seen |= 8;
            }
            55 => {
                symbol = symbol8(v)?;
                seen |= 16;
            }
            _ => {}
        }
        i = ve + 1;
    }
    (seen == 0b11111).then_some(FixOrder { buy, shares, price, symbol })
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

    #[test]
    fn fix_parse_round_trip() {
        let mut b = [0u8; FIX_MAX];
        let ts = 34_200_000_000_000u64;
        for (seq, it, sym) in [
            (1u64, intent(true), *b"MSFT    "),
            (77, Intent { price: 5, shares: 1, locate: 3, buy: false }, *b"A       "),
            (u32::MAX as u64, Intent { price: 999_999_999, shares: 4_000_000, locate: 9, buy: true }, *b"ABCDEFGH"),
        ] {
            let n = fix42_new_order(&mut b, &it, &sym, seq, ts, &Session::default());
            let want = FixOrder { buy: it.buy, shares: it.shares, price: it.price, symbol: fix_symbol(&sym) };
            assert_eq!(fix42_parse_full(&b[..n]), Ok(want));
            assert_eq!(fix42_parse_full_neon(&b[..n]), Ok(want));
            assert_eq!(fix42_parse_selective(&b[..n]), Some(want));
            let mut x = [0u16; FIX_MAX_FIELDS];
            let mut y = [0u16; FIX_MAX_FIELDS];
            let (nx, ny) = (soh_scalar(&b[..n], &mut x), soh_neon(&b[..n], &mut y));
            assert_eq!(&x[..nx], &y[..ny]);
        }
    }

    #[test]
    fn fix_full_parse_rejects_damage() {
        let mut b = [0u8; FIX_MAX];
        let n = fix42_new_order(&mut b, &intent(true), b"MSFT    ", 5, 0, &Session::default());
        let good = b;
        // A changed price byte: the checksum no longer matches; the selective
        // parse does not check and returns the wrong price.
        let p = (0..n).find(|&i| b[i..].starts_with(b"44=")).unwrap() + 3;
        b[p] = if b[p] == b'9' { b'8' } else { b'9' };
        assert_eq!(fix42_parse_full(&b[..n]), Err(FixError::CheckSum));
        assert!(fix42_parse_selective(&b[..n]).is_some_and(|o| o.price != 1_234_500));
        // A wrong BodyLength digit.
        let mut c = good;
        c[12] = if c[12] == b'1' { b'2' } else { b'1' };
        assert_eq!(fix42_parse_full(&c[..n]), Err(FixError::BodyLength));
        // Truncated: no trailing SOH.
        assert_eq!(fix42_parse_full(&good[..n - 1]), Err(FixError::Framing));
        // Not FIX.4.2.
        let mut d = good;
        d[8] = b'4';
        assert!(fix42_parse_full(&d[..n]).is_err());
    }
}
