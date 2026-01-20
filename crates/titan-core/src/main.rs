use std::sync::Arc;
use std::thread;
use std::time::Instant; // Removed unused 'Duration'
use titan_core::buffer::RingBuffer;
use titan_core::orderbook::GlobalOrderBook;
use titan_core::network::BatchReceiver;
use titan_core::logging::AsyncLogger;
use titan_core::risk::{RiskManager, GlobalKillSwitch};
mod strategy;
use crate::strategy::{TradingStrategy, MarketMaker};

// Apple Silicon M4 High Priority Enforcement
#[cfg(target_os = "macos")]
fn set_high_priority() {
    // In a real deployment, we would use thread_policy_set via libc
    // For this lab, we assume the OS scheduler favors active threads
}
#[cfg(not(target_os = "macos"))]
fn set_high_priority() {}

const BENCHMARK_SAMPLES: usize = 1_000_000;

fn main() {
    println!("=== TITAN HFT ENGINE v1.0 (Benchmark Mode) ===");
    println!(">>> Hardware: Apple M4 (ARM64)");
    println!(">>> Mode: Latency Benchmarking (Recording {} samples)", BENCHMARK_SAMPLES);

    let ring = Arc::new(RingBuffer::new(16_384));
    
    // We keep the logger reference to prevent it from dropping, 
    // even though we don't use it in the hot loop.
    let _logger = Arc::new(AsyncLogger::new("/dev/null")); 
    let kill_switch = Arc::new(GlobalKillSwitch::new());

    // CONSUMER THREAD (Engine)
    let consumer_ring = ring.clone();
    let consumer_kill_switch = kill_switch.clone();
    
    let consumer_handle = thread::spawn(move || {
        set_high_priority();
        let mut orderbook = GlobalOrderBook::new();
        let mut strategy = MarketMaker::new();
        let risk_manager = RiskManager::new(consumer_kill_switch);
        
        // Pre-allocate latency histogram (bucketed by microseconds)
        // Index = microseconds, Value = count. Max tracking 10ms (10,000us)
        let mut histogram = vec![0u64; 10_001]; 
        let mut samples_collected = 0;
        let mut total_latency_ns = 0u64;

        println!(">>> [CORE] Warmup complete. Capturing metrics...");

        loop {
            match consumer_ring.read() {
                Some(tick) => {
                    // --- START MEASUREMENT ---
                    // We measure the "Tick-to-Decision" latency:
                    // Time to update Orderbook + Time to run Strategy + Time to check Risk
                    let start_process = Instant::now();

                    // 1. Orderbook Update
                    if let Some(_trade) = orderbook.execute_order(&tick) { }
                    
                    // 2. Strategy Signal
                    let _signal = strategy.on_tick(&tick);

                    // 3. Risk Check
                    if !risk_manager.check(strategy.position, strategy.pnl) {
                        break; 
                    }

                    // --- STOP MEASUREMENT ---
                    let elapsed = start_process.elapsed(); 
                    let nanos = elapsed.as_nanos() as u64; 

                    // 4. Record Metrics
                    if tick.timestamp > 0 && samples_collected < BENCHMARK_SAMPLES {
                        let micros = (nanos / 1_000) as usize;
                        if micros < 10_000 {
                            histogram[micros] += 1;
                        } else {
                            histogram[10_000] += 1; // Overflow bucket
                        }
                        
                        total_latency_ns += nanos;
                        samples_collected += 1;

                        if samples_collected == BENCHMARK_SAMPLES {
                            println!(">>> [BENCHMARK] Collection Complete. Calculating stats...");
                            break;
                        }
                    }
                }
                None => { std::hint::spin_loop(); }
            }
        }
        
        // REPORT GENERATION
        let avg_ns = total_latency_ns as f64 / samples_collected as f64;
        let mut count = 0;
        let mut p50 = 0;
        let mut p99 = 0;
        let mut p999 = 0;
        
        for (us, &c) in histogram.iter().enumerate() {
            count += c;
            if p50 == 0 && count >= (samples_collected / 2) as u64 { p50 = us; }
            if p99 == 0 && count >= (samples_collected as f64 * 0.99) as u64 { p99 = us; }
            if p999 == 0 && count >= (samples_collected as f64 * 0.999) as u64 { p999 = us; }
        }

        println!("\n=== TITAN BENCHMARK RESULTS (M4/ARM64) ===");
        println!("Samples: {}", samples_collected);
        println!("Avg Latency: {:.2} us", avg_ns / 1000.0);
        println!("p50 (Median): {} us", p50);
        println!("p99 (Tail):   {} us", p99);
        println!("p99.9:        {} us", p999);
        println!("==========================================\n");
    });

    // PRODUCER (Load Generator)
    let producer_ring = ring.clone();
    thread::spawn(move || {
        set_high_priority();
        let addr = "127.0.0.1:5555".parse().unwrap();
        let mut rx = BatchReceiver::new(addr).expect("Bind failed");
        rx.listen_loop(&producer_ring);
    });

    consumer_handle.join().unwrap();
}