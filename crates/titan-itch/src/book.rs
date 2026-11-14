//! Zero-allocation (after warm-up) L3 book for all instruments of a feed.
//!
//! Orders: open-addressed table in a pinned region, keyed by order reference.
//! Slot index is the upper bits of `order * 0x9E3779B97F4A7C15` (Fibonacci
//! hashing); collisions probe linearly; deletion shifts later entries back so
//! no tombstones accumulate. Reference 0 marks an empty slot (ITCH order
//! references start at 1).
//!
//! Levels: per instrument and side, a vector of levels sorted so the best
//! level is last. Bids sort by price, asks by inverted price. Levels are
//! located by binary search; on wide synthetic books this was 30 to 44%
//! faster per message than scanning from the best end.
//! Vectors grow only when an instrument reaches a new maximum depth; replaying
//! the same feed again after [`L3Book::clear`] allocates nothing.

use crate::decode::Msg;
use crate::{Book, Side, Top};
use titan_platform::mem::PinnedRegion;

#[derive(Clone, Copy, Default)]
#[repr(C)]
struct Slot {
    order: u64,
    price: u32,
    shares: u32,
    locate: u16,
    buy: bool,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Level {
    /// Sort key: price for bids, `u32::MAX - price` for asks.
    key: u32,
    pub shares: u64,
    pub orders: u32,
}

impl Level {
    fn price(&self, buy: bool) -> u32 {
        if buy { self.key } else { u32::MAX - self.key }
    }
}

pub struct L3Book {
    slots: *mut Slot,
    mask: usize,
    shift: u32,
    live: usize,
    _region: PinnedRegion,
    /// `levels[locate][0]` bids, `[1]` asks.
    levels: Vec<[Vec<Level>; 2]>,
    /// Messages referring to unknown orders, adds with reference 0 and adds for
    /// a locate beyond the instrument count (all 0 on a complete, valid feed).
    pub unknown: u64,
}

unsafe impl Send for L3Book {}

#[inline(always)]
fn key_of(price: u32, buy: bool) -> u32 {
    if buy { price } else { u32::MAX - price }
}

impl L3Book {
    /// `log2_capacity` bounds the number of simultaneously live orders
    /// (kept below 70% load). `instruments` is one more than the highest
    /// stock locate in the feed.
    pub fn new(log2_capacity: u32, instruments: usize) -> L3Book {
        let cap = 1usize << log2_capacity;
        let region = PinnedRegion::new(cap * std::mem::size_of::<Slot>()).expect("order table");
        let mut levels = Vec::with_capacity(instruments);
        levels.resize_with(instruments, || [Vec::new(), Vec::new()]);
        L3Book {
            slots: region.as_ptr() as *mut Slot,
            mask: cap - 1,
            shift: 64 - log2_capacity,
            live: 0,
            _region: region,
            levels,
            unknown: 0,
        }
    }

    /// Shares already resting at `price` on one side, 0 if no order rests
    /// there. Under price-time priority this is the number of shares ahead of
    /// an order arriving at that price, so it is that order's queue position.
    /// The level is found by the same binary search an add performs.
    #[inline(always)]
    pub fn level_shares(&self, locate: u16, side: Side, price: u32) -> u64 {
        let buy = side == Side::Buy;
        let Some(levels) = self.levels.get(locate as usize) else { return 0 };
        let v = &levels[(!buy) as usize];
        let key = key_of(price, buy);
        let i = v.partition_point(|l| l.key <= key);
        if i > 0 && v[i - 1].key == key { v[i - 1].shares } else { 0 }
    }

