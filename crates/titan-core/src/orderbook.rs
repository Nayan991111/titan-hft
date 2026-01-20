// crates/titan-core/src/orderbook.rs

use rustc_hash::FxHashMap;
use titan_common::MarketTick;

#[derive(Debug, Clone, Copy)]
pub struct Trade {
    pub price: f64,
    pub quantity: f64,
    pub buyer_order_id: u64,
    pub seller_order_id: u64,
}

#[derive(Debug, Clone, Copy)]
pub struct Order {
    pub price: f64,
    pub quantity: f64,
    pub order_id: u64,
}

pub struct GlobalOrderBook {
    // STORAGE
    bids: FxHashMap<u64, Order>,
    asks: FxHashMap<u64, Order>,
    
    // CACHE
    pub best_bid: f64,
    pub best_ask: f64,
    
    // METRICS
    pub trades_count: u64,
}

impl GlobalOrderBook {
    pub fn new() -> Self {
        Self {
            bids: FxHashMap::default(),
            asks: FxHashMap::default(),
            best_bid: 0.0,
            best_ask: 999999.9,
            trades_count: 0,
        }
    }

    #[inline(always)]
    pub fn execute_order(&mut self, tick: &MarketTick) -> Option<Trade> {
        // 1. Parse Fields
        let is_bid = tick.side == 1; 
        let price = tick.price;
        let quantity = tick.quantity; // This is u64
        let order_id = tick.timestamp;

        // 2. MATCHING ENGINE LOGIC
        if is_bid {
            // INCOMING BID vs BEST ASK
            if price >= self.best_ask {
                // MATCH!
                self.trades_count += 1;
                return Some(Trade {
                    price: self.best_ask,
                    quantity: quantity as f64, // FIX 1: Cast to f64
                    buyer_order_id: order_id,
                    seller_order_id: 0, 
                });
            } else {
                // NO MATCH: Book it
                self.bids.insert(order_id, Order { 
                    price, 
                    quantity: quantity as f64, // FIX 2: Cast to f64
                    order_id 
                });
                if price > self.best_bid {
                    self.best_bid = price;
                }
            }
        } else {
            // INCOMING ASK vs BEST BID
            if price <= self.best_bid {
                // MATCH!
                self.trades_count += 1;
                return Some(Trade {
                    price: self.best_bid,
                    quantity: quantity as f64, // FIX 3: Cast to f64
                    buyer_order_id: 0, 
                    seller_order_id: order_id,
                });
            } else {
                // NO MATCH: Book it
                self.asks.insert(order_id, Order { 
                    price, 
                    quantity: quantity as f64, // FIX 4: Cast to f64
                    order_id 
                });
                if price < self.best_ask {
                    self.best_ask = price;
                }
            }
        }
        None
    }

    pub fn depth(&self) -> usize {
        self.bids.len() + self.asks.len()
    }
}