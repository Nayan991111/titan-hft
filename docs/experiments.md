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
| H17 | SeqCst at or below AcqRel cost (seen in three runs) is a code placement effect, not an ordering effect: it disappears when loop alignment is fixed | supported | Retest 2026-11-12 on wall power, 10 PMU epochs per layout, identical instruction census. Cached: no difference in any layout (SeqCst/AcqRel 1.013, 0.977, 0.997; p 0.65 to 0.91). Uncached: no difference in the default layout (1.008, p = 0.33) but SeqCst 11.9% and 20.3% cheaper with 64 and 128-byte alignment (p = 0.0001 and 0.0005). The sign differs between layouts, so by the pre-registered rule the effect is code placement, not ordering. SeqCst uncached is bimodal in the aligned layouts (epochs near 165 to 175 or 200 to 222 cycles) while AcqRel stays at 213 to 224; mechanism open. First pass (3 epochs, battery) was inconclusive |
| H18 | Single-core software tick-to-trade (`titan-t2t` stage `full`, synthetic default mix) costs more than 30 ns per message; predicted 45 to 60 ns | supported; predicted range missed (low) | 42.7 ns (bootstrap 95% CI 42.7 to 42.9), 189 cycles per message; real BX data 31.8 ns (31.8 to 32.2), 135 cycles |
| H19 | Ring operations add less than 10% to the trading path: `full` within 10% of `direct` in cycles per message | supported | +7.8% cycles synthetic (189 vs 176), +6.7% real (135 vs 127) |
| H20 | Component ablation is subtractive: stage `book` costs no more cycles per message than stage `full` | supported on M4 | 169 vs 189 cycles synthetic, 119 vs 135 real, 5 of 5 epochs each. The inversion seen first on the x86 development VM did not reproduce: on the VM `book` was slower than `full` (115 to 142 vs 94 to 123 ns per message) while executing fewer instructions (223 vs 274 per message, callgrind) |
| H21 | With limits no order can breach, the eight pre-trade checks (`risk` minus `full`) cost under 60 cycles per checked order on both feeds | not supported (method flawed) | Real data 28 cycles per order, synthetic 1,196. The subtraction also charges the checks with a code-generation overhead of about 5 instructions on every message (188 to 199 per message real, 236 to 241 synthetic), most of which never reach the checks. Whole-path cost: +2.0% cycles per message on real data, +3.3% synthetic (5 of 5 epochs, p = 0.008). Replaced by H21b |
| H22 | Two-core pipeline at 1M msg/s offered (synthetic): median scheduled-arrival-to-decision latency is between 80 and 250 ns for spin, isb and wfe | supported | 179 (spin), 174 (isb), 171 (wfe) ns |
| H23 | At 1M msg/s the p99.99 exceeds 1 us for every wait strategy: interrupts and scheduling, not the data path, set the extreme tail | supported | 10.2 to 10.6 us for all three waits |
| H24 | The median latency with wfe is within one counter step (41.67 ns) of spin at every load that does not saturate | supported | 8, 5 and 0 ns apart at 1M, 5M and 10M msg/s; wfe and isb are slightly faster than spin at 1M and 5M (p about 0.01) |
| H25 | The p99 at the highest offered load that does not saturate is more than 5 times the p99 at 1M msg/s | refuted | p99 at 10M msg/s is 1.13x (spin), 1.27x (isb), 1.46x (wfe) the p99 at 1M; no gradual knee: flat to 10M, then saturation, with capacity about 12.7M msg/s |
| H21b | Measured alone (recorded orders replayed through the checks, PMU over batches), the eight checks cost under 60 cycles per order on both feeds | supported | Checks alone, 5 of 5 epochs: 21.1 cycles (5.15 ns) per order on real data, 17.0 cycles (3.80 ns) synthetic; replay floor 4.5 and 4.3 cycles; 115 to 117 instructions per order at an IPC of 5.5 to 6.9; 0.18 and 0.05 branch mispredicts per order. Over 1,936,537 real orders. Wall power |
| H26 | On the real BX open, messages that arrive within 100 ns of the previous one (bursts) have a higher median latency than messages that arrive 1 to 10 us after it: queueing inside bursts sets the p99 | partly supported | Messages under 100 ns after the previous one: median 165 to 177 ns vs 107 to 115 ns for 1 to 10 us gaps, all three waits, 5 of 5 epochs (p about 0.008). But the class holds 193 of 198,807 messages (0.1%), too few to set the p99; the p99 is 1.9 to 3.6 us in every recorded gap class (the 10 to 100 us class, about 44% of messages, was not recorded). Bursts are slower, but they are not the cause of the real-pacing tail |
| H27 | Messages that arrive more than 100 us after the previous one have a higher median latency than messages arriving 1 to 10 us after it: an idle core pays to wake (cold caches, predictors or clock) | refuted | Over 100 us gaps: median 98 to 112 ns vs 107 to 115 ns for 1 to 10 us gaps, 0 of 5 epochs higher. Not at the median; the p99 of that class is 1.30 to 1.35x the 1 to 10 us p99 (5 of 5 epochs, p = 0.008), so waking from idle shows in the tail only |
| H28 | The first 100,000 messages of the BX open replayed at a constant 1M msg/s have a p99 below 1 us for all three waits: the real book is not what makes the real-pacing tail | supported | p99 183 (isb), 206 (wfe), 258 (spin) ns at a constant 1M msg/s on the real messages; median 103 to 109 ns. The real book does not make the tail |
| H29 | The synthetic feed at a constant 6,627 msg/s (the mean rate of the real 30 s window) has a p99 above 1 us for all three waits: low utilisation alone produces the tail | supported | Synthetic feed at 6,627 msg/s: p99 1,888 (wfe), 2,064 (spin), 2,153 (isb) ns, against 333 to 360 ns at 1M msg/s; the median also rises, to 574 to 926 ns from 171 to 179 |
| H30 | The same 100,000 real messages at a constant 6,627 msg/s have a p99 within 1.5x of their p99 under the real timestamps: the arrival structure adds little beyond the rate | not supported | Real messages at a constant 6,627 msg/s vs their real timestamps: p99 ratio 1.16 (wfe), 1.21 (spin), 1.57 (isb); isb exceeds 1.5x. In all three waits the constant schedule is the slower one: the real arrival structure does not add to the tail, the low rate does |
| H31 | Encoding an order as a FIX 4.2 NewOrderSingle costs at least 3x the cycles of an OUCH 4.2 Enter Order, on both feeds; predicted 4 to 7x | supported | FIX 4.2 vs OUCH 4.2 per order: 124.7 vs 28.7 cycles (4.34x) real, 114.6 vs 22.2 (5.17x) synthetic, 5 of 5 epochs; FIX 832 instructions and 0.87 branch mispredicts per order, OUCH 209 and 0.0003; replay floor 4.5 cycles |
| H32 | At 6,627 msg/s the trading path's own L1D load misses per message (`full` minus `none`, same wait and rate, consumer PMU) are at least 1.5x those at 1M msg/s, for wfe and spin, on the synthetic feed: cache contents decay between sparse messages | pending | |
| H33 | With no trading work at all (`path=none`), the p99 at 6,627 msg/s is above 1 us for wfe and spin on both feeds: the hand-off and wake-up, not the data path, set the low-rate tail | pending | |

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

