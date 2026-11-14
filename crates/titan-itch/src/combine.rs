//! Consolidated top of book across venues, and the locked and crossed states
//! it exposes.
//!
//! Each venue publishes its own book under its own stock locate codes, so a
//! consolidated view needs one identifier per symbol across venues. The
//! consolidated best bid is the highest bid of any venue, the consolidated
//! best ask the lowest ask; the market is *locked* when they are equal and
//! *crossed* when the bid is above the ask. A resting book on one venue
//! cannot be crossed (a marketable order executes instead), so both states
//! are statements about two venues disagreeing, and both are visible only to
//! a participant who consolidates them.
//!
//! Two ways to keep the consolidated top, measured against each other by
//! `titan-cross`: update the instrument a message touched ([`set`]), or
//! re-read every venue's book for every instrument from time to time
//! ([`rebuild`]). The first is what [`Consolidated::set`] does; the second
//! is [`Consolidated::rebuild_from`].
//!
//! [`set`]: Consolidated::set
//! [`rebuild`]: Consolidated::rebuild_from

/// Venues consolidated here (BX and PSX).
pub const VENUES: usize = 2;

/// One venue's top of book for one instrument. An empty side is `bid` 0 or
/// `ask` [`u32::MAX`], so the folds below need no special cases.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Quote {
    pub bid: u32,
    pub ask: u32,
}

impl Default for Quote {
    fn default() -> Quote {
        Quote { bid: 0, ask: u32::MAX }
    }
}

impl Quote {
    pub fn has_both(&self) -> bool {
        self.bid != 0 && self.ask != u32::MAX
    }
}

/// What the consolidated top says about the two venues.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Market {
    /// At least one side has no quote on any venue.
    OneSided,
    /// Best bid below best ask.
    Normal,
    /// Best bid equal to best ask.
    Locked,
    /// Best bid above best ask.
    Crossed,
}

impl Quote {
    pub fn market(&self) -> Market {
        if !self.has_both() {
            Market::OneSided
        } else if self.bid > self.ask {
            Market::Crossed
        } else if self.bid == self.ask {
            Market::Locked
        } else {
            Market::Normal
        }
    }
}

/// Per-venue tops and the consolidated top, one entry per instrument.
pub struct Consolidated {
    venue: Vec<[Quote; VENUES]>,
    top: Vec<Quote>,
}

impl Consolidated {
    pub fn new(instruments: usize) -> Consolidated {
        Consolidated { venue: vec![[Quote::default(); VENUES]; instruments], top: vec![Quote::default(); instruments] }
    }

    pub fn len(&self) -> usize {
        self.top.len()
    }

    pub fn is_empty(&self) -> bool {
        self.top.is_empty()
    }

    #[inline(always)]
    pub fn top(&self, gid: usize) -> Quote {
        self.top[gid]
    }

    #[inline(always)]
    pub fn venue_quote(&self, gid: usize, v: usize) -> Quote {
        self.venue[gid][v]
    }

    /// Records venue `v`'s top for `gid` and refolds that instrument.
    /// Returns true when the venue's quote changed.
    #[inline(always)]
    pub fn set(&mut self, gid: usize, v: usize, q: Quote) -> bool {
        let slot = &mut self.venue[gid][v];
        if *slot == q {
            return false;
        }
        *slot = q;
        self.top[gid] = fold(&self.venue[gid]);
        true
    }

    /// Re-reads every venue's top for every instrument through `read` and
    /// refolds all of them: the cost of keeping no per-instrument state.
    #[inline(always)]
    pub fn rebuild_from<F: FnMut(usize, usize) -> Quote>(&mut self, mut read: F) {
        for gid in 0..self.top.len() {
            for v in 0..VENUES {
                self.venue[gid][v] = read(gid, v);
            }
            self.top[gid] = fold(&self.venue[gid]);
        }
    }

    /// Forgets every quote, keeping the allocations.
    pub fn clear(&mut self) {
        self.venue.fill([Quote::default(); VENUES]);
        self.top.fill(Quote::default());
    }

