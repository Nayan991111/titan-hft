//! The order book as the third source brief specifies it: one binary heap
//! per instrument and side, holding orders rather than aggregated price
//! levels.
//!
//! The brief's design is "Bid book: Priority queue ordered by price (highest
//! first), then time", with "Insert quote: O(log n)", "Remove quote:
//! O(log n)" and "Match order: O(1)". [`L3Book`](crate::book::L3Book)
//! instead keeps one sorted array of price levels per instrument and side,
//! where the best level is the last element and orders at a price are
//! aggregated into it. Both answer "what is the best bid" after every
//! message; this module exists so the two can be measured against each other
//! on the same real feed.
//!
//! A heap of orders cannot delete an arbitrary order by itself, and ITCH
//! cancels and deletes name an order reference, so two policies are offered,
//! and both are measured rather than one being assumed:
//!
//! * [`Policy::Lazy`]: a deleted order is dropped from the order table and
//!   left in the heap; the top is popped while it names an order the table
//!   no longer holds. The usual shortcut, and what a heap alone allows.
//! * [`Policy::Indexed`]: the order table also holds each order's position
//!   in its heap, kept up to date through every sift, so a delete is a true
//!   O(log n) removal. More code, and the fairer opponent.
//!
//! The order table here is deliberately the same structure as `L3Book`'s:
//! open addressing with linear probing over a pinned region, the same
//! multiplicative hash and the same backward-shift deletion, so that what is
//! measured is the priority structure and not the hash table around it.
//!
//! What a heap of orders cannot do cheaply is report how much is resting at
//! the best price, because that means visiting every order at it. The
//! brief's own success criteria ask for exactly that ("Quote Depth: Shares
//! available at best bid/ask"), so [`best_depth`](HeapBook::best_depth) is
//! provided and measured next to [`best`](HeapBook::best).

use crate::decode::Msg;
use crate::{Side, Top};
use titan_platform::mem::PinnedRegion;

/// How a delete reaches an order that is somewhere inside a heap.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Policy {
    /// Leave it in the heap and skip it when it reaches the top.
    Lazy,
    /// Track every order's heap position and remove it in place.
    Indexed,
}

/// One resting order inside a heap. `key` is the price for a bid and
/// `u32::MAX - price` for an ask, so both sides are max-heaps, and ties in
/// price are broken by the earlier sequence number, which is the brief's
/// price-time priority.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct Entry {
    key: u32,
    seq: u64,
    order: u64,
}

impl Entry {
    #[inline(always)]
    fn above(&self, other: &Entry) -> bool {
        (self.key, other.seq) > (other.key, self.seq)
    }
}

/// An order in the table: where it rests and, for [`Policy::Indexed`], where
/// it sits in its heap. The price lives in the heap entry's key rather than
/// here; with alignment this slot is the same 24 bytes as the array book's.
#[derive(Clone, Copy, Default)]
struct Slot {
    order: u64,
    shares: u32,
    pos: u32,
    locate: u16,
    buy: bool,
}

/// The brief's order book: a heap of orders per instrument and side.
pub struct HeapBook {
    slots: *mut Slot,
    mask: usize,
    shift: u32,
    live: usize,
    _region: PinnedRegion,
    heaps: Vec<[Vec<Entry>; 2]>,
    policy: Policy,
    seq: u64,
    /// Entries skipped at the top because their order is gone (lazy only).
    pub stale_popped: u64,
    /// Orders named by a message that the book does not hold.
    pub unknown: u64,
}

impl HeapBook {
    /// `log2_capacity` bounds the number of simultaneously live orders (kept
    /// below 70% load); `instruments` is one more than the highest locate.
    pub fn new(log2_capacity: u32, instruments: usize, policy: Policy) -> HeapBook {
        let cap = 1usize << log2_capacity;
        let region = PinnedRegion::new(cap * std::mem::size_of::<Slot>()).expect("order table");
        let mut heaps = Vec::with_capacity(instruments);
        heaps.resize_with(instruments, || [Vec::new(), Vec::new()]);
        HeapBook {
            slots: region.as_ptr() as *mut Slot,
            mask: cap - 1,
            shift: 64 - log2_capacity,
            live: 0,
            _region: region,
            heaps,
            policy,
            seq: 0,
            stale_popped: 0,
            unknown: 0,
        }
    }

    /// Remove every order, keeping the allocations.
    pub fn clear(&mut self) {
        // SAFETY: the region holds `self.mask + 1` slots.
        unsafe { std::ptr::write_bytes(self.slots, 0, self.mask + 1) };
        self.live = 0;
        self.seq = 0;
        self.stale_popped = 0;
        self.unknown = 0;
        for h in &mut self.heaps {
            h[0].clear();
            h[1].clear();
        }
    }

