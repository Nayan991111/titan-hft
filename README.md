# Titan HFT Engine

**Architect:** Nayan Pandit
**Platform:** Apple Silicon M4 (ARM64)
**Performance:** ~30ns Internal Latency | 8.5M msgs/sec

## Overview
Titan HFT is an ultra-low latency trading engine built in Rust from first principles. It is designed to exploit the specific hardware characteristics of the Apple M4 chip, utilizing cache-line aligned data structures and lock-free concurrency patterns to achieve nanosecond-scale decision loops.

## Key Architecture
* **Core:** Rust-based engine with zero-allocation hot paths.
* **Concurrency:** Single-Producer Single-Consumer (SPSC) Lock-Free Ring Buffer.
* **Memory:** Custom memory layout with 128-byte padding to prevent False Sharing on ARM64.
* **Network:** Jumbo Packet processing (32 ticks/packet) to minimize syscall overhead.

## Benchmarks
Running on MacBook Air (M4):
* **Avg Latency:** 30 nanoseconds (Tick-to-Trade)
* **p99 Latency:** < 1 microsecond
* **Throughput:** ~9 Million ticks/second

## Usage
### 1. Start the Engine
```bash
cargo run --release --bin titan-core