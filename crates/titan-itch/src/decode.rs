//! ITCH 5.0 message decoding. Field layouts follow the NASDAQ TotalView-ITCH
//! 5.0 specification; every message starts with type (1), stock locate (2),
//! tracking number (2) and a 6-byte nanosecond timestamp. All integers are
//! big-endian. Only messages that change the order book are decoded into
//! fields; the rest are reported by type and skipped.

use crate::Side;

/// Decoded message. Order-book messages carry their fields; others carry only
/// the type byte.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Msg {
    /// 'A' (36 bytes) and 'F' (40 bytes, with attribution).
    Add { locate: u16, ts: u64, order: u64, side: Side, shares: u32, price: u32 },
    /// 'E' (31): execution at the order's price.
    Executed { locate: u16, ts: u64, order: u64, shares: u32 },
    /// 'C' (36): execution with a different price; book effect is the same.
    ExecutedWithPrice { locate: u16, ts: u64, order: u64, shares: u32, price: u32 },
    /// 'X' (23): partial cancel.
    Cancel { locate: u16, ts: u64, order: u64, shares: u32 },
    /// 'D' (19): delete the remaining shares.
    Delete { locate: u16, ts: u64, order: u64 },
    /// 'U' (35): replace; the new order keeps side and instrument.
    Replace { locate: u16, ts: u64, old: u64, new: u64, shares: u32, price: u32 },
    /// Any other message type (system, directory, trades, imbalance...).
    Other { kind: u8 },
}

/// Expected length of each book message type.
pub const fn book_len(kind: u8) -> Option<usize> {
    match kind {
        b'A' => Some(36),
        b'F' => Some(40),
        b'E' => Some(31),
        b'C' => Some(36),
        b'X' => Some(23),
        b'D' => Some(19),
        b'U' => Some(35),
        _ => None,
    }
}

#[inline(always)]
fn u16be(b: &[u8], at: usize) -> u16 {
    u16::from_be_bytes([b[at], b[at + 1]])
}

#[inline(always)]
fn u32be(b: &[u8], at: usize) -> u32 {
    u32::from_be_bytes([b[at], b[at + 1], b[at + 2], b[at + 3]])
}

#[inline(always)]
fn u64be(b: &[u8], at: usize) -> u64 {
    let mut a = [0u8; 8];
    a.copy_from_slice(&b[at..at + 8]);
    u64::from_be_bytes(a)
}

#[inline(always)]
fn ts48(b: &[u8]) -> u64 {
    let mut a = [0u8; 8];
    a[2..].copy_from_slice(&b[5..11]);
    u64::from_be_bytes(a)
}

#[derive(Debug, PartialEq, Eq)]
pub enum DecodeError {
    Truncated { offset: usize },
    BadLength { offset: usize, kind: u8, len: usize },
}

/// Decode one message body (without the length prefix).
#[inline(always)]
pub fn decode(b: &[u8]) -> Result<Msg, u8> {
    let Some(&kind) = b.first() else {
        return Err(0);
    };
    if let Some(n) = book_len(kind) {
        if b.len() != n {
            return Err(kind);
        }
    } else {
        return Ok(Msg::Other { kind });
    }
    let locate = u16be(b, 1);
    let ts = ts48(b);
    Ok(match kind {
        b'A' | b'F' => Msg::Add {
            locate,
            ts,
            order: u64be(b, 11),
            side: if b[19] == b'B' { Side::Buy } else { Side::Sell },
            shares: u32be(b, 20),
            price: u32be(b, 32),
        },
        b'E' => Msg::Executed { locate, ts, order: u64be(b, 11), shares: u32be(b, 19) },
        b'C' => Msg::ExecutedWithPrice { locate, ts, order: u64be(b, 11), shares: u32be(b, 19), price: u32be(b, 32) },
        b'X' => Msg::Cancel { locate, ts, order: u64be(b, 11), shares: u32be(b, 19) },
        b'D' => Msg::Delete { locate, ts, order: u64be(b, 11) },
        _ => Msg::Replace { locate, ts, old: u64be(b, 11), new: u64be(b, 19), shares: u32be(b, 27), price: u32be(b, 31) },
    })
}

/// Iterator over a length-prefixed ITCH buffer. Borrows the buffer; no copies.
pub struct Reader<'a> {
    buf: &'a [u8],
    pos: usize,
}

impl<'a> Reader<'a> {
    pub fn new(buf: &'a [u8]) -> Reader<'a> {
        Reader { buf, pos: 0 }
    }

    /// Next message body, or an error describing malformed input.
    #[inline(always)]
    pub fn next_raw(&mut self) -> Option<Result<&'a [u8], DecodeError>> {
        if self.pos >= self.buf.len() {
            return None;
        }
        if self.pos + 2 > self.buf.len() {
            return Some(Err(DecodeError::Truncated { offset: self.pos }));
        }
        let len = u16be(self.buf, self.pos) as usize;
        let start = self.pos + 2;
        let end = start + len;
        if len == 0 || end > self.buf.len() {
            return Some(Err(DecodeError::Truncated { offset: self.pos }));
        }
        self.pos = end;
        Some(Ok(&self.buf[start..end]))
    }

    /// Next decoded message.
    #[inline(always)]
    pub fn next_msg(&mut self) -> Option<Result<Msg, DecodeError>> {
        let off = self.pos;
        match self.next_raw()? {
            Ok(b) => Some(decode(b).map_err(|kind| DecodeError::BadLength { offset: off, kind, len: b.len() })),
            Err(e) => Some(Err(e)),
        }
    }
}
