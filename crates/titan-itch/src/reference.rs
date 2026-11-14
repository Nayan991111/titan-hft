//! Correctness oracle: the obvious book built from standard collections.
//! Slow and allocation-heavy by design; never used on a measured path.

use crate::decode::Msg;
use crate::{Book, Side, Top};
use std::collections::{BTreeMap, HashMap};

struct Order {
    locate: u16,
    side: Side,
    price: u32,
    shares: u32,
}

#[derive(Default)]
pub struct RefBook {
    orders: HashMap<u64, Order>,
    /// (locate, side) -> price -> (shares, orders)
    levels: HashMap<(u16, bool), BTreeMap<u32, (u64, u32)>>,
    pub unknown: u64,
}

impl RefBook {
    /// Total shares resting at one price level (0 if the level is empty).
    pub fn level_shares(&self, locate: u16, side: Side, price: u32) -> u64 {
        self.levels.get(&(locate, side == Side::Buy)).and_then(|l| l.get(&price)).map_or(0, |e| e.0)
    }

    /// Naive equivalent of `L3Book::walk`: the cost of filling `shares`
    /// against one side, with the levels visited, built by sorting the whole
    /// side rather than by walking a maintained order.
    pub fn walk(&self, locate: u16, side: Side, shares: u64) -> Option<(u64, u32, u32)> {
        if shares == 0 {
            return Some((0, 0, 0));
        }
        let l = self.levels.get(&(locate, side == Side::Buy))?;
        let mut v: Vec<(u32, u64)> = l.iter().map(|(&p, &(s, _))| (p, s)).collect();
        // Bids are consumed from the highest price down, asks from the lowest up.
        if side == Side::Buy {
            v.reverse();
        }
        let (mut left, mut cost, mut n) = (shares, 0u64, 0u32);
        for (p, s) in v {
            let take = s.min(left);
            cost += take * p as u64;
            n += 1;
            left -= take;
            if left == 0 {
                return Some((cost, n, p));
            }
        }
        None
    }

    /// Instrument, side and price of a live order.
    pub fn order_level(&self, order: u64) -> Option<(u16, Side, u32)> {
        self.orders.get(&order).map(|o| (o.locate, o.side, o.price))
    }

    fn level(&mut self, locate: u16, side: Side) -> &mut BTreeMap<u32, (u64, u32)> {
        self.levels.entry((locate, side == Side::Buy)).or_default()
    }

    fn add(&mut self, locate: u16, order: u64, side: Side, shares: u32, price: u32) {
        let e = self.level(locate, side).entry(price).or_insert((0, 0));
        e.0 += shares as u64;
        e.1 += 1;
        self.orders.insert(order, Order { locate, side, price, shares });
    }

    fn take(&mut self, order: u64, shares: Option<u32>) {
        let Some(o) = self.orders.get_mut(&order) else {
            self.unknown += 1;
            return;
        };
        let n = shares.unwrap_or(o.shares).min(o.shares);
        o.shares -= n;
        let (locate, side, price, gone) = (o.locate, o.side, o.price, o.shares == 0);
        if gone {
            self.orders.remove(&order);
        }
        let lv = self.level(locate, side);
        let e = lv.get_mut(&price).expect("level");
        e.0 -= n as u64;
        if gone {
            e.1 -= 1;
        }
        if e.1 == 0 {
            lv.remove(&price);
        }
    }
}

impl Book for RefBook {
    fn apply(&mut self, m: &Msg) {
        match *m {
            Msg::Add { locate, order, side, shares, price, .. } => self.add(locate, order, side, shares, price),
            Msg::Executed { order, shares, .. } | Msg::ExecutedWithPrice { order, shares, .. } | Msg::Cancel { order, shares, .. } => {
                self.take(order, Some(shares))
            }
            Msg::Delete { order, .. } => self.take(order, None),
            Msg::Replace { old, new, shares, price, .. } => {
                let Some(o) = self.orders.get(&old) else {
                    self.unknown += 1;
                    return;
                };
                let (locate, side) = (o.locate, o.side);
                self.take(old, None);
                self.add(locate, new, side, shares, price);
            }
            Msg::Other { .. } => {}
        }
    }

    fn best(&self, locate: u16, side: Side) -> Top {
        let lv = self.levels.get(&(locate, side == Side::Buy))?;
        let (p, (s, _)) = if side == Side::Buy { lv.iter().next_back()? } else { lv.iter().next()? };
        Some((*p, *s))
    }

    fn depth(&self, locate: u16, side: Side) -> Vec<(u32, u64, u32)> {
        let Some(lv) = self.levels.get(&(locate, side == Side::Buy)) else { return Vec::new() };
        let it = lv.iter().map(|(p, (s, n))| (*p, *s, *n));
        if side == Side::Buy { it.rev().collect() } else { it.collect() }
    }

    fn live_orders(&self) -> usize {
        self.orders.len()
    }
}