    /// The consolidated top each instrument would have from the venue quotes
    /// held now, computed from scratch. Used by the tests and by the
    /// harness's correctness check, never on a measured path.
    pub fn refolded(&self) -> Vec<Quote> {
        self.venue.iter().map(fold).collect()
    }
}

#[inline(always)]
fn fold(q: &[Quote; VENUES]) -> Quote {
    let mut t = Quote::default();
    for v in q {
        if v.bid > t.bid {
            t.bid = v.bid;
        }
        if v.ask < t.ask {
            t.ask = v.ask;
        }
    }
    t
}

/// How long each state lasted and how often it began. Durations are summed
/// over instruments, so the total can exceed the wall time of the window.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct CrossStats {
    pub locked_episodes: u64,
    pub crossed_episodes: u64,
    pub locked_ns: u128,
    pub crossed_ns: u128,
    pub max_crossed_ns: u64,
    pub max_locked_ns: u64,
    /// Sum over crossed episodes of (best bid minus best ask) when the
    /// episode began, in ITCH price units (1 = 0.0001 of a dollar).
    pub crossed_depth_units: u64,
    /// Instruments that were crossed at least once.
    pub instruments_crossed: u64,
    /// Episodes still open when the window ended (their time is counted).
    pub open_at_end: u64,
}

/// Watches each instrument's state and accumulates [`CrossStats`].
pub struct CrossWatch {
    state: Vec<Market>,
    since: Vec<u64>,
    ever_crossed: Vec<bool>,
    pub stats: CrossStats,
}

impl CrossWatch {
    pub fn new(instruments: usize) -> CrossWatch {
        CrossWatch {
            state: vec![Market::OneSided; instruments],
            since: vec![0; instruments],
            ever_crossed: vec![false; instruments],
            stats: CrossStats::default(),
        }
    }

    /// Records `gid`'s state at `ts`. Timestamps must not go backwards.
    #[inline(always)]
    pub fn observe(&mut self, gid: usize, q: Quote, ts: u64) {
        let now = q.market();
        let was = self.state[gid];
        if now == was {
            return;
        }
        self.close_one(gid, was, ts);
        self.state[gid] = now;
        self.since[gid] = ts;
        match now {
            Market::Locked => self.stats.locked_episodes += 1,
            Market::Crossed => {
                self.stats.crossed_episodes += 1;
                self.stats.crossed_depth_units += (q.bid - q.ask) as u64;
                if !self.ever_crossed[gid] {
                    self.ever_crossed[gid] = true;
                    self.stats.instruments_crossed += 1;
                }
            }
            _ => {}
        }
    }

    #[inline(always)]
    fn close_one(&mut self, gid: usize, was: Market, ts: u64) {
        let d = ts.saturating_sub(self.since[gid]);
        match was {
            Market::Locked => {
                self.stats.locked_ns += d as u128;
                self.stats.max_locked_ns = self.stats.max_locked_ns.max(d);
            }
            Market::Crossed => {
                self.stats.crossed_ns += d as u128;
                self.stats.max_crossed_ns = self.stats.max_crossed_ns.max(d);
            }
            _ => {}
        }
    }

