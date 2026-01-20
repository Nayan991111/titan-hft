use std::sync::atomic::{AtomicU64, Ordering};
use std::cell::UnsafeCell;
use titan_common::MarketTick;

#[cfg(target_arch = "aarch64")]
const CACHE_LINE_SIZE: usize = 128;
#[cfg(not(target_arch = "aarch64"))]
const CACHE_LINE_SIZE: usize = 64;

#[repr(C, align(128))]
pub struct RingBuffer {
    head: AtomicU64,
    _pad1: [u8; CACHE_LINE_SIZE],
    tail: AtomicU64,
    _pad2: [u8; CACHE_LINE_SIZE],
    // Changed from fixed array to Vec (Heap)
    buffer: Vec<UnsafeCell<MarketTick>>, 
    capacity: usize,
    mask: usize,
}

unsafe impl Sync for RingBuffer {}
unsafe impl Send for RingBuffer {}

impl RingBuffer {
    // Now accepts a specific capacity
    pub fn new(capacity: usize) -> Self {
        // Ensure power of 2 for fast bitwise masking
        assert!(capacity.is_power_of_two(), "Buffer capacity must be power of 2");
        
        let mut buffer = Vec::with_capacity(capacity);
        for _ in 0..capacity {
            buffer.push(UnsafeCell::new(MarketTick::default()));
        }

        Self {
            head: AtomicU64::new(0),
            _pad1: [0; CACHE_LINE_SIZE],
            tail: AtomicU64::new(0),
            _pad2: [0; CACHE_LINE_SIZE],
            buffer,
            capacity,
            mask: capacity - 1,
        }
    }

    #[inline(always)]
    pub fn write(&self, value: MarketTick) -> bool {
        let tail = self.tail.load(Ordering::Relaxed);
        let head = self.head.load(Ordering::Acquire);
        
        // Use dynamic capacity
        if tail.wrapping_sub(head) >= self.capacity as u64 { 
            return false; 
        }
        
        let index = (tail as usize) & self.mask;
        // Unsafe get check eliminated by logic above, but Vec bounds check exists technically. 
        // For raw speed we use get_unchecked or just array indexing (Rust optimizes Vec index well)
        unsafe { *self.buffer.get_unchecked(index).get() = value; }
        
        self.tail.store(tail.wrapping_add(1), Ordering::Release);
        true
    }

    #[inline(always)]
    pub fn read(&self) -> Option<MarketTick> {
        let head = self.head.load(Ordering::Relaxed);
        let tail = self.tail.load(Ordering::Acquire);
        
        if head == tail { 
            return None; 
        }
        
        let index = (head as usize) & self.mask;
        let value = unsafe { *self.buffer.get_unchecked(index).get() };
        
        self.head.store(head.wrapping_add(1), Ordering::Release);
        Some(value)
    }
}