    /// Cost of filling `shares` immediately against the orders resting on one
    /// side, as the sum over levels of price times the shares taken there,
    /// with the number of levels visited and the price of the last level
    /// consumed. `None` when the side does not hold that many shares. The
    /// side named is the side of the resting orders consumed, so the cost of
    /// buying is a walk of `Side::Sell`.
    ///
    /// The cost answers what the size is paid for on average; the last price
    /// answers how far the quote has to move to fill it. They differ, and a
    /// claim about price impact is a claim about one of them, not both.
    ///
    /// Cost cannot overflow: a walk takes at most `shares` in total, and
    /// `shares` times the largest representable price stays well inside
    /// `u64` for any size this feed can express.
    #[inline(always)]
    pub fn walk(&self, locate: u16, side: Side, shares: u64) -> Option<(u64, u32, u32)> {
        if shares == 0 {
            return Some((0, 0, 0));
        }
        let buy = side == Side::Buy;
        let levels = self.levels.get(locate as usize)?;
        let (mut left, mut cost, mut n) = (shares, 0u64, 0u32);
        for l in levels[(!buy) as usize].iter().rev() {
            let price = l.price(buy);
            let take = l.shares.min(left);
            cost += take * price as u64;
            n += 1;
            left -= take;
            if left == 0 {
                return Some((cost, n, price));
            }
        }
        None
    }

    /// Best price on one side whose level holds at least `min_shares`,
    /// with that level's shares. Thinner levels nearer the top are skipped.
    /// At `min_shares` of 1 this is `best`; at 100 it is a proxy for a
    /// protected quotation, since an odd lot is not one.
    #[inline(always)]
    pub fn best_sized(&self, locate: u16, side: Side, min_shares: u64) -> Top {
        let buy = side == Side::Buy;
        let levels = self.levels.get(locate as usize)?;
        levels[(!buy) as usize].iter().rev().find(|l| l.shares >= min_shares).map(|l| (l.price(buy), l.shares))
    }

    /// Remove all orders and levels, keeping every allocation.
    pub fn clear(&mut self) {
        unsafe { std::ptr::write_bytes(self.slots, 0, self.mask + 1) };
        self.live = 0;
        self.unknown = 0;
        for l in &mut self.levels {
            l[0].clear();
            l[1].clear();
        }
    }

    #[inline(always)]
    fn home(&self, order: u64) -> usize {
        (order.wrapping_mul(0x9E37_79B9_7F4A_7C15) >> self.shift) as usize
    }

    #[inline(always)]
    fn slot(&self, i: usize) -> &Slot {
        debug_assert!(i <= self.mask);
        unsafe { &*self.slots.add(i) }
    }

    #[inline(always)]
    fn slot_mut(&mut self, i: usize) -> &mut Slot {
        debug_assert!(i <= self.mask);
        unsafe { &mut *self.slots.add(i) }
    }

    /// Side and price of a live order, read-only (the aggressor of an
    /// execution is on the other side).
    pub fn order_info(&self, order: u64) -> Option<(Side, u32)> {
        self.find(order).map(|i| {
            let s = self.slot(i);
            (if s.buy { Side::Buy } else { Side::Sell }, s.price)
        })
    }

    #[inline(always)]
    fn find(&self, order: u64) -> Option<usize> {
        // Reference 0 is the empty-slot marker and never names a live order.
        if order == 0 {
            return None;
        }
        let mut i = self.home(order);
        loop {
            let s = self.slot(i);
            if s.order == order {
                return Some(i);
            }
            if s.order == 0 {
                return None;
            }
            i = (i + 1) & self.mask;
        }
    }

    #[inline(always)]
    fn insert_order(&mut self, s: Slot) {
        debug_assert!(s.order != 0);
        assert!(self.live * 10 < (self.mask + 1) * 7, "order table above 70% load");
        let mut i = self.home(s.order);
        while self.slot(i).order != 0 {
            i = (i + 1) & self.mask;
        }
        *self.slot_mut(i) = s;
        self.live += 1;
    }

    /// Backward-shift deletion for linear probing.
    #[inline(always)]
    fn remove_at(&mut self, mut i: usize) {
        let mut j = i;
        loop {
            j = (j + 1) & self.mask;
            let sj = *self.slot(j);
            if sj.order == 0 {
                break;
            }
            let k = self.home(sj.order);
            // Move sj back to i unless its home lies cyclically in (i, j].
            let stays = if i <= j { i < k && k <= j } else { i < k || k <= j };
            if !stays {
                *self.slot_mut(i) = sj;
                i = j;
            }
        }
        self.slot_mut(i).order = 0;
        self.live -= 1;
    }

