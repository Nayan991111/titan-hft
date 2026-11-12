//! Malformed and adversarial input: the decoder and the L3 book must never
//! panic or lose their invariants on input that a valid feed never contains.
//! The one deliberate panic, an order table above 70% load, is a capacity
//! contract and is not exercised here.

use titan_itch::book::L3Book;
use titan_itch::decode::{decode, Msg, Reader};
use titan_itch::{Book, Side};

const INSTRUMENTS: usize = 64;

struct Rng(u64);
impl Rng {
    fn next_u64(&mut self) -> u64 {
        self.0 ^= self.0 << 13;
        self.0 ^= self.0 >> 7;
        self.0 ^= self.0 << 17;
        self.0
    }
}

fn header(kind: u8, locate: u16) -> Vec<u8> {
    let mut b = vec![kind];
    b.extend_from_slice(&locate.to_be_bytes());
    b.extend_from_slice(&[0, 0]); // tracking number
    b.extend_from_slice(&[0, 0, 0, 0, 0, 1]); // timestamp
    b
}

fn add(locate: u16, order: u64, buy: bool, shares: u32, price: u32) -> Vec<u8> {
    let mut b = header(b'A', locate);
    b.extend_from_slice(&order.to_be_bytes());
    b.push(if buy { b'B' } else { b'S' });
    b.extend_from_slice(&shares.to_be_bytes());
    b.extend_from_slice(b"TEST    ");
    b.extend_from_slice(&price.to_be_bytes());
    b
}

fn exec(locate: u16, order: u64, shares: u32) -> Vec<u8> {
    let mut b = header(b'E', locate);
    b.extend_from_slice(&order.to_be_bytes());
    b.extend_from_slice(&shares.to_be_bytes());
    b.extend_from_slice(&7u64.to_be_bytes());
    b
}

fn delete(locate: u16, order: u64) -> Vec<u8> {
    let mut b = header(b'D', locate);
    b.extend_from_slice(&order.to_be_bytes());
    b
}

fn replace(locate: u16, old: u64, new: u64, shares: u32, price: u32) -> Vec<u8> {
    let mut b = header(b'U', locate);
    b.extend_from_slice(&old.to_be_bytes());
    b.extend_from_slice(&new.to_be_bytes());
    b.extend_from_slice(&shares.to_be_bytes());
    b.extend_from_slice(&price.to_be_bytes());
    b
}

fn apply(book: &mut L3Book, body: &[u8]) {
    if let Ok(m) = decode(body) {
        book.apply(&m);
    }
}

/// Every level on every instrument has positive order count and the book's
/// best prices are consistent with its depth listing.
fn check(book: &L3Book) {
    for loc in 0..INSTRUMENTS as u16 {
        for side in [Side::Buy, Side::Sell] {
            let d = book.depth(loc, side);
            assert!(d.iter().all(|&(_, _, orders)| orders > 0), "empty level kept");
            assert_eq!(book.best(loc, side), d.first().map(|&(p, s, _)| (p, s)));
        }
    }
}

#[test]
fn empty_and_short_bodies() {
    assert_eq!(decode(&[]), Err(0));
    for &kind in b"AFECXDU" {
        for len in 1..34 {
            let mut b = vec![0u8; len];
            b[0] = kind;
            let _ = decode(&b);
        }
    }
    // A zero length prefix and a prefix running past the end are errors.
    assert!(Reader::new(&[0, 0]).next_raw().unwrap().is_err());
    assert!(Reader::new(&[0, 9, b'D']).next_raw().unwrap().is_err());
    assert!(Reader::new(&[0]).next_raw().unwrap().is_err());
}

