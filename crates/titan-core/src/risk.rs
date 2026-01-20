use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;

/// Global Kill Switch.
/// If true, the system MUST stop trading immediately.
/// Using Relaxed ordering for 'load' in the hot loop is usually acceptable for 
/// this specific flag to minimize cache coherency traffic, but we will use 
/// Acquire/Release to ensure memory visibility of the reason for the kill.
pub struct GlobalKillSwitch {
    tripped: AtomicBool,
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

/// Thread-local Risk Manager for the Consumer thread.
/// Tracks aggregate stats across all strategies (currently just one).
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

    /// Returns TRUE if the state is safe.
    /// Returns FALSE if a limit is breached (triggers kill switch).
    #[inline(always)]
    pub fn check(&self, current_position: i64, current_pnl: f64) -> bool {
        // 1. Check Global Kill Switch (Atomic Load - ~10-20 cycles)
        if self.kill_switch.is_tripped() {
            return false;
        }

        // 2. Check Position Limits (Integer comparison - ~1 cycle)
        if current_position.abs() > self.max_position {
            println!(">>> [RISK] Position Limit Breached: {} > {}", current_position, self.max_position);
            self.kill_switch.trip();
            return false;
        }

        // 3. Check Drawdown (Float comparison - ~3 cycles)
        if current_pnl < self.max_drawdown {
            println!(">>> [RISK] Max Drawdown Breached: {:.2} < {:.2}", current_pnl, self.max_drawdown);
            self.kill_switch.trip();
            return false;
        }

        true
    }
}