//! Deterministic synthetic ITCH 5.0 feed.
//!
//! Produces a valid length-prefixed stream: every execute, cancel, delete and
//! replace refers to a live order with enough shares. Prices follow a random
//! walk per instrument in ITCH fixed point (4 implied decimals, $0.01 ticks);
//! bids rest below and asks above the midpoint. The message mix is a
//! parameter so the cost of each message type can be studied. Offline only:
//! the generator allocates freely.

use crate::Side;
use std::collections::HashMap;

#[derive(Clone, Copy, Debug)]
pub struct Mix {
    pub add: u32,
    pub execute: u32,
    pub execute_price: u32,
    pub cancel: u32,
    pub delete: u32,
    pub replace: u32,
}

impl Default for Mix {
    /// Add-heavy mix with most orders cancelled or deleted rather than
    /// executed, the typical shape of a lit-venue order flow.
    fn default() -> Mix {
        Mix { add: 45, execute: 8, execute_price: 2, cancel: 10, delete: 25, replace: 10 }
    }
}

/// xorshift64*: small, fast, reproducible.
pub struct Rng(u64);

impl Rng {
    pub fn new(seed: u64) -> Rng {
        Rng(seed.max(1))
    }
    pub fn next_u64(&mut self) -> u64 {
        let mut x = self.0;
        x ^= x >> 12;
        x ^= x << 25;
        x ^= x >> 27;
        self.0 = x;
        x.wrapping_mul(0x2545_F491_4F6C_DD1D)
    }
    pub fn below(&mut self, n: u64) -> u64 {
        self.next_u64() % n
    }
}

struct Live {
    locate: u16,
    side: Side,
    shares: u32,
}

pub struct Generator {
    rng: Rng,
    mix: Mix,
    instruments: u16,
    mid: Vec<u32>,
    next_ref: u64,
    match_no: u64,
    ts: u64,
    live: Vec<u64>,
    pos: HashMap<u64, usize>,
    info: HashMap<u64, Live>,
}

impl Generator {
    pub fn new(seed: u64, instruments: u16, mix: Mix) -> Generator {
        let mut rng = Rng::new(seed);
        let mid = (0..=instruments).map(|_| 100_000 + rng.below(5_000_000) as u32 / 100 * 100).collect();
        Generator { rng, mix, instruments, mid, next_ref: 1, match_no: 1, ts: 34_200_000_000_000, live: Vec::new(), pos: HashMap::new(), info: HashMap::new() }
    }

    fn header(out: &mut Vec<u8>, kind: u8, len: u16, locate: u16, ts: u64) {
        out.extend_from_slice(&len.to_be_bytes());
        out.push(kind);
        out.extend_from_slice(&locate.to_be_bytes());
        out.extend_from_slice(&0u16.to_be_bytes());
        out.extend_from_slice(&ts.to_be_bytes()[2..]);
    }

    fn forget(&mut self, order: u64) -> Live {
        let i = self.pos.remove(&order).expect("live order");
        let last = self.live.pop().expect("non-empty");
        if last != order {
            self.live[i] = last;
            self.pos.insert(last, i);
        }
        self.info.remove(&order).expect("info")
    }

    fn remember(&mut self, order: u64, l: Live) {
        self.pos.insert(order, self.live.len());
        self.live.push(order);
        self.info.insert(order, l);
    }

    fn price(&mut self, locate: u16, side: Side) -> u32 {
        let m = &mut self.mid[locate as usize];
        let step = self.rng.below(3) as i64 - 1;
        *m = (*m as i64 + step * 100).max(10_000) as u32;
        let off = (1 + self.rng.below(20)) as u32 * 100;
        match side {
            Side::Buy => m.saturating_sub(off).max(100),
            Side::Sell => *m + off,
        }
    }