    #[inline(always)]
    fn level_add(&mut self, locate: u16, buy: bool, price: u32, shares: u32) {
        let v = &mut self.levels[locate as usize][(!buy) as usize];
        let key = key_of(price, buy);
        let i = v.partition_point(|l| l.key <= key);
        if i > 0 && v[i - 1].key == key {
            v[i - 1].shares += shares as u64;
            v[i - 1].orders += 1;
        } else {
            v.insert(i, Level { key, shares: shares as u64, orders: 1 });
        }
    }

    #[inline(always)]
    fn level_reduce(&mut self, locate: u16, buy: bool, price: u32, shares: u32, order_gone: bool) {
        let v = &mut self.levels[locate as usize][(!buy) as usize];
        let key = key_of(price, buy);
        let i = v.partition_point(|l| l.key <= key);
        debug_assert!(i > 0 && v[i - 1].key == key, "level missing");
        let l = &mut v[i - 1];
        l.shares -= shares as u64;
        if order_gone {
            l.orders -= 1;
        }
        if l.orders == 0 {
            v.remove(i - 1);
        }
    }

    /// Take `shares` from an order; remove it when nothing is left.
    #[inline(always)]
    fn take(&mut self, order: u64, shares: Option<u32>) {
        let Some(i) = self.find(order) else {
            self.unknown += 1;
            return;
        };
        let s = *self.slot(i);
        let n = shares.unwrap_or(s.shares).min(s.shares);
        let gone = n == s.shares;
        self.level_reduce(s.locate, s.buy, s.price, n, gone);
        if gone {
            self.remove_at(i);
        } else {
            self.slot_mut(i).shares -= n;
        }
    }

    #[inline(always)]
    fn add(&mut self, locate: u16, order: u64, buy: bool, shares: u32, price: u32) {
        // Reference 0 marks an empty slot and a locate beyond the instrument
        // count has no level vectors: neither occurs in a valid feed, and both
        // are counted and dropped rather than corrupting the table.
        if order == 0 || locate as usize >= self.levels.len() {
            self.unknown += 1;
            return;
        }
        self.insert_order(Slot { order, price, shares, locate, buy });
        self.level_add(locate, buy, price, shares);
    }
}

impl Book for L3Book {
    #[inline(always)]
    fn apply(&mut self, m: &Msg) {
        match *m {
            Msg::Add { locate, order, side, shares, price, .. } => self.add(locate, order, side == Side::Buy, shares, price),
            Msg::Executed { order, shares, .. } | Msg::ExecutedWithPrice { order, shares, .. } | Msg::Cancel { order, shares, .. } => {
                self.take(order, Some(shares))
            }
            Msg::Delete { order, .. } => self.take(order, None),
            Msg::Replace { old, new, shares, price, .. } => {
                let Some(i) = self.find(old) else {
                    self.unknown += 1;
                    return;
                };
                let s = *self.slot(i);
                self.level_reduce(s.locate, s.buy, s.price, s.shares, true);
                self.remove_at(i);
                self.add(s.locate, new, s.buy, shares, price);
            }
            Msg::Other { .. } => {}
        }
    }

    #[inline(always)]
    fn best(&self, locate: u16, side: Side) -> Top {
        let buy = side == Side::Buy;
        self.levels.get(locate as usize)?[(!buy) as usize].last().map(|l| (l.price(buy), l.shares))
    }

    fn depth(&self, locate: u16, side: Side) -> Vec<(u32, u64, u32)> {
        let buy = side == Side::Buy;
        match self.levels.get(locate as usize) {
            Some(l) => l[(!buy) as usize].iter().rev().map(|l| (l.price(buy), l.shares, l.orders)).collect(),
            None => Vec::new(),
        }
    }

    fn live_orders(&self) -> usize {
        self.live
    }
}
