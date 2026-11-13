//! MoldUDP64 framing (NASDAQ MoldUDP64 Protocol Specification v1.00):
//! downstream packet header of session (10 bytes), sequence number of the
//! first message (8 bytes, big-endian) and message count (2 bytes,
//! big-endian), followed by message blocks of a 2-byte big-endian length
//! and the message. A count of 0 is a heartbeat, 0xFFFF end of session.

pub const HEADER: usize = 20;
pub const HEARTBEAT: u16 = 0;
pub const END_OF_SESSION: u16 = 0xFFFF;

/// Writes a packet holding `msgs` into `buf`; returns its length, or `None`
/// if it does not fit or a message is longer than 65,535 bytes.
pub fn packet(buf: &mut [u8], session: &[u8; 10], seq: u64, msgs: &[&[u8]]) -> Option<usize> {
    let count = u16::try_from(msgs.len()).ok().filter(|&c| c != END_OF_SESSION)?;
    let need = HEADER + msgs.iter().map(|m| 2 + m.len()).sum::<usize>();
    if need > buf.len() {
        return None;
    }
    header(buf, session, seq, count);
    let mut at = HEADER;
    for m in msgs {
        buf[at..at + 2].copy_from_slice(&u16::try_from(m.len()).ok()?.to_be_bytes());
        buf[at + 2..at + 2 + m.len()].copy_from_slice(m);
        at += 2 + m.len();
    }
    Some(at)
}

/// Header only: heartbeat (`count` 0) or end of session (`count` 0xFFFF);
/// `seq` is the next expected sequence number.
pub fn header(buf: &mut [u8], session: &[u8; 10], seq: u64, count: u16) {
    buf[..10].copy_from_slice(session);
    buf[10..18].copy_from_slice(&seq.to_be_bytes());
    buf[18..20].copy_from_slice(&count.to_be_bytes());
}

/// A parsed downstream packet.
#[derive(Debug, PartialEq, Eq)]
pub struct Packet<'a> {
    pub session: &'a [u8],
    pub seq: u64,
    pub count: u16,
    blocks: &'a [u8],
}

impl<'a> Packet<'a> {
    pub fn parse(p: &'a [u8]) -> Option<Self> {
        if p.len() < HEADER {
            return None;
        }
        Some(Packet {
            session: &p[..10],
            seq: u64::from_be_bytes(p[10..18].try_into().ok()?),
            count: u16::from_be_bytes([p[18], p[19]]),
            blocks: &p[HEADER..],
        })
    }

    /// The message blocks; stops at the first truncated block.
    pub fn messages(&self) -> Messages<'a> {
        let n = if self.count == END_OF_SESSION { 0 } else { self.count };
        Messages { rest: self.blocks, left: n }
    }
}

pub struct Messages<'a> {
    rest: &'a [u8],
    left: u16,
}

impl<'a> Iterator for Messages<'a> {
    type Item = &'a [u8];
    fn next(&mut self) -> Option<&'a [u8]> {
        if self.left == 0 || self.rest.len() < 2 {
            return None;
        }
        let n = u16::from_be_bytes([self.rest[0], self.rest[1]]) as usize;
        let m = self.rest.get(2..2 + n)?;
        self.rest = &self.rest[2 + n..];
        self.left -= 1;
        Some(m)
    }
}

/// Sequence tracking on the receive side.
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
pub struct Gaps {
    /// Next expected sequence number (first message is 1).
    pub expected: u64,
    /// Messages skipped over by a later packet.
    pub missing: u64,
    /// Packets that were not new (duplicates or late).
    pub stale: u64,
}

impl Gaps {
    pub fn new() -> Self {
        Gaps { expected: 1, missing: 0, stale: 0 }
    }

    /// Account for a packet carrying `count` messages from `seq`; returns
    /// false if the packet is stale and should be ignored.
    pub fn on(&mut self, seq: u64, count: u16) -> bool {
        if seq < self.expected {
            self.stale += 1;
            return false;
        }
        self.missing += seq - self.expected;
        self.expected = seq + count as u64;
        true
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn round_trip() {
        let mut b = [0u8; 256];
        let msgs: [&[u8]; 3] = [b"A123", b"", b"xyz"];
        let n = packet(&mut b, b"SESSION001", 42, &msgs).unwrap();
        assert_eq!(n, HEADER + 6 + 2 + 5);
        let p = Packet::parse(&b[..n]).unwrap();
        assert_eq!((p.session, p.seq, p.count), (&b"SESSION001"[..], 42, 3));
        assert_eq!(p.messages().collect::<Vec<_>>(), msgs.to_vec());
        // Truncated packet: the last block is dropped, not misread.
        assert_eq!(Packet::parse(&b[..n - 1]).unwrap().messages().count(), 2);
        // Does not fit.
        assert_eq!(packet(&mut b[..24], b"SESSION001", 1, &msgs), None);
        // End of session carries no messages.
        header(&mut b, b"SESSION001", 9, END_OF_SESSION);
        let p = Packet::parse(&b[..HEADER]).unwrap();
        assert_eq!((p.count, p.messages().count()), (END_OF_SESSION, 0));
    }

    #[test]
    fn gaps_count_drops_and_stale_packets() {
        let mut g = Gaps::new();
        let mut dropped = 0;
        for seq in 1..=10_000u64 {
            if seq % 1_000 == 500 {
                dropped += 1;
                continue;
            }
            assert!(g.on(seq, 1));
        }
        assert_eq!(g.missing, dropped);
        assert_eq!(g.expected, 10_001);
        assert!(!g.on(5, 1));
        assert_eq!(g.stale, 1);
        // A multi-message packet advances by its count.
        assert!(g.on(10_001, 5));
        assert_eq!(g.expected, 10_006);
    }
}
