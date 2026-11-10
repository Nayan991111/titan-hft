use titan_common::MarketTick;

/// A strategy consumes ticks and reports whether it acted.
pub trait TradingStrategy {
    fn name(&self) -> &str;
    
    fn on_tick(&mut self, tick: &MarketTick) -> bool;
}

/// Baseline market maker used to load the hot path.
pub struct MarketMaker {
    pub position: i64,
    pub pnl: f64,
    pub trade_count: u64,
}

impl MarketMaker {
    pub fn new() -> Self {
        Self { position: 0, pnl: 0.0, trade_count: 0 }
    }
}

impl TradingStrategy for MarketMaker {
    fn name(&self) -> &str { "NaiveMM_v1" }

    #[inline(always)]
    fn on_tick(&mut self, tick: &MarketTick) -> bool {
        // Threshold 0: every tick produces a decision (worst case for the hot path).
        if tick.quantity > 0 {
            if tick.side == 1 { // Buy trade
                self.position -= tick.quantity as i64;
                self.pnl += tick.quantity as f64 * tick.price; 
            } else { // Sell trade
                self.position += tick.quantity as i64;
                self.pnl -= tick.quantity as f64 * tick.price;
            }
            self.trade_count += 1;
            return true;
        }
        false
    }
}