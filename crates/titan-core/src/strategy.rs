use titan_common::MarketTick;

/// The Brain.
pub trait TradingStrategy {
    fn name(&self) -> &str;
    
    // Warning fixed: Removed #[inline(always)] from trait def. 
    // It belongs in the implementation.
    fn on_tick(&mut self, tick: &MarketTick) -> bool;
}

// --- STRATEGY 1: NAIVE MARKET MAKER ---
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
        // CHANGED: Threshold lowered to 0 to force activity on all ticks
        if tick.quantity > 0 {
            if tick.side == 1 { // Buy trade
                self.position -= tick.quantity as i64;
                // Warning fixed: Removed parens
                self.pnl += tick.quantity as f64 * tick.price; 
            } else { // Sell trade
                self.position += tick.quantity as i64;
                // Warning fixed: Removed parens
                self.pnl -= tick.quantity as f64 * tick.price;
            }
            self.trade_count += 1;
            return true;
        }
        false
    }
}