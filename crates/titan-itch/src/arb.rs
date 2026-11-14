//! A/B line arbitration and gap recovery for a MoldUDP64 stream.
//!
//! A feed handler subscribed to two copies of the same multicast stream (an
//! A line and a B line) takes each message from whichever line delivers it
//! first and drops the other copy. A message lost on every line is asked for
//! from the re-request server with a MoldUDP64 Request Packet (session, first
//! requested sequence number, requested message count). The answer is a
//! standard downstream packet unicast back to the requester, holding only the
//! requested messages that completely fit in one packet; the rest must be
//! requested again (NASDAQ MoldUDP64 Protocol Specification v1.00). Packets
//! that arrive ahead of a gap are held and released in sequence order once
//! the gap is filled.
//!
//! Rules used here: each line delivers its own packets in order (no
//! reordering within a line); a message counts as lost on a line once that
//! line has delivered anything beyond it (a later packet, a heartbeat or end
//! of session, which carry the next expected sequence number); a
//! retransmission is requested only when every subscribed line has passed
//! the gap, at most one request outstanding at a time.

use crate::mold::{Packet, END_OF_SESSION, HEADER, HEARTBEAT};
use std::collections::VecDeque;

/// The largest UDP payload in one 1500-byte Ethernet frame over IPv4
/// (1500 minus 20 bytes of IPv4 header and 8 of UDP header).
pub const MAX_PAYLOAD: usize = 1472;

/// Messages back to back; message `seq` (from 1) is `get(seq)`.
#[derive(Default)]
pub struct Store {
    bytes: Vec<u8>,
    off: Vec<u32>,
}

impl Store {
    pub fn new() -> Store {
        Store { bytes: Vec::new(), off: vec![0] }
    }
    pub fn push(&mut self, m: &[u8]) {
        assert!(m.len() <= u16::MAX as usize);
        self.bytes.extend_from_slice(m);
        assert!(self.bytes.len() < u32::MAX as usize);
        self.off.push(self.bytes.len() as u32);
    }
    /// Number of messages.
    pub fn len(&self) -> usize {
        self.off.len().saturating_sub(1)
    }
    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }
    /// Message with sequence number `seq` (from 1).
    #[inline(always)]
    pub fn get(&self, seq: u64) -> &[u8] {
        let i = (seq - 1) as usize;
        &self.bytes[self.off[i] as usize..self.off[i + 1] as usize]
    }
}

/// Appends one downstream packet holding messages `seq..` of `store`: at
/// most `count` of them, and only as many as completely fit in `payload`
/// bytes. Returns the number of messages it holds (at least one if `count`
/// is at least one, `seq` is in the store and the message fits at all).
pub fn downstream(store: &Store, session: &[u8; 10], seq: u64, count: u16, payload: usize, out: &mut Vec<u8>) -> u16 {
    let start = out.len();
    out.extend_from_slice(session);
    out.extend_from_slice(&seq.to_be_bytes());
    out.extend_from_slice(&[0, 0]);
    let (mut size, mut n) = (HEADER, 0u16);
    while n < count && n < END_OF_SESSION - 1 && seq + (n as u64) <= store.len() as u64 {
        let m = store.get(seq + n as u64);
        if size + 2 + m.len() > payload {
            break;
        }
        out.extend_from_slice(&(m.len() as u16).to_be_bytes());
        out.extend_from_slice(m);
        size += 2 + m.len();
        n += 1;
    }
    out[start + 18..start + 20].copy_from_slice(&n.to_be_bytes());
    n
}

/// A header-only packet: heartbeat or end of session, with the next
/// expected sequence number.
pub fn header_only(session: &[u8; 10], next: u64, count: u16) -> [u8; HEADER] {
    let mut b = [0u8; HEADER];
    crate::mold::header(&mut b, session, next, count);
    b
}

/// Downstream packets back to back.
#[derive(Default)]
pub struct Packets {
    bytes: Vec<u8>,
    off: Vec<u32>,
}

