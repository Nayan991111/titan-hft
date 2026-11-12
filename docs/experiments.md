# Experiment log and pre-registered hypotheses

Each hypothesis is committed to this file before the measurement that tests
it, so the repository history shows that predictions preceded data. Verdicts
are added afterwards and are never edited away when a prediction fails.

## Protocol

* Unit of analysis: one epoch (one value per configuration per epoch).
  Statistics in `scripts/analyze.py`: median, bootstrap 95% CI, two-sided
  Mann-Whitney U, Cliff's delta.
* Preliminary runs: 3 to 5 epochs inside one process, on battery power.
  Epochs in one process share thermal and memory state, so their p-values
  overstate confidence. Final measurements: wall power, Low Power Mode off,
  at least 30 independent process launches per configuration.
* Primary unit for costs: CPU cycles from the PMU (core frequency varies by
  workload, 3.3 to 4.4 GHz observed). Nanoseconds are reported with the
  frequency they were measured at.
* PMU counts include kernel work done on the thread's behalf: kperf's
  counting-mode flag did not exclude kernel instructions in a `getppid` test
  (1110.2 instructions per call in both modes).
* Hardware: MacBook Air (Mac16,13), Apple M4, 4 P-cores + 6 E-cores, macOS 27.2.

## Hypotheses

| ID | Prediction | Status | Evidence (preliminary) |
|---|---|---|---|
| H1 | Ping-pong: AcqRel and SeqCst within 5% | supported | -1.7% to +3.9% |
| H1b | Streaming: SeqCst more than 5% slower than AcqRel | refuted | 8.34 vs 8.35 cycles per message |
| H2 | Fenced (`dmb ish`) is the slowest ring variant | supported | +7% to +32% ping-pong, 2.6x cycles streaming |
| H3 | Ping-pong, uncached: 64-byte padding slower than 128 | not supported | -2.3% to +0.6% |
| H3b | Streaming, uncached: 64-byte padding slower than 128 | refuted (reversed), pending re-test | 185 vs 219 cycles per message; index placement was not controlled in that run |
| H4 | `wfe` has a higher median round trip than `isb` spinning | refuted | `wfe` 272 vs `isb` 347 cycles per round trip |
| H5 | kperf flag value 1 is not a user-only filter | confirmed | identical `getppid` instruction count |
| H6 | Sub-step counter deltas (17 or 18 units) below 1 per 10,000 | refuted | 5.70 per 10,000 |
| H7 | `wfe` wins because polling steals the line | refuted | plain spin misses L1D 10x more than `isb` yet is faster |
| H8 | E-core QoS round trip at least 3x P-core | refuted | 2.05x |
| H9 | Core frequency within 5% of 4.4 GHz during ping-pong | refuted | 3.3 to 3.7 GHz |
| H10 | L3 book at least 2x faster than the reference book | supported | 3.1x to 3.3x |
| H11 | L3 book allocates nothing in measured epochs | supported | 0 vs 21k to 27k |
| H12 | L3 book takes at least 1 L1D miss per message | supported | 13.1 to 16.0 |
| H13 | Decoding is under 20% of L3 per-message cycles | supported | 14.6% |
| H14 | With head and tail forced into one 128-byte line, 64-byte padding is faster than 128-byte padding in uncached streaming | inconclusive | cycles 169 vs 220 per message, lower in 3 of 3 PMU epochs; timing epochs p = 0.69; retest at N = 30 |
| H15 | On a real NASDAQ BX day the L3 book matches the reference book after every message, with no unknown order references | supported | BX 2019-07-30, 28,734,686 messages: 0 top-of-book mismatches, 0 unknown references |
| H16 | On real data the L3 book takes fewer L1D misses per message than on the synthetic default mix (13.8) | supported | 5.98 per message (98.3 cycles, 24.2 ns); peak 67,283 live orders vs 982,360 synthetic |
| H17 | SeqCst at or below AcqRel cost (seen in three runs) is a code placement effect, not an ordering effect: it disappears when loop alignment is fixed | pending | streaming uncached -4.4% (p = 0.03), cached -3.7%. Test and decision rule below |
| H18 | Single-core software tick-to-trade (`titan-t2t` stage `full`, synthetic default mix) costs more than 30 ns per message; predicted 45 to 60 ns | supported; predicted range missed (low) | 42.7 ns (bootstrap 95% CI 42.7 to 42.9), 189 cycles per message; real BX data 31.8 ns (31.8 to 32.2), 135 cycles |
| H19 | Ring operations add less than 10% to the trading path: `full` within 10% of `direct` in cycles per message | supported | +7.8% cycles synthetic (189 vs 176), +6.7% real (135 vs 127) |
| H20 | Component ablation is subtractive: stage `book` costs no more cycles per message than stage `full` | supported on M4 | 169 vs 189 cycles synthetic, 119 vs 135 real, 5 of 5 epochs each. The inversion seen first on the x86 development VM did not reproduce: on the VM `book` was slower than `full` (115 to 142 vs 94 to 123 ns per message) while executing fewer instructions (223 vs 274 per message, callgrind) |
| H21 | With limits no order can breach, the eight pre-trade checks (`risk` minus `full`) cost under 60 cycles per checked order on both feeds | pending | |
| H22 | Two-core pipeline at 1M msg/s offered (synthetic): median scheduled-arrival-to-decision latency is between 80 and 250 ns for spin, isb and wfe | pending | |
| H23 | At 1M msg/s the p99.99 exceeds 1 us for every wait strategy: interrupts and scheduling, not the data path, set the extreme tail | pending | |
| H24 | The median latency with wfe is within one counter step (41.67 ns) of spin at every load that does not saturate | pending | |
| H25 | The p99 at the highest offered load that does not saturate is more than 5 times the p99 at 1M msg/s | pending | |

