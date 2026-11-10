# Titan

Titan is a Rust market-data pipeline and measurement harness for studying
nanosecond-scale latency on Apple silicon (M4, macOS, arm64).

The project asks a narrow question: on a modern weakly ordered Arm core,
what does it actually cost to hand a market-data event from one core to
another and act on it? The answer depends on memory ordering (`ldapr`,
`ldar`, `dmb`), cache-line layout, how the waiting thread waits, and how the
XNU scheduler places threads that cannot be pinned. Titan measures each of
these with hardware performance counters instead of wall-clock timers.

## Status

Work in progress. No performance figures are published yet. Numbers will be
added here only together with the exact command, commit, hardware state and
raw data that produced them.

| Component | State |
|---|---|
| `titan-platform`: pinned memory, counter timer, QoS, kperf PMU access, allocation accounting | done, self-test in `titan-probe` |
| SPSC ring with selectable ordering, padding and wait strategy | in progress |
| ITCH 5.0 decoder and zero-allocation L3 order book | planned |
| Shared-memory and UDP (MoldUDP64) transports | planned |
| Reproduction script and analysis | planned |

The original prototype (`titan-core`, `titan-feed-sim`) is kept as a
baseline and is being replaced component by component.

## Layout

```
crates/
  titan-platform/   platform layer and the titan-probe self-test
  titan-core/       engine prototype (SPSC ring, order book, strategy, risk)
  titan-common/     shared wire types
  titan-feed-sim/   UDP feed simulator
```

## Measurement principles

* The arm64 generic timer on Apple silicon ticks at 24 MHz (41.67 ns). It is
  used for spans and frequency calibration only. Per-operation costs come from
  PMU cycle counts over batches.
* Counter reads enter the kernel, so they bracket batches, never single events.
* macOS has no hard thread affinity on Apple silicon. Threads are steered with
  QoS classes and placement effects are reported, not assumed away.
* Hot-path memory is page aligned (16 KiB), pre-faulted and locked; fault and
  allocation counts are checked to be zero inside measured regions.

## Build and self-test

Tested with Rust 1.92 (macOS 27, Apple M4) and Rust 1.97 (Linux x86_64).

```
cargo build --release
cargo test --release -- --test-threads=1
./target/release/titan-probe            # platform checks, no root
sudo ./target/release/titan-probe pmu   # also programs and validates the PMU
```

PMU access uses the private `kperf` and `kperfdata` frameworks and needs root.
Quit `powermetrics` and Instruments first; they hold the counters.

## License

MIT