    /// Append one message to `out`.
    pub fn step(&mut self, out: &mut Vec<u8>) {
        self.ts += 1 + self.rng.below(2_000);
        let ts = self.ts;
        let total = self.mix.add + self.mix.execute + self.mix.execute_price + self.mix.cancel + self.mix.delete + self.mix.replace;
        let mut r = self.rng.below(total as u64) as u32;
        if self.live.is_empty() {
            r = 0;
        }
        let pick = |g: &mut Generator| g.live[g.rng.below(g.live.len() as u64) as usize];

        if r < self.mix.add {
            let locate = 1 + self.rng.below(self.instruments as u64) as u16;
            let side = if self.rng.below(2) == 0 { Side::Buy } else { Side::Sell };
            let shares = 1 + self.rng.below(1000) as u32;
            let price = self.price(locate, side);
            let order = self.next_ref;
            self.next_ref += 1;
            let mpid = self.rng.below(10) == 0;
            Self::header(out, if mpid { b'F' } else { b'A' }, if mpid { 40 } else { 36 }, locate, ts);
            out.extend_from_slice(&order.to_be_bytes());
            out.push(if side == Side::Buy { b'B' } else { b'S' });
            out.extend_from_slice(&shares.to_be_bytes());
            out.extend_from_slice(b"TITAN   ");
            out.extend_from_slice(&price.to_be_bytes());
            if mpid {
                out.extend_from_slice(b"TTAN");
            }
            self.remember(order, Live { locate, side, shares });
            return;
        }
        r -= self.mix.add;
        let order = pick(self);
        let (locate, have) = {
            let l = &self.info[&order];
            (l.locate, l.shares)
        };
        if r < self.mix.execute + self.mix.execute_price {
            let n = 1 + self.rng.below(have as u64) as u32;
            let with_price = r >= self.mix.execute;
            let m = self.match_no;
            self.match_no += 1;
            Self::header(out, if with_price { b'C' } else { b'E' }, if with_price { 36 } else { 31 }, locate, ts);
            out.extend_from_slice(&order.to_be_bytes());
            out.extend_from_slice(&n.to_be_bytes());
            out.extend_from_slice(&m.to_be_bytes());
            if with_price {
                out.push(b'Y');
                out.extend_from_slice(&self.mid[locate as usize].to_be_bytes());
            }
            self.consume(order, n);
            return;
        }
        r -= self.mix.execute + self.mix.execute_price;
        if r < self.mix.cancel {
            let n = 1 + self.rng.below(have as u64) as u32;
            Self::header(out, b'X', 23, locate, ts);
            out.extend_from_slice(&order.to_be_bytes());
            out.extend_from_slice(&n.to_be_bytes());
            self.consume(order, n);
            return;
        }
        r -= self.mix.cancel;
        if r < self.mix.delete {
            Self::header(out, b'D', 19, locate, ts);
            out.extend_from_slice(&order.to_be_bytes());
            self.forget(order);
            return;
        }
        let l = self.forget(order);
        let new = self.next_ref;
        self.next_ref += 1;
        let shares = 1 + self.rng.below(1000) as u32;
        let price = self.price(locate, l.side);
        Self::header(out, b'U', 35, locate, ts);
        out.extend_from_slice(&order.to_be_bytes());
        out.extend_from_slice(&new.to_be_bytes());
        out.extend_from_slice(&shares.to_be_bytes());
        out.extend_from_slice(&price.to_be_bytes());
        self.remember(new, Live { locate, side: l.side, shares });
    }

    fn consume(&mut self, order: u64, n: u32) {
        let left = {
            let l = self.info.get_mut(&order).expect("live");
            l.shares -= n;
            l.shares
        };
        if left == 0 {
            self.forget(order);
        }
    }

    pub fn live_orders(&self) -> usize {
        self.live.len()
    }
}

/// Generate `messages` messages for `instruments` instruments.
pub fn feed(seed: u64, instruments: u16, messages: usize, mix: Mix) -> Vec<u8> {
    let mut g = Generator::new(seed, instruments, mix);
    let mut out = Vec::with_capacity(messages * 38);
    for _ in 0..messages {
        g.step(&mut out);
    }
    out
}