    /// Closes every open episode at the end of the window.
    pub fn close(&mut self, ts: u64) {
        for gid in 0..self.state.len() {
            let was = self.state[gid];
            if matches!(was, Market::Locked | Market::Crossed) {
                self.stats.open_at_end += 1;
            }
            self.close_one(gid, was, ts);
            self.state[gid] = Market::OneSided;
            self.since[gid] = ts;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::gen::Rng;

    fn q(bid: u32, ask: u32) -> Quote {
        Quote { bid, ask }
    }

    #[test]
    fn one_venue_alone_is_never_crossed() {
        let mut c = Consolidated::new(1);
        assert_eq!(c.top(0).market(), Market::OneSided);
        // A well formed book on one venue: bid below ask.
        assert!(c.set(0, 0, q(100, 101)));
        assert_eq!(c.top(0).market(), Market::Normal);
        // The same quote again is not a change.
        assert!(!c.set(0, 0, q(100, 101)));
        // A second venue quoting wider changes nothing about the top.
        assert!(c.set(0, 1, q(99, 102)));
        assert_eq!(c.top(0), q(100, 101));
        assert_eq!(c.top(0).market(), Market::Normal);
    }

    #[test]
    fn two_venues_lock_and_cross() {
        let mut c = Consolidated::new(1);
        c.set(0, 0, q(100, 103));
        // B's ask meets A's bid: locked.
        c.set(0, 1, q(97, 100));
        assert_eq!(c.top(0), q(100, 100));
        assert_eq!(c.top(0).market(), Market::Locked);
        // B's ask falls below A's bid: crossed by 2 units.
        c.set(0, 1, q(97, 98));
        assert_eq!(c.top(0), q(100, 98));
        assert_eq!(c.top(0).market(), Market::Crossed);
        // One side empties: no longer a two-sided market.
        c.set(0, 0, q(0, u32::MAX));
        c.set(0, 1, q(97, u32::MAX));
        assert_eq!(c.top(0).market(), Market::OneSided);
    }

    #[test]
    fn episodes_are_timed_and_counted() {
        let mut c = Consolidated::new(1);
        let mut w = CrossWatch::new(1);
        c.set(0, 0, q(100, 103));
        c.set(0, 1, q(97, 102));
        w.observe(0, c.top(0), 1_000);
        c.set(0, 1, q(97, 98));
        w.observe(0, c.top(0), 2_000); // crossed by 2 at t=2000
        c.set(0, 1, q(97, 100));
        w.observe(0, c.top(0), 2_500); // locked at t=2500
        c.set(0, 1, q(97, 104));
        w.observe(0, c.top(0), 4_000); // normal again
        w.close(10_000);
        let s = w.stats;
        assert_eq!((s.crossed_episodes, s.locked_episodes), (1, 1));
        assert_eq!((s.crossed_ns, s.locked_ns), (500, 1500));
        assert_eq!((s.max_crossed_ns, s.max_locked_ns), (500, 1500));
        assert_eq!((s.crossed_depth_units, s.instruments_crossed, s.open_at_end), (2, 1, 0));
    }

    #[test]
    fn an_episode_open_at_the_end_is_counted_once() {
        let mut c = Consolidated::new(1);
        let mut w = CrossWatch::new(1);
        c.set(0, 0, q(100, 103));
        c.set(0, 1, q(97, 99));
        w.observe(0, c.top(0), 1_000);
        w.close(3_000);
        assert_eq!((w.stats.crossed_episodes, w.stats.crossed_ns, w.stats.open_at_end), (1, 2000, 1));
        // Closing twice must not count the time again.
        w.close(5_000);
        assert_eq!((w.stats.crossed_episodes, w.stats.crossed_ns, w.stats.open_at_end), (1, 2000, 1));
    }

    /// Keeping the top per instrument must give exactly what re-reading every
    /// venue would give, over a long random sequence.
    #[test]
    fn incremental_matches_a_full_rebuild() {
        let n = 64;
        let mut c = Consolidated::new(n);
        let mut mirror = vec![[Quote::default(); VENUES]; n];
        let mut rng = Rng::new(7);
        for step in 0..20_000 {
            let gid = rng.below(n as u64) as usize;
            let v = rng.below(VENUES as u64) as usize;
            let empty = rng.below(8) == 0;
            let q = if empty {
                Quote::default()
            } else {
                let bid = 90 + rng.below(20) as u32;
                let ask = 95 + rng.below(20) as u32;
                Quote { bid, ask }
            };
            mirror[gid][v] = q;
            c.set(gid, v, q);
            if step % 997 == 0 {
                for (gid, m) in mirror.iter().enumerate() {
                    assert_eq!(c.venue_quote(gid, 0), m[0]);
                    assert_eq!(c.venue_quote(gid, 1), m[1]);
                }
                assert_eq!(c.refolded(), (0..n).map(|g| c.top(g)).collect::<Vec<_>>());
            }
        }
        let mut rebuilt = Consolidated::new(n);
        rebuilt.rebuild_from(|gid, v| mirror[gid][v]);
        for gid in 0..n {
            assert_eq!(rebuilt.top(gid), c.top(gid), "instrument {gid}");
        }
    }
}