impl Packets {
    /// The whole store as consecutive packets of at most `payload` bytes.
    pub fn of(store: &Store, session: &[u8; 10], payload: usize) -> Packets {
        let mut p = Packets { bytes: Vec::new(), off: vec![0] };
        let mut seq = 1u64;
        while seq <= store.len() as u64 {
            let n = downstream(store, session, seq, u16::MAX, payload, &mut p.bytes);
            assert!(n > 0, "message {seq} does not fit in a packet");
            p.off.push(p.bytes.len() as u32);
            seq += n as u64;
        }
        p
    }
    /// A separate copy at a different address (the other line's buffers).
    pub fn copy(&self) -> Packets {
        Packets { bytes: self.bytes.clone(), off: self.off.clone() }
    }
    pub fn len(&self) -> usize {
        self.off.len() - 1
    }
    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }
    #[inline(always)]
    pub fn get(&self, i: usize) -> &[u8] {
        &self.bytes[self.off[i] as usize..self.off[i + 1] as usize]
    }
}

/// A MoldUDP64 Request Packet's content.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Request {
    pub seq: u64,
    pub count: u16,
}

impl Request {
    /// The 20-byte Request Packet: session, sequence number, count.
    pub fn packet(&self, session: &[u8; 10]) -> [u8; HEADER] {
        header_only(session, self.seq, self.count)
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct ArbStats {
    /// Line packets received (data, heartbeat or end of session).
    pub packets: u64,
    /// Data packets that brought nothing new.
    pub duplicate: u64,
    /// Data packets held because they arrived ahead of a gap.
    pub held: u64,
    /// Most packets held at once.
    pub max_held: u64,
    /// Request Packets sent.
    pub requests: u64,
    /// Messages delivered from retransmissions.
    pub resent: u64,
    /// Retransmissions holding fewer messages than requested.
    pub partial: u64,
    /// Packets of another session, ignored.
    pub foreign: u64,
}

pub struct Arbiter<'a> {
    session: [u8; 10],
    expected: u64,
    high: [u64; 2],
    lines: usize,
    held: VecDeque<(u64, u64, &'a [u8])>,
    outstanding: Option<Request>,
    pub stats: ArbStats,
}

impl<'a> Arbiter<'a> {
    /// `lines` is 1 or 2; `capacity` packets can be held without allocating.
    pub fn new(session: [u8; 10], lines: usize, capacity: usize) -> Arbiter<'a> {
        assert!(lines == 1 || lines == 2);
        Arbiter {
            session,
            expected: 1,
            high: [1, 1],
            lines,
            held: VecDeque::with_capacity(capacity),
            outstanding: None,
            stats: ArbStats::default(),
        }
    }

    /// Next sequence number to deliver.
    pub fn expected(&self) -> u64 {
        self.expected
    }

    pub fn held(&self) -> usize {
        self.held.len()
    }

    /// A packet from line `line` (0 or 1). Messages are passed to `deliver`
    /// in sequence order, each exactly once. Returns a request to send, if
    /// one is now due.
    #[inline(always)]
    pub fn on_line<F: FnMut(&[u8])>(&mut self, line: usize, p: &'a [u8], deliver: &mut F) -> Option<Request> {
        self.stats.packets += 1;
        let Some(pk) = Packet::parse(p) else { return self.next_request() };
        if pk.session != self.session {
            self.stats.foreign += 1;
            return self.next_request();
        }
        let next = match pk.count {
            HEARTBEAT | END_OF_SESSION => pk.seq,
            c => pk.seq + c as u64,
        };
        if next > self.high[line] {
            self.high[line] = next;
        }
        if pk.count != HEARTBEAT && pk.count != END_OF_SESSION {
            if next <= self.expected {
                self.stats.duplicate += 1;
            } else if pk.seq > self.expected {
                self.hold(pk.seq, next, p);
            } else {
                self.take(&pk, deliver);
                self.drain(deliver);
            }
        }
        self.next_request()
    }

    /// A retransmitted packet from the re-request server. It is not held:
    /// a response always starts at or before the expected message.
    #[inline(always)]
    pub fn on_resend<F: FnMut(&[u8])>(&mut self, p: &[u8], deliver: &mut F) -> Option<Request> {
        let asked = self.outstanding.take();
        if let Some(pk) = Packet::parse(p).filter(|pk| pk.session == self.session) {
            if let Some(r) = asked {
                if pk.count < r.count {
                    self.stats.partial += 1;
                }
            }
            if pk.count != HEARTBEAT && pk.count != END_OF_SESSION && pk.seq <= self.expected {
                let before = self.expected;
                self.take(&pk, deliver);
                self.stats.resent += self.expected - before;
                self.drain(deliver);
            }
        } else {
            self.stats.foreign += 1;
        }
        self.next_request()
    }

    #[inline(always)]
    fn take<F: FnMut(&[u8])>(&mut self, pk: &Packet, deliver: &mut F) {
        for (s, m) in (pk.seq..).zip(pk.messages()) {
            if s == self.expected {
                deliver(m);
                self.expected += 1;
            }
        }
    }

    fn hold(&mut self, seq: u64, next: u64, p: &'a [u8]) {
        let at = self.held.partition_point(|h| h.0 < seq);
        if self.held.get(at).is_some_and(|h| h.0 == seq) {
            self.stats.duplicate += 1;
            return;
        }
        self.held.insert(at, (seq, next, p));
        self.stats.held += 1;
        self.stats.max_held = self.stats.max_held.max(self.held.len() as u64);
    }

    #[inline(always)]
    fn drain<F: FnMut(&[u8])>(&mut self, deliver: &mut F) {
        while let Some(&(seq, next, p)) = self.held.front() {
            if next <= self.expected {
                self.held.pop_front();
                continue;
            }
            if seq > self.expected {
                break;
            }
            self.held.pop_front();
            let pk = Packet::parse(p).expect("held packets parsed before");
            self.take(&pk, deliver);
        }
    }

    #[inline(always)]
    fn next_request(&mut self) -> Option<Request> {
        if self.outstanding.is_some() {
            return None;
        }
        let passed = if self.lines == 1 { self.high[0] } else { self.high[0].min(self.high[1]) };
        if passed <= self.expected {
            return None;
        }
        let end = match self.held.front() {
            Some(&(seq, _, _)) => seq.min(passed),
            None => passed,
        };
        let r = Request { seq: self.expected, count: (end - self.expected).min((END_OF_SESSION - 1) as u64) as u16 };
        self.outstanding = Some(r);
        self.stats.requests += 1;
        Some(r)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::gen::Rng;

    const S: &[u8; 10] = b"SESSION001";

    fn store(n: usize) -> Store {
        let mut s = Store::new();
        for i in 0..n {
            let len = 11 + (i * 7) % 40;
            s.push(&(0..len).map(|k| (i * 31 + k) as u8).collect::<Vec<u8>>());
        }
        s
    }

    /// Runs lines with the given per-line loss in parts per million and a
    /// lag of B behind A in packets; the server answers after `rtt` arrivals.
    /// Returns the delivered messages and the statistics.
    fn simulate(st: &Store, payload: usize, lines: usize, loss_ppm: [u64; 2], lag: usize, rtt: usize, seed: u64) -> (Vec<Vec<u8>>, ArbStats) {
        let a = Packets::of(st, S, payload);
        let b = a.copy();
        let eos = header_only(S, st.len() as u64 + 1, END_OF_SESSION);
        let mut rng = Rng::new(seed);
        // (time, line, packet index or MAX for end of session)
        let mut ev: Vec<(usize, usize, usize)> = Vec::new();
        for i in 0..a.len() {
            for (l, ppm) in loss_ppm.iter().enumerate().take(lines) {
                if rng.below(1_000_000) >= *ppm {
                    ev.push((2 * (i + l * lag) + l, l, i));
                }
            }
        }
        for l in 0..lines {
            ev.push((2 * (a.len() + l * lag) + l, l, usize::MAX));
        }
        ev.sort();
        let mut arb = Arbiter::new(*S, lines, 4096);
        let mut got: Vec<Vec<u8>> = Vec::new();
        let mut pending: VecDeque<(usize, Request)> = VecDeque::new();
        let mut buf = Vec::new();
        let mut t = 0usize;
        let mut k = 0usize;
        let mut deliver = |m: &[u8]| got.push(m.to_vec());
        while k < ev.len() || !pending.is_empty() {
            while pending.front().is_some_and(|&(due, _)| due <= t) {
                let (_, r) = pending.pop_front().unwrap();
                buf.clear();
                downstream(st, S, r.seq, r.count, payload, &mut buf);
                if let Some(r) = arb.on_resend(&buf, &mut deliver) {
                    pending.push_back((t + rtt, r));
                }
            }
            if k < ev.len() {
                let (_, l, i) = ev[k];
                let p = if i == usize::MAX { &eos[..] } else if l == 0 { a.get(i) } else { b.get(i) };
                if let Some(r) = arb.on_line(l, p, &mut deliver) {
                    pending.push_back((t + rtt, r));
                }
                k += 1;
            }
            t += 1;
        }
        // Rebind: the packet buffers outlive the arbiter's held references.
        let stats = arb.stats;
        drop(arb);
        (got, stats)
    }

    fn exact(st: &Store, got: &[Vec<u8>]) {
        assert_eq!(got.len(), st.len(), "message count");
        for (i, m) in got.iter().enumerate() {
            assert_eq!(&m[..], st.get(i as u64 + 1), "message {}", i + 1);
        }
    }

    #[test]
    fn packets_respect_the_payload_and_cover_the_store() {
        let st = store(5_000);
        let p = Packets::of(&st, S, MAX_PAYLOAD);
        let mut seq = 1u64;
        for i in 0..p.len() {
            let raw = p.get(i);
            assert!(raw.len() <= MAX_PAYLOAD);
            let pk = Packet::parse(raw).unwrap();
            assert_eq!((pk.session, pk.seq), (&S[..], seq));
            assert_eq!(pk.messages().count(), pk.count as usize);
            for m in pk.messages() {
                assert_eq!(m, st.get(seq));
                seq += 1;
            }
        }
        assert_eq!(seq, 5_001);
    }

    #[test]
    fn a_response_holds_only_what_fits() {
        let st = store(100);
        let mut out = Vec::new();
        let n = downstream(&st, S, 10, 90, 200, &mut out);
        assert!(n > 0 && n < 90 && out.len() <= 200);
        let pk = Packet::parse(&out).unwrap();
        assert_eq!((pk.seq, pk.count), (10, n));
        // Past the end of the store: nothing.
        out.clear();
        assert_eq!(downstream(&st, S, 101, 5, MAX_PAYLOAD, &mut out), 0);
    }

    #[test]
    fn request_packet_layout() {
        let r = Request { seq: 0x0102_0304_0506_0708, count: 0x0A0B };
        let b = r.packet(S);
        assert_eq!(&b[..10], S);
        assert_eq!(&b[10..18], &[1, 2, 3, 4, 5, 6, 7, 8]);
        assert_eq!(&b[18..20], &[0x0A, 0x0B]);
    }

    #[test]
    fn no_loss_drops_every_second_copy() {
        let st = store(3_000);
        let (got, s) = simulate(&st, MAX_PAYLOAD, 2, [0, 0], 3, 50, 1);
        exact(&st, &got);
        let packets = Packets::of(&st, S, MAX_PAYLOAD).len() as u64;
        assert_eq!(s.duplicate, packets);
        assert_eq!((s.held, s.requests, s.resent), (0, 0, 0));
    }

    #[test]
    fn one_line_losses_are_filled_by_the_other_line() {
        let st = store(20_000);
        let (got, s) = simulate(&st, MAX_PAYLOAD, 2, [50_000, 0], 4, 50, 2);
        exact(&st, &got);
        assert!(s.held > 0);
        assert_eq!((s.requests, s.resent), (0, 0), "B never loses, so nothing is requested");
    }

    #[test]
    fn losses_on_both_lines_are_recovered_by_retransmission() {
        let st = store(50_000);
        for seed in 1..=8u64 {
            let (got, s) = simulate(&st, MAX_PAYLOAD, 2, [100_000, 100_000], 4, 60, seed);
            exact(&st, &got);
            assert!(s.requests > 0 && s.resent > 0, "seed {seed}");
        }
        // A single line recovers everything through the server alone.
        let (got, s) = simulate(&st, MAX_PAYLOAD, 1, [50_000, 0], 0, 40, 9);
        exact(&st, &got);
        assert!(s.requests > 0);
    }

    #[test]
    fn partial_responses_are_requested_again() {
        // Small packets on the line, so a lost run spans many messages, and
        // a small response payload, so answers come back partial.
        let st = store(20_000);
        let a = Packets::of(&st, S, MAX_PAYLOAD);
        // Lose packets 10..60 on both lines.
        let mut arb = Arbiter::new(*S, 2, 4096);
        let mut got: Vec<Vec<u8>> = Vec::new();
        let mut deliver = |m: &[u8]| got.push(m.to_vec());
        let mut req = None;
        for i in 0..a.len() {
            if (10..60).contains(&i) {
                continue;
            }
            for l in 0..2 {
                if let Some(r) = arb.on_line(l, a.get(i), &mut deliver) {
                    assert!(req.is_none());
                    req = Some(r);
                }
            }
        }
        let eos = header_only(S, st.len() as u64 + 1, END_OF_SESSION);
        for l in 0..2 {
            assert!(arb.on_line(l, &eos, &mut deliver).is_none());
        }
        let mut rounds = 0;
        let mut buf = Vec::new();
        while let Some(r) = req.take() {
            buf.clear();
            downstream(&st, S, r.seq, r.count, 300, &mut buf);
            req = arb.on_resend(&buf, &mut deliver);
            rounds += 1;
        }
        assert!(arb.stats.partial > 0 && rounds > 1);
        assert_eq!(arb.held(), 0);
        let stats = arb.stats;
        drop(arb);
        exact(&st, &got);
        assert_eq!(stats.requests, rounds);
    }

    #[test]
    fn a_lost_tail_is_found_from_end_of_session() {
        let st = store(2_000);
        let a = Packets::of(&st, S, MAX_PAYLOAD);
        let mut arb = Arbiter::new(*S, 2, 64);
        let mut n = 0usize;
        let mut deliver = |_: &[u8]| n += 1;
        for i in 0..a.len() - 1 {
            for l in 0..2 {
                assert!(arb.on_line(l, a.get(i), &mut deliver).is_none());
            }
        }
        let eos = header_only(S, st.len() as u64 + 1, END_OF_SESSION);
        assert!(arb.on_line(0, &eos, &mut deliver).is_none(), "B has not passed the gap yet");
        let r = arb.on_line(1, &eos, &mut deliver).expect("both lines passed the gap");
        let last = Packet::parse(a.get(a.len() - 1)).unwrap();
        assert_eq!(r, Request { seq: last.seq, count: last.count });
        let mut buf = Vec::new();
        downstream(&st, S, r.seq, r.count, MAX_PAYLOAD, &mut buf);
        assert!(arb.on_resend(&buf, &mut deliver).is_none());
        assert_eq!(arb.expected(), st.len() as u64 + 1);
        drop(arb);
        assert_eq!(n, st.len());
    }

    #[test]
    fn foreign_sessions_and_heartbeats_deliver_nothing() {
        let st = store(500);
        let a = Packets::of(&st, S, MAX_PAYLOAD);
        let mut arb = Arbiter::new(*S, 1, 8);
        let mut n = 0usize;
        let mut deliver = |_: &[u8]| n += 1;
        let other = Packets::of(&st, b"SESSION002", MAX_PAYLOAD);
        assert!(arb.on_line(0, other.get(0), &mut deliver).is_none());
        let hb = header_only(S, 1, HEARTBEAT);
        assert!(arb.on_line(0, &hb, &mut deliver).is_none());
        assert!(arb.on_line(0, a.get(0), &mut deliver).is_none());
        let s = arb.stats;
        let e = arb.expected();
        drop(arb);
        assert_eq!(s.foreign, 1);
        assert_eq!(e as usize, 1 + n);
    }
}
