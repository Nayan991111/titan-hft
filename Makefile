# Titan HFT Engine - Automation Interface

.PHONY: all run sim clean bench check

# Default: Build everything in release mode
all:
	cargo build --release

# Run the Engine (Consumer)
run:
	@echo ">>> Launching Titan Engine (M4 Optimized)..."
	cargo run --release --bin titan-core

# Run the Feed Simulator (Producer)
sim:
	@echo ">>> Injecting Market Data..."
	cargo run --release --bin titan-feed-sim

# Clean build artifacts
clean:
	cargo clean
	rm -f trade_log.bin

# Fast check for errors (Development)
check:
	cargo check