## Pre-registered 2026-11-12, second run

**H21b.** `titan-t2t stages=checks`: an untimed `full` pass records every
order with its mid and feed time; the timed part replays only those orders
through the eight checks with limits nobody breaches, in batches of 4,096,
PMU around each batch. Reported per order. Stage `replay` runs the same loop
with the checks replaced by a fold of the order fields; it is reported as
the loop and load floor, and H21b is judged on `checks` itself (the floor
only makes the verdict more conservative). Both stages must accept every
recorded order (the harness aborts otherwise).

**H26, H27.** `titan-pipeline` records each message into one of four
histograms by its scheduled gap to the previous message: under 100 ns,
100 ns to 1 us, 1 to 10 us, over 100 us (10 to 100 us is counted in the
totals only). The extra histogram update runs after the latency is read,
so it does not delay the message it records, but it adds work before the
next pop; totals from `gaps` runs are not compared with runs without it.
Same window as before (BX 2019-07-30, 09:30:00 plus 30 s, real
timestamps, speed 1), 3 waits, 5 epochs. A hypothesis is supported if the
stated class has the higher median in all three waits in at least 4 of 5
epochs.

**H17 retest.** Same three layouts, PMU epochs raised from 3 to 10 so the
pre-registered rule can be evaluated; per-epoch cycles are printed for
SeqCst and AcqRel, 128-byte padding, cached and uncached, isb wait.

## Pre-registered 2026-11-12, third run

**H28 to H30 (data versus rate versus arrival structure).** The real-pacing
p99 (about 2.2 us) is about 6 times the synthetic constant-rate p99 at
1M msg/s, and H26 showed bursts are too rare to explain it. Three cells
separate the candidate causes: real data at a constant 1M msg/s (H28),
synthetic data at the real window's mean rate (H29), and real data at that
mean rate against the same messages at their real timestamps (H30). Real
window: BX 2019-07-30 from 09:30:00, first 100,000 messages. Synthetic:
default mix, first 100,000 messages. `titan-pipeline gaps`, 3 waits, 5
epochs. Verdicts on the median over epochs of each wait's p99.

**H31 (order encoding).** `titan-t2t stages=replay,ouch,fix pmu` on the
orders recorded from both feeds. OUCH 4.2 Enter Order follows the published
layout (49 bytes, big-endian integers, 4 implied price decimals); FIX 4.2
NewOrderSingle carries the standard header (8, 9, 35, 49, 56, 34, 52), the
order fields (11, 21, 55, 54, 60, 38, 40, 44) and the trailer (10), with
BodyLength and CheckSum computed per message. Both use the same digit
loop and allocate nothing; tests check every field. Ratio on PMU cycles per
order.

## Pre-registered 2026-11-12, fourth run

**H32, H33 (mechanism of the low-rate tail).** H28 to H30 located the
real-pacing tail in the low offered rate. Two runs per feed (synthetic, and
the real window of H28), each at 1M and 6,627 msg/s, waits wfe and spin, 5
epochs, with the consumer thread's counters over each run (`pmu`): one with
the trading path (`path=full`) and one that only pops and records
(`path=none`). Counters include waiting and kernel work done on the thread;
subtracting `none` from `full` at the same wait and rate removes both.
H32 is judged on medians over epochs of L1D load misses per message; H33 on
medians over epochs of the p99.
