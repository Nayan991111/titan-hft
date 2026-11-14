use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;

/// Global kill switch. Once tripped, trading stops.
/// Acquire/Release so that state written before `trip` is visible to any
/// thread that observes the flag.
pub struct GlobalKillSwitch {
    tripped: AtomicBool,
}

impl Default for GlobalKillSwitch {
    fn default() -> Self {
        Self::new()
    }
}

impl GlobalKillSwitch {
    pub fn new() -> Self {
        Self { tripped: AtomicBool::new(false) }
    }

    #[inline(always)]
    pub fn is_tripped(&self) -> bool {
        self.tripped.load(Ordering::Acquire)
    }

    pub fn trip(&self) {
        println!("!!! KILL SWITCH TRIGGERED !!!");
        self.tripped.store(true, Ordering::Release);
    }
}

/// Per-thread risk limits for the consumer thread.
pub struct RiskManager {
    // Limits
    max_position: i64,
    max_drawdown: f64,
    
    // External Safety
    kill_switch: Arc<GlobalKillSwitch>,
}

impl RiskManager {
    pub fn new(kill_switch: Arc<GlobalKillSwitch>) -> Self {
        Self {
            max_position: 5_000,     // 5,000 unit limit
            max_drawdown: -20_000.0, // Stop if we lose $20k
            kill_switch,
        }
    }

    /// True if all limits hold; false (and trips the kill switch) otherwise.
    #[inline(always)]
    pub fn check(&self, current_position: i64, current_pnl: f64) -> bool {
        // Kill switch.
        if self.kill_switch.is_tripped() {
            return false;
        }

        // Position limit.
        if current_position.abs() > self.max_position {
            println!(">>> [RISK] Position Limit Breached: {} > {}", current_position, self.max_position);
            self.kill_switch.trip();
            return false;
        }

        // Drawdown limit.
        if current_pnl < self.max_drawdown {
            println!(">>> [RISK] Max Drawdown Breached: {:.2} < {:.2}", current_pnl, self.max_drawdown);
            self.kill_switch.trip();
            return false;
        }

        true
    }
}