#[test]
fn adversarial_messages_are_counted_not_fatal() {
    let mut book = L3Book::new(12, INSTRUMENTS);
    let cases = [
        add(1, 0, true, 100, 10_000),         // reference 0 is the empty marker
        add(u16::MAX, 5, true, 100, 10_000),  // locate beyond the instrument count
        add(INSTRUMENTS as u16, 6, false, 100, 10_000),
        delete(1, 0),                         // reference 0 must not match an empty slot
        exec(1, 0, 10),
        replace(1, 0, 7, 10, 10_000),
        delete(1, 999),                       // unknown order
        exec(1, 999, 10),
        replace(1, 999, 1000, 10, 10_000),
    ];
    for c in &cases {
        apply(&mut book, c);
    }
    assert_eq!(book.unknown, cases.len() as u64);
    assert_eq!(book.live_orders(), 0);
    check(&book);

    // Execution larger than the resting size removes the order once.
    apply(&mut book, &add(2, 10, true, 50, 20_000));
    apply(&mut book, &exec(2, 10, 1_000_000));
    assert_eq!(book.live_orders(), 0);
    assert_eq!(book.best(2, Side::Buy), None);

    // Zero-share order, extreme prices, duplicate references.
    apply(&mut book, &add(3, 11, true, 0, 0));
    apply(&mut book, &add(3, 12, false, 1, u32::MAX));
    apply(&mut book, &add(3, 13, true, 5, 1));
    apply(&mut book, &add(3, 13, true, 7, 2));
    check(&book);
    assert_eq!(book.live_orders(), 4);
    apply(&mut book, &delete(3, 13));
    apply(&mut book, &delete(3, 13));
    apply(&mut book, &delete(3, 13));
    check(&book);
    assert_eq!(book.live_orders(), 2);

    // Replace onto reference 0 drops the order and counts it.
    let before = book.unknown;
    apply(&mut book, &replace(3, 12, 0, 1, 5));
    assert_eq!(book.unknown, before + 1);
    assert_eq!(book.best(3, Side::Sell), None);
    check(&book);
}

#[test]
fn random_well_framed_messages() {
    let kinds = *b"AFECXDUPSRZ";
    let mut rng = Rng(0x2545_F491_4F6C_DD1D);
    let mut book = L3Book::new(16, INSTRUMENTS);
    for round in 0..200_000u32 {
        let kind = kinds[(rng.next_u64() % kinds.len() as u64) as usize];
        // Mostly the right length for the type, sometimes not.
        let len = match (titan_itch::decode::book_len(kind), rng.next_u64() % 8) {
            (Some(n), 0) => (n + (rng.next_u64() % 5) as usize).saturating_sub(2).max(1),
            (Some(n), _) => n,
            (None, _) => 1 + (rng.next_u64() % 50) as usize,
        };
        let mut b = vec![0u8; len];
        for x in b.iter_mut() {
            *x = rng.next_u64() as u8;
        }
        b[0] = kind;
        if len > 2 {
            // Keep most locates and order references in a small range so
            // messages hit live orders.
            let loc = (rng.next_u64() % (INSTRUMENTS as u64 + 4)) as u16;
            b[1..3].copy_from_slice(&loc.to_be_bytes());
        }
        if len >= 19 {
            let order = rng.next_u64() % 4096;
            b[11..19].copy_from_slice(&order.to_be_bytes());
        }
        if len >= 27 && kind == b'U' {
            let new = rng.next_u64() % 4096;
            b[19..27].copy_from_slice(&new.to_be_bytes());
        }
        if let Ok(m) = decode(&b) {
            if !matches!(m, Msg::Other { .. }) && book.live_orders() * 10 < (1 << 16) * 6 {
                book.apply(&m);
            }
        }
        if round % 20_000 == 0 {
            check(&book);
        }
    }
    check(&book);
}

#[test]
fn random_bytes_through_the_reader() {
    let mut rng = Rng(0x9E37_79B9_7F4A_7C15);
    let mut book = L3Book::new(14, INSTRUMENTS);
    for _ in 0..200 {
        let n = (rng.next_u64() % 4096) as usize;
        let buf: Vec<u8> = (0..n).map(|_| rng.next_u64() as u8).collect();
        let mut r = Reader::new(&buf);
        while let Some(x) = r.next_msg() {
            match x {
                Ok(m) => book.apply(&m),
                Err(_) => break,
            }
        }
    }
    check(&book);
}