## Notes on `titan-t2t`

* What is timed: the trading path on one core for messages already in the
  ingress ring. Filling the ingress ring and draining the egress ring happen
  on the same thread outside the timed region. No NIC, network stack or
  kernel I/O is included, so the result is a software path cost, not a
  wire-to-wire latency, and it is not comparable to published wire-to-wire
  figures.
* Values are batch means (default 4,096 messages per batch). The generic
  counter steps in 41.67 ns units and one PMU read costs about 2,500 cycles,
  so single messages cannot be timed; batch percentiles describe batch
  means, not per-message tail latency.
* `full` and `direct` must produce the same intents and checksum; the
  harness aborts if they differ.
* Results of 2026-11-11 (commit c3b850d): `results/2026-11-11_m4.txt`. Code
  changed after that run (hostile-input hardening adds a branch to order
  lookup and to adds), so later figures are measured again, not reused.

## Pre-registered 2026-11-12: tests and decision rules

**H17 (code placement).** `titan-stream` is built three times from the same
commit: default code layout; every function and every non-fall-through basic
block aligned to 64 bytes (`-C llvm-args=-align-all-functions=6
-C llvm-args=-align-all-nofallthru-blocks=6`); and the same at 128 bytes
(`=7`). The instruction census is repeated for each build to confirm the
ordering instructions are unchanged. Decision: H17 is supported if the sign
of SeqCst minus AcqRel (cached and uncached streaming, PMU cycles per
message) differs between layouts or its size changes by more than the
within-layout bootstrap interval; it is refuted if SeqCst is at least 2%
cheaper with p < 0.05 in all three layouts.

**H21 (pre-trade checks).** `titan-t2t stages=full,risk limits=open pmu`:
every intent passes all eight checks and is sent, so the two stages emit
identical orders (the harness aborts otherwise) and differ only by the
checks. Cost per checked order = (risk minus full cycles per message) times
messages divided by orders. A second run with the default limits records the
reject mix; it is descriptive. The checks follow the kinds of control the
Market Access Rule (SEC Rule 15c3-5) asks for: kill switch, order size,
price collar, order notional, position, credit (open exposure), duplicate
order and order rate. Limits are illustrative, not a firm's settings.

**H22 to H25 (`titan-pipeline`).** Two threads at QoS USER_INTERACTIVE; the
feed thread releases message i at start + schedule[i] (spinning on the
counter) and pushes its descriptor; the trading thread pops, decodes, updates
the book and runs the signal, then records completion minus schedule[i].
Because latency is taken from the schedule, a late feed thread or a full ring
adds to the measured latency (no coordinated omission). Synthetic feed,
2,000,000 messages, offered loads 1, 5, 10, 15, 20 and 25 million messages
per second, waits spin, isb and wfe, 5 epochs, wait order rotated per epoch
and load. A run is marked saturated when achieved throughput is below 97% of
offered; saturated runs are excluded from H22 to H25. Values are quantised to
the 41.67 ns counter step. Real data (NASDAQ BX 2019-07-30, 09:30:00 plus 30 s
of feed time, replayed at the feed's own timestamps) is exploratory and
carries no hypothesis.
