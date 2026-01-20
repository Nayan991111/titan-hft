use std::fs::File;
use std::io::{Write, BufWriter};
use std::sync::Arc;
use std::thread;
use std::time::Duration;
use crate::buffer::RingBuffer;
use titan_common::MarketTick;

const WRITE_BUFFER_SIZE: usize = 128 * 1024; // Increased to 128KB for better batching
const LOG_RING_CAPACITY: usize = 1024 * 1024; // 1 Million Slots (32MB RAM)

pub struct AsyncLogger {
    ring: Arc<RingBuffer>,
    _handle: thread::JoinHandle<()>,
}

impl AsyncLogger {
    pub fn new(filename: &str) -> Self {
        // Initialize with HUGE capacity
        let ring = Arc::new(RingBuffer::new(LOG_RING_CAPACITY));
        let ring_consumer = ring.clone();
        let filename = filename.to_string();

        let handle = thread::spawn(move || {
            println!(">>> [LOGGER] Async Writer Started: {} | Buffer: 1M slots", filename);
            let file = File::create(filename).expect("Failed to create log file");
            let mut writer = BufWriter::with_capacity(WRITE_BUFFER_SIZE, file);

            loop {
                let mut drained = 0;
                while let Some(tick) = ring_consumer.read() {
                    let bytes = unsafe {
                        std::slice::from_raw_parts(
                            (&tick as *const MarketTick) as *const u8,
                            std::mem::size_of::<MarketTick>(),
                        )
                    };
                    
                    if let Err(_) = writer.write_all(bytes) {
                        // Error handling
                    }
                    
                    drained += 1;
                    // Drain up to 4096 items before checking flush or yielding
                    if drained > 4096 { break; } 
                }

                if drained == 0 {
                    thread::sleep(Duration::from_micros(10));
                }
            }
        });

        Self {
            ring,
            _handle: handle,
        }
    }

    #[inline(always)]
    pub fn log(&self, tick: MarketTick) {
        // If 1M buffer is full, we are in serious trouble. 
        // We spin, but with 1M slots, this should almost never happen.
        while !self.ring.write(tick) {
            std::hint::spin_loop();
        }
    }
}