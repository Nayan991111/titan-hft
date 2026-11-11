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
| `titan-spsc`: SPSC ring with selectable ordering (`ldapr`, `ldar`, `dmb`), 64/128-byte index padding, cached indices, four wait strategies (`spin`, `isb`, `yield`, `wfe`), `Mutex<VecDeque>` baseline | done, stress-tested; measurement in progress |
| `titan-pingpong`: cross-core round-trip harness (optional PMU counters, P-core or E-core QoS) | done |
| `titan-stream`: streaming throughput harness with sum check | done |
| `scripts/census.sh`, `scripts/analyze.py`: instruction census, per-epoch statistics (bootstrap CI, Mann-Whitney U, Cliff's delta) | done |
| `titan-itch`: ITCH 5.0 decoder, zero-allocation L3 order book (Fibonacci-hashed order table, sorted price levels, fixed-point prices), reference book, synthetic feed generator | done; differential-tested against the reference after every message, mutation-checked |
| `titan-replay`: order-book replay benchmark | done |
| Replay of a real NASDAQ ITCH 5.0 sample day (`scripts/fetch_itch_sample.sh`, `titan-replay file=...`), verified against the reference book after every message | done |
| `titan-t2t`: single-core software tick-to-trade path (ingress ring, decode, L3 book, signal, egress ring) with component ablation | done; measurement in progress |
| Shared-memory and UDP (MoldUDP64) transports | planned |
| Reproduction script and analysis | planned |

The original prototype (`titan-core`, `titan-feed-sim`) is kept as a
baseline and is being replaced component by component.

## Layout

```
crates/
  titan-platform/   platform layer and the titan-probe self-test
  titan-spsc/       SPSC rings, wait strategies, titan-pingpong and titan-stream
  titan-itch/       ITCH 5.0 decoder, L3 book, reference book, generator,
                    titan-replay and titan-t2t
  titan-core/       engine prototype (SPSC ring, order book, strategy, risk)
  titan-common/     shared wire types
  titan-feed-sim/   UDP feed simulator
scripts/            data download, instruction census and result analysis
docs/               experiment log
```

Hypotheses are recorded in `docs/experiments.md` before the runs that test
them.

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
./target/release/titan-pingpong 200000 3 # round trips per case, epochs
./target/release/titan-pingpong 200000 3 bg          # efficiency-core QoS
sudo ./target/release/titan-pingpong 200000 3 pmu    # with PMU counters
./target/release/titan-stream 10000000 3             # messages per case, epochs
./target/release/titan-replay 5000000 500 3          # messages, instruments, epochs
scripts/fetch_itch_sample.sh                         # public BX sample day into data/
./target/release/titan-replay file=data/20190730.BX_ITCH_50 limit=20000000 epochs=3
./target/release/titan-t2t messages=5000000 epochs=5          # tick-to-trade stages
sudo ./target/release/titan-t2t messages=5000000 epochs=5 pmu # with PMU counters
./target/release/titan-t2t file=data/20190730.BX_ITCH_50 limit=20000000 epochs=5
scripts/census.sh --list                             # ring instructions
scripts/analyze.py results.txt                       # statistics over epochs
```

PMU access uses the private `kperf` and `kperfdata` frameworks and needs root.
Quit `powermetrics` and Instruments first; they hold the counters.

## License

MIT