    pub fn live_orders(&self) -> usize {
        self.live
    }

    #[inline(always)]
    fn home(&self, order: u64) -> usize {
        (order.wrapping_mul(0x9E37_79B9_7F4A_7C15) >> self.shift) as usize
    }

    #[inline(always)]
    fn slot(&self, i: usize) -> &Slot {
        debug_assert!(i <= self.mask);
        // SAFETY: `i` is masked to the region's length.
        unsafe { &*self.slots.add(i) }
    }

    #[inline(always)]
    fn slot_mut(&mut self, i: usize) -> &mut Slot {
        debug_assert!(i <= self.mask);
        // SAFETY: `i` is masked to the region's length.
        unsafe { &mut *self.slots.add(i) }
    }

    #[inline(always)]
    fn find(&self, order: u64) -> Option<usize> {
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
    fn insert_slot(&mut self, s: Slot) {
        debug_assert!(s.order != 0);
        assert!(self.live * 10 < (self.mask + 1) * 7, "order table above 70% load");
        let mut i = self.home(s.order);
        while self.slot(i).order != 0 {
            i = (i + 1) & self.mask;
        }
        *self.slot_mut(i) = s;
        self.live += 1;
    }

    /// Backward-shift deletion for linear probing, as in `L3Book`. Any slot
    /// that moves has its heap entry's back-pointer corrected.
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
            let stays = if i <= j { i < k && k <= j } else { i < k || k <= j };
            if !stays {
                *self.slot_mut(i) = sj;
                if self.policy == Policy::Indexed {
                    let side = usize::from(!sj.buy);
                    self.heaps[sj.locate as usize][side][sj.pos as usize].order = sj.order;
                }
                i = j;
            }
        }
        self.slot_mut(i).order = 0;
        self.live -= 1;
    }

    /// Sifts the entry at `p` towards the top, writing back each moved
    /// order's position. Returns where it came to rest.
    #[inline(always)]
    fn sift_up(&mut self, locate: u16, side: usize, mut p: usize) -> usize {
        let mut h = std::mem::take(&mut self.heaps[locate as usize][side]);
        while p > 0 {
            let parent = (p - 1) / 2;
            if !h[p].above(&h[parent]) {
                break;
            }
            h.swap(p, parent);
            if self.policy == Policy::Indexed {
                if let Some(i) = self.find(h[p].order) {
                    self.slot_mut(i).pos = p as u32;
                }
            }
            p = parent;
        }
        if self.policy == Policy::Indexed {
            if let Some(i) = self.find(h[p].order) {
                self.slot_mut(i).pos = p as u32;
            }
        }
        self.heaps[locate as usize][side] = h;
        p
    }

    /// Sifts the entry at `p` towards the leaves, writing back positions.
    #[inline(always)]
    fn sift_down(&mut self, locate: u16, side: usize, mut p: usize) {
        let mut h = std::mem::take(&mut self.heaps[locate as usize][side]);
        let n = h.len();
        loop {
            let (l, r) = (2 * p + 1, 2 * p + 2);
            let mut top = p;
            if l < n && h[l].above(&h[top]) {
                top = l;
            }
            if r < n && h[r].above(&h[top]) {
                top = r;
            }
            if top == p {
                break;
            }
            h.swap(p, top);
            if self.policy == Policy::Indexed {
                if let Some(i) = self.find(h[p].order) {
                    self.slot_mut(i).pos = p as u32;
                }
            }
            p = top;
        }
        if self.policy == Policy::Indexed && p < n {
            if let Some(i) = self.find(h[p].order) {
                self.slot_mut(i).pos = p as u32;
            }
        }
        self.heaps[locate as usize][side] = h;
    }

    #[inline(always)]
    fn push(&mut self, locate: u16, side: usize, e: Entry) -> usize {
        self.heaps[locate as usize][side].push(e);
        let p = self.heaps[locate as usize][side].len() - 1;
        self.sift_up(locate, side, p)
    }

    /// Removes the entry at `p` from one heap.
    #[inline(always)]
    fn remove_entry(&mut self, locate: u16, side: usize, p: usize) {
        let n = self.heaps[locate as usize][side].len();
        if p + 1 == n {
            self.heaps[locate as usize][side].pop();
            return;
        }
        let last = self.heaps[locate as usize][side][n - 1];
        self.heaps[locate as usize][side][p] = last;
        self.heaps[locate as usize][side].pop();
        if self.policy == Policy::Indexed {
            if let Some(i) = self.find(last.order) {
                self.slot_mut(i).pos = p as u32;
            }
        }
        let up = self.sift_up(locate, side, p);
        if up == p {
            self.sift_down(locate, side, p);
        }
    }

    /// Drops entries from the top of one heap while they name orders the
    /// table no longer holds. Only [`Policy::Lazy`] leaves any.
    #[inline(always)]
    fn settle(&mut self, locate: u16, side: usize) {
        loop {
            let Some(&e) = self.heaps[locate as usize][side].first() else { return };
            if self.find(e.order).is_some() {
                return;
            }
            self.stale_popped += 1;
            let n = self.heaps[locate as usize][side].len();
            if n == 1 {
                self.heaps[locate as usize][side].pop();
                return;
            }
            let last = self.heaps[locate as usize][side][n - 1];
            self.heaps[locate as usize][side][0] = last;
            self.heaps[locate as usize][side].pop();
            if self.policy == Policy::Indexed {
                if let Some(i) = self.find(last.order) {
                    self.slot_mut(i).pos = 0;
                }
            }
            self.sift_down(locate, side, 0);
        }
    }

    /// Best price on one side, with the shares of the single order resting
    /// there. This is what the brief's "Match order: O(1)" reaches.
    #[inline(always)]
    pub fn best(&mut self, locate: u16, side: Side) -> Top {
        let s = usize::from(side != Side::Buy);
        if self.heaps.len() <= locate as usize {
            return None;
        }
        self.settle(locate, s);
        let e = *self.heaps[locate as usize][s].first()?;
        let shares = self.find(e.order).map_or(0, |i| self.slot(i).shares as u64);
        Some((if side == Side::Buy { e.key } else { u32::MAX - e.key }, shares))
    }

    /// Best price and every live share resting at it, which a heap of orders
    /// can only find by visiting all of them.
    #[inline(always)]
    pub fn best_depth(&mut self, locate: u16, side: Side) -> Top {
        let s = usize::from(side != Side::Buy);
        if self.heaps.len() <= locate as usize {
            return None;
        }
        self.settle(locate, s);
        let key = self.heaps[locate as usize][s].first()?.key;
        let mut shares = 0u64;
        let h = std::mem::take(&mut self.heaps[locate as usize][s]);
        for e in h.iter().filter(|e| e.key == key) {
            if let Some(i) = self.find(e.order) {
                shares += self.slot(i).shares as u64;
            }
        }
        self.heaps[locate as usize][s] = h;
        Some((if side == Side::Buy { key } else { u32::MAX - key }, shares))
    }

    fn add(&mut self, locate: u16, order: u64, side: Side, shares: u32, price: u32) {
        if shares == 0 || order == 0 {
            return;
        }
        let buy = side == Side::Buy;
        let s = usize::from(!buy);
        self.seq += 1;
        let key = if buy { price } else { u32::MAX - price };
        self.insert_slot(Slot { order, shares, locate, buy, pos: 0 });
        let p = self.push(locate, s, Entry { key, seq: self.seq, order });
        if self.policy == Policy::Indexed {
            if let Some(i) = self.find(order) {
                self.slot_mut(i).pos = p as u32;
            }
        }
    }

    fn reduce(&mut self, order: u64, shares: u32) {
        let Some(i) = self.find(order) else {
            self.unknown += 1;
            return;
        };
        let s = *self.slot(i);
        if shares < s.shares {
            self.slot_mut(i).shares = s.shares - shares;
            return;
        }
        self.kill(i, s);
    }

    fn delete(&mut self, order: u64) {
        let Some(i) = self.find(order) else {
            self.unknown += 1;
            return;
        };
        let s = *self.slot(i);
        self.kill(i, s);
    }

    #[inline(always)]
    fn kill(&mut self, i: usize, s: Slot) {
        if self.policy == Policy::Indexed {
            let side = usize::from(!s.buy);
            self.remove_entry(s.locate, side, s.pos as usize);
        }
        self.remove_at(i);
    }

    /// Applies one ITCH book message, as [`crate::Book::apply`] does for the
    /// array book.
    #[inline(always)]
    pub fn apply(&mut self, m: &Msg) {
        match *m {
            Msg::Add { locate, order, side, shares, price, .. } => self.add(locate, order, side, shares, price),
            Msg::Executed { order, shares, .. } | Msg::Cancel { order, shares, .. } => self.reduce(order, shares),
            Msg::ExecutedWithPrice { order, shares, .. } => self.reduce(order, shares),
            Msg::Delete { order, .. } => self.delete(order),
            Msg::Replace { old, new, shares, price, .. } => {
                let Some(i) = self.find(old) else {
                    self.unknown += 1;
                    return;
                };
                let s = *self.slot(i);
                self.kill(i, s);
                self.add(s.locate, new, if s.buy { Side::Buy } else { Side::Sell }, shares, price);
            }
            Msg::Other { .. } => {}
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::book::L3Book;
    use crate::gen::{feed, Mix, Rng};
    use crate::decode::Reader;
    use crate::Book;

    /// Both policies must track the array book's best price and its depth at
    /// that price, message for message, on a real-shaped feed.
    fn agrees_with_the_array_book(policy: Policy) {
        let src = feed(11, 64, 200_000, Mix::default());
        let mut fast = L3Book::new(18, 65);
        let mut heap = HeapBook::new(18, 65, policy);
        let mut r = Reader::new(&src);
        let mut checked = 0u64;
        while let Some(Ok(b)) = r.next_raw() {
            let Ok(m) = crate::decode::decode(b) else { continue };
            fast.apply(&m);
            heap.apply(&m);
            let locate = match m {
                Msg::Add { locate, .. }
                | Msg::Executed { locate, .. }
                | Msg::ExecutedWithPrice { locate, .. }
                | Msg::Cancel { locate, .. }
                | Msg::Delete { locate, .. }
                | Msg::Replace { locate, .. } => locate,
                Msg::Other { .. } => continue,
            };
            for side in [Side::Buy, Side::Sell] {
                let want = fast.best(locate, side);
                let got = heap.best(locate, side);
                assert_eq!(want.map(|t| t.0), got.map(|t| t.0), "best price, locate {locate}");
                assert_eq!(want, heap.best_depth(locate, side), "best price and depth, locate {locate}");
                checked += 1;
            }
        }
        assert!(checked > 100_000, "only {checked} comparisons");
        assert_eq!(heap.live_orders(), fast.live_orders());
    }

    #[test]
    fn lazy_policy_agrees_with_the_array_book() {
        agrees_with_the_array_book(Policy::Lazy);
    }

    #[test]
    fn indexed_policy_agrees_with_the_array_book() {
        agrees_with_the_array_book(Policy::Indexed);
    }

    /// Price-time priority: at one price the earlier order is on top.
    #[test]
    fn ties_break_by_arrival() {
        for policy in [Policy::Lazy, Policy::Indexed] {
            let mut b = HeapBook::new(12, 2, policy);
            b.apply(&Msg::Add { locate: 1, ts: 0, order: 10, side: Side::Buy, shares: 100, price: 500 });
            b.apply(&Msg::Add { locate: 1, ts: 1, order: 11, side: Side::Buy, shares: 200, price: 500 });
            // The top is the earlier order, so its own size is 100.
            assert_eq!(b.best(1, Side::Buy), Some((500, 100)), "{policy:?}");
            // Depth at that price counts both.
            assert_eq!(b.best_depth(1, Side::Buy), Some((500, 300)), "{policy:?}");
            b.apply(&Msg::Delete { locate: 1, ts: 2, order: 10 });
            assert_eq!(b.best(1, Side::Buy), Some((500, 200)), "{policy:?}");
            assert_eq!(b.best_depth(1, Side::Buy), Some((500, 200)), "{policy:?}");
        }
    }

    /// Deleting from the middle of a heap keeps the rest ordered.
    #[test]
    fn deletes_from_the_middle_keep_the_order() {
        for policy in [Policy::Lazy, Policy::Indexed] {
            let mut b = HeapBook::new(14, 2, policy);
            let mut rng = Rng::new(5);
            let mut live: Vec<(u64, u32)> = Vec::new();
            for order in 1..=2_000u64 {
                let price = 100 + rng.below(50) as u32;
                b.apply(&Msg::Add { locate: 1, ts: order, order, side: Side::Buy, shares: 10, price });
                live.push((order, price));
                if live.len() > 4 && rng.below(3) == 0 {
                    let k = rng.below(live.len() as u64) as usize;
                    let (o, _) = live.swap_remove(k);
                    b.apply(&Msg::Delete { locate: 1, ts: order, order: o });
                }
                let want = live.iter().map(|&(_, p)| p).max();
                assert_eq!(b.best(1, Side::Buy).map(|t| t.0), want, "{policy:?} after order {order}");
            }
            let want: u64 = live.len() as u64 * 10;
            let top = b.best(1, Side::Buy).unwrap().0;
            let at_top: u64 = live.iter().filter(|&&(_, p)| p == top).count() as u64 * 10;
            assert_eq!(b.best_depth(1, Side::Buy), Some((top, at_top)), "{policy:?}");
            assert_eq!(b.live_orders() as u64 * 10, want);
        }
    }
}
