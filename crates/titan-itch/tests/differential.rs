//! The fast book must agree with the reference book after every message:
//! same best bid and ask (price and shares) on the instrument touched, and the
//! same full depth on every instrument at the end.

use titan_itch::book::L3Book;
use titan_itch::decode::{decode, Msg, Reader};
use titan_itch::gen::{feed, Mix};
use titan_itch::reference::RefBook;
use titan_itch::{Book, Side};

fn locate_of(m: &Msg) -> Option<u16> {
    match *m {
        Msg::Add { locate, .. }
        | Msg::Executed { locate, .. }
        | Msg::ExecutedWithPrice { locate, .. }
        | Msg::Cancel { locate, .. }
        | Msg::Delete { locate, .. }
        | Msg::Replace { locate, .. } => Some(locate),
        Msg::Other { .. } => None,
    }
}

fn run(seed: u64, instruments: u16, messages: usize, mix: Mix) {
    let buf = feed(seed, instruments, messages, mix);
    let mut fast = L3Book::new(20, instruments as usize + 1);
    let mut slow = RefBook::default();
    let mut r = Reader::new(&buf);
    let mut n = 0usize;
    while let Some(m) = r.next_msg() {
        let m = m.expect("well-formed feed");
        fast.apply(&m);
        slow.apply(&m);
        if let Some(loc) = locate_of(&m) {
            for side in [Side::Buy, Side::Sell] {
                assert_eq!(fast.best(loc, side), slow.best(loc, side), "top of book differs after message {n} ({m:?}), side {side:?}");
            }
        }
        n += 1;
    }
    assert_eq!(n, messages);
    assert_eq!(fast.unknown, 0);
    assert_eq!(slow.unknown, 0);
    assert_eq!(fast.live_orders(), slow.live_orders());
    for loc in 0..=instruments {
        for side in [Side::Buy, Side::Sell] {
            assert_eq!(fast.depth(loc, side), slow.depth(loc, side), "depth differs for locate {loc} {side:?}");
        }
    }
}

#[test]
fn default_mix_many_instruments() {
    run(7, 50, 400_000, Mix::default());
}

#[test]
fn few_instruments_deep_books() {
    run(11, 2, 200_000, Mix { add: 60, execute: 5, execute_price: 1, cancel: 10, delete: 14, replace: 10 });
}

#[test]
fn replace_and_execute_heavy() {
    run(13, 10, 200_000, Mix { add: 35, execute: 20, execute_price: 5, cancel: 10, delete: 5, replace: 25 });
}

#[test]
fn clear_then_replay_matches() {
    let buf = feed(17, 20, 100_000, Mix::default());
    let mut book = L3Book::new(18, 21);
    let mut snapshot = |b: &mut L3Book| {
        let mut r = Reader::new(&buf);
        while let Some(m) = r.next_msg() {
            b.apply(&m.unwrap());
        }
        (0..=20u16).map(|l| (b.depth(l, Side::Buy), b.depth(l, Side::Sell))).collect::<Vec<_>>()
    };
    let first = snapshot(&mut book);
    book.clear();
    assert_eq!(book.live_orders(), 0);
    assert_eq!(snapshot(&mut book), first);
}

#[test]
fn decoder_field_offsets() {
    // Add Order 'A': locate 7, ts 0x0102_0304_0506, ref 42, Buy, 300 shares, $12.3400.
    let mut m = vec![b'A', 0, 7, 0, 0, 1, 2, 3, 4, 5, 6];
    m.extend_from_slice(&42u64.to_be_bytes());
    m.push(b'B');
    m.extend_from_slice(&300u32.to_be_bytes());
    m.extend_from_slice(b"AAPL    ");
    m.extend_from_slice(&123_400u32.to_be_bytes());
    assert_eq!(m.len(), 36);
    assert_eq!(
        decode(&m),
        Ok(Msg::Add { locate: 7, ts: 0x0102_0304_0506, order: 42, side: Side::Buy, shares: 300, price: 123_400 })
    );
    // Replace 'U': old 42 -> new 43, 200 shares at $12.3500.
    let mut u = vec![b'U', 0, 7, 0, 0, 0, 0, 0, 0, 0, 9];
    u.extend_from_slice(&42u64.to_be_bytes());
    u.extend_from_slice(&43u64.to_be_bytes());
    u.extend_from_slice(&200u32.to_be_bytes());
    u.extend_from_slice(&123_500u32.to_be_bytes());
    assert_eq!(u.len(), 35);
    assert_eq!(decode(&u), Ok(Msg::Replace { locate: 7, ts: 9, old: 42, new: 43, shares: 200, price: 123_500 }));
    // Wrong length is rejected, unknown types pass through.
    assert_eq!(decode(&u[..34]), Err(b'U'));
    assert_eq!(decode(b"S\0\0\0\0\0\0\0\0\0\0O"), Ok(Msg::Other { kind: b'S' }));
}

/// Order table sized so peak occupancy is 50 to 70%: long probe chains, so
/// lookups across collisions and backward-shift deletion are exercised.
#[test]
fn order_table_near_capacity() {
    let mix = Mix { add: 40, execute: 6, execute_price: 2, cancel: 10, delete: 32, replace: 10 };
    let instruments = 30u16;
    let buf = feed(23, instruments, 600_000, mix);
    let mut slow = RefBook::default();
    let mut peak = 0usize;
    let mut r = Reader::new(&buf);
    while let Some(m) = r.next_msg() {
        slow.apply(&m.unwrap());
        peak = peak.max(slow.live_orders());
    }
    let mut log2 = 4u32;
    while ((1usize << log2) * 7) / 10 < peak {
        log2 += 1;
    }
    let load = peak as f64 / (1usize << log2) as f64;
    assert!(load > 0.34, "test must run at high occupancy, got {load:.2}");
    let mut fast = L3Book::new(log2, instruments as usize + 1);
    let mut slow = RefBook::default();
    let mut r = Reader::new(&buf);
    let mut n = 0usize;
    while let Some(m) = r.next_msg() {
        let m = m.unwrap();
        fast.apply(&m);
        slow.apply(&m);
        if let Some(loc) = locate_of(&m) {
            for side in [Side::Buy, Side::Sell] {
                assert_eq!(fast.best(loc, side), slow.best(loc, side), "message {n} at peak load {load:.2}");
            }
        }
        n += 1;
    }
    assert_eq!(fast.unknown, 0, "orders lost in the hash table");
    assert_eq!(fast.live_orders(), slow.live_orders());
}
