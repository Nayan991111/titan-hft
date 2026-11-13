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
| H17 | SeqCst at or below AcqRel cost (seen in three runs) is a code placement effect, not an ordering effect: it disappears when loop alignment is fixed | partly supported | Retest 2026-11-12 on wall power, 10 PMU epochs per layout, identical instruction census. Cached: no difference in any layout (SeqCst/AcqRel 1.013, 0.977, 0.997; p 0.65 to 0.91). Uncached: no difference in the default layout (1.008, p = 0.33) but SeqCst 11.9% and 20.3% cheaper with 64 and 128-byte alignment (p = 0.0001 and 0.0005). The effect depends on layout, so the pre-registered rule (size change beyond the within-layout interval) is met and the effect is not a fixed cost of the ordering instructions. The stated prediction failed: the advantage does not disappear with fixed alignment, it appears and grows with it. SeqCst uncached epochs spread from 162 to 222 cycles in the aligned layouts while AcqRel stays at 215 to 224. In the first pass SeqCst also executed fewer instructions per message (28.4 and 28.7 vs 29.9), which points at the wait loop; mechanism open. First pass (3 epochs, battery) was inconclusive |
| H18 | Single-core software tick-to-trade (`titan-t2t` stage `full`, synthetic default mix) costs more than 30 ns per message; predicted 45 to 60 ns | supported; predicted range missed (low) | 42.7 ns (bootstrap 95% CI 42.7 to 42.9), 189 cycles per message; real BX data 31.8 ns (31.8 to 32.2), 135 cycles |
| H19 | Ring operations add less than 10% to the trading path: `full` within 10% of `direct` in cycles per message | supported | +7.8% cycles synthetic (189 vs 176), +6.7% real (135 vs 127) |
| H20 | Component ablation is subtractive: stage `book` costs no more cycles per message than stage `full` | supported on M4 | 169 vs 189 cycles synthetic, 119 vs 135 real, 5 of 5 epochs each. The inversion seen first on the x86 development VM did not reproduce: on the VM `book` was slower than `full` (115 to 142 vs 94 to 123 ns per message) while executing fewer instructions (223 vs 274 per message, callgrind) |
| H21 | With limits no order can breach, the eight pre-trade checks (`risk` minus `full`) cost under 60 cycles per checked order on both feeds | not supported (method flawed on the synthetic feed) | Real data 28 cycles per order, synthetic 1,196. On the synthetic feed (0.5% of messages emit an order) the subtraction charges the checks with about 5 instructions of code-generation overhead on every message (236 to 241 per message), most of which never reach the checks. On real data (9.7% emit an order) the difference, 11 instructions per message (188 to 199), matches the checks own cost (115 instructions x 0.0968 = 11.1), and 28 cycles per order is close to the isolated 21.1 of H21b. Whole-path cost: +2.0% cycles per message on real data, +3.3% synthetic (5 of 5 epochs, p = 0.008). Replaced by H21b |
| H22 | Two-core pipeline at 1M msg/s offered (synthetic): median scheduled-arrival-to-decision latency is between 80 and 250 ns for spin, isb and wfe | supported | 179 (spin), 174 (isb), 171 (wfe) ns |
| H23 | At 1M msg/s the p99.99 exceeds 1 us for every wait strategy: interrupts and scheduling, not the data path, set the extreme tail | supported (threshold only) | 10.2 to 10.6 us for all three waits. The mechanism in the claim (interrupts and scheduling) was not tested |
| H24 | The median latency with wfe is within one counter step (41.67 ns) of spin at every load that does not saturate | supported | 8, 5 and 0 ns apart at 1M, 5M and 10M msg/s; wfe and isb are slightly faster than spin at 1M and 5M (p about 0.01) |
| H25 | The p99 at the highest offered load that does not saturate is more than 5 times the p99 at 1M msg/s | refuted | p99 at 10M msg/s is 1.13x (spin), 1.27x (isb), 1.46x (wfe) the p99 at 1M. Flat to 10M, saturated at 15M with capacity about 12.7M msg/s; loads between 10M and 15M were not tested, so the shape of the transition is not resolved |
| H21b | Measured alone (recorded orders replayed through the checks, PMU over batches), the eight checks cost under 60 cycles per order on both feeds | supported | Checks alone, 5 of 5 epochs: 21.1 cycles (5.15 ns) per order on real data, 17.0 cycles (3.80 ns) synthetic; replay floor 4.5 and 4.3 cycles; 115 to 117 instructions per order at an IPC of 5.5 to 6.9; 0.18 and 0.05 branch mispredicts per order. Over 1,936,537 real orders. Wall power |
| H26 | On the real BX open, messages that arrive within 100 ns of the previous one (bursts) have a higher median latency than messages that arrive 1 to 10 us after it: queueing inside bursts sets the p99 | partly supported | Messages under 100 ns after the previous one: median 165 to 177 ns vs 107 to 115 ns for 1 to 10 us gaps, all three waits, 5 of 5 epochs (p about 0.008). But the class holds 193 of 198,807 messages (0.1%), too few to set the p99; the p99 is 1.9 to 3.6 us in every recorded gap class (the 10 to 100 us class, about 44% of messages, was not recorded). Bursts are slower, but they are not the cause of the real-pacing tail |
| H27 | Messages that arrive more than 100 us after the previous one have a higher median latency than messages arriving 1 to 10 us after it: an idle core pays to wake (cold caches, predictors or clock) | refuted | Over 100 us gaps: median 98 to 112 ns vs 107 to 115 ns for 1 to 10 us gaps, 0 of 5 epochs higher. Not at the median; the p99 of that class is 1.30 to 1.35x the 1 to 10 us p99 (5 of 5 epochs, p = 0.008), so waking from idle shows in the tail only |
| H28 | The first 100,000 messages of the BX open replayed at a constant 1M msg/s have a p99 below 1 us for all three waits: the real book is not what makes the real-pacing tail | supported | p99 183 (isb), 206 (wfe), 258 (spin) ns at a constant 1M msg/s on the real messages; median 103 to 109 ns. The real book does not make the tail |
| H29 | The synthetic feed at a constant 6,627 msg/s (the mean rate of the real 30 s window) has a p99 above 1 us for all three waits: low utilisation alone produces the tail | supported | Synthetic feed at 6,627 msg/s: p99 1,888 (wfe), 2,064 (spin), 2,153 (isb) ns. For comparison (different run, battery): 333 to 360 ns at 1M msg/s and median 171 to 179 ns; here the median is 574 to 926 ns |
| H30 | The same 100,000 real messages at a constant 6,627 msg/s have a p99 within 1.5x of their p99 under the real timestamps: the arrival structure adds little beyond the rate | invalid (design error) | The rate used, 6,627 msg/s, is the mean of the earlier 30 s window, not of these 100,000 messages: they span 7.535 s of feed time, a mean of 13,271 msg/s. The comparison therefore mixes a 2x rate difference with the arrival structure and cannot test the claim. Recorded values: p99 at a constant 6,627 msg/s 2,173 (wfe), 2,331 (spin), 2,961 (isb) ns; under the real timestamps 1,873, 1,931, 1,886 ns. To be rerun at 13,271 msg/s |
| H31 | Encoding an order as a FIX 4.2 NewOrderSingle costs at least 3x the cycles of an OUCH 4.2 Enter Order, on both feeds; predicted 4 to 7x | supported | FIX 4.2 vs OUCH 4.2 per order: 124.7 vs 28.7 cycles (4.34x) real, 114.6 vs 22.2 (5.17x) synthetic, 5 of 5 epochs; FIX 832 instructions and 0.87 branch mispredicts per order, OUCH 209 and 0.0003; replay floor 4.5 cycles |
| H32 | At 6,627 msg/s the trading path's own L1D load misses per message (`full` minus `none`, same wait and rate, consumer PMU) are at least 1.5x those at 1M msg/s, for wfe and spin, on the synthetic feed: cache contents decay between sparse messages | refuted | Synthetic, path L1D load misses per message (full minus none): wfe 9.8 at 1M vs 11.2 at 6,627 msg/s (1.14x), spin 9.5 vs 12.3 (1.29x). Real (not pre-registered): wfe 3.7 vs 6.3 (1.71x), spin 6.0 vs 6.9 (1.15x). Branch mispredicts per message within 0.1 at both rates (synthetic about 3.6, real 2.89 vs 2.95). The few extra L1D misses cannot by themselves add about 2 us; the cost may sit deeper (L2, system cache, memory, TLB), which these counters do not see |
| H33 | With no trading work at all (`path=none`), the p99 at 6,627 msg/s is above 1 us for wfe and spin on both feeds: the hand-off and wake-up, not the data path, set the low-rate tail | refuted | path=none at 6,627 msg/s: p99 94 (wfe) and 123 (spin) ns synthetic, 83 and 122 ns real, at or below the same at 1M (123 to 124 ns). The hand-off and wake-up alone are fast. One real epoch (wfe and spin, path=none) shows a stall of about 164 s and a 64 us p99; the section ran about 2.5 min longer than planned, consistent with the machine sleeping; medians over epochs are unaffected, and later scripts keep the machine awake |
| H34 | At 6,627 msg/s the tail is in the service time: pop-to-decision p99 above 1 us while schedule-to-pop p99 stays below 300 ns, isb wait, both feeds | supported | At 6,627 msg/s, keep-warm off: service p99 3,000 ns (synthetic) and 2,958 ns (real); delay-to-pop p99 125 and 115 ns. At 1M msg/s the service p99 is 250 and 125 ns. Millisecond pop stalls occurred in at least 3 of 5 synthetic epochs (median of the per-epoch maximum 2.37 ms); one raised that epoch's pop p99 to about 1.3 ms. The median over epochs is 125 ns |
| H35 | Keeping the book warm while waiting (`warm=book`) lowers the full-path p99 at 6,627 msg/s by at least 2x, both feeds | refuted | warm=book, end-to-end p99 at 6,627 msg/s: 3,050 vs 3,102 ns synthetic (1.02x lower), 2,491 vs 3,073 ns real (1.23x); service p99 2,959 and 2,208 ns |
| H36 | Keeping memory active while waiting without touching the book (`warm=mem`) lowers the full-path p99 at 6,627 msg/s by at least 2x, both feeds | supported | warm=mem, end-to-end p99 at 6,627 msg/s: 618 vs 3,102 ns synthetic (5.0x lower), 626 vs 3,073 ns real (4.9x); service p99 541 and 542 ns. Costs: the delay-to-pop p99 rises to 284 and 285 ns (a message can wait for one unit), and the real-feed median rises from 173 to 289 ns (synthetic falls from 527 to 312). The outcome, memory helps and the book does not, was not one of the three pre-registered readings. By design the mem units are loads meant to miss the system cache, so they probably keep the core busy longer than the book units (unit duration was not recorded); memory activity and core activity are therefore not separated; neither is identified by these counters |
| H30b | The first 100,000 messages of the BX open at a constant 13,271 msg/s (their own mean rate: 100,000 over 7.535 s) have a p99 within 1.5x of their p99 under the real timestamps, for all three waits: the arrival structure adds little beyond the rate | supported (noisy run) | Median p99, real timestamps vs constant 13,271 msg/s: wfe 2,598 vs 1,867 ns (1.39x), spin 2,843 vs 2,533 (1.12x), isb 2,896 vs 2,779 (1.04x); all within 1.5x. Medians 112 to 122 ns in all arms. The direction is not established: the identical real-timestamp arms were 1.39 to 1.54x higher here than in the third run (1,873, 1,931, 1,886 ns), a run-to-run change as large as the effect. The run had more browser activity than earlier runs: the three real-timestamp arms and the constant wfe arm show millisecond stalls (p99.99 1.3 to 2.2 ms) and wide intervals (p99 upper bounds 7.3 to 24.8 us), and more messages released over 1 us late (341 to 594 vs 121 to 180). To be repeated in the final protocol on a quiet machine |
| H30c | Repeat of H30b on a quiet machine (one browser window, no other applications): p99 under the real timestamps within 1.5x of the p99 at a constant 13,271 msg/s, for all three waits | supported | Quiet machine (VS Code and one browser window). Median p99, real timestamps vs constant 13,271 msg/s: wfe 1,904 vs 1,865 ns (1.02x), spin 2,157 vs 2,780 (constant 1.29x higher), isb 2,214 vs 2,397 (constant 1.08x higher); all within 1.5x, direction mixed. Medians 103 to 115 ns. Together with H30b: at the same mean rate the arrival pattern changes the p99 by less than 1.5x |
| H37 | One ITCH message per MoldUDP64 datagram over UDP loopback: (a) at 100k msg/s the median latency with a polling receiver is at least 5x the shared-memory pipeline median (isb) at the same rate and window; (b) at 10k msg/s a blocking receiver's median is at least 1.5x the polling receiver's; (c) at 1M msg/s both UDP receivers saturate (achieved below 97% of offered, or messages lost) | (a) not testable; (b) refuted; (c) supported | (a) Every measured UDP run at 100k msg/s was invalid: 5,536 to 7,008 of 100,000 messages never received (no datagrams held back in these runs), 16 of 20 runs ending on the receive timeout, achieved 58k to 59k msg/s. The comparison at 100k msg/s cannot be made; the UDP path with one message per datagram does not sustain 100k msg/s on this machine. (b) At 10k msg/s, median blocking vs polling: 23,256 vs 20,720 ns real (1.12x), 23,280 vs 20,456 synthetic (1.14x), below 1.5x; blocking is slower in 5 of 5 epochs (p = 0.008). (c) At 1M msg/s every UDP run saturated (achieved 70k to 72k msg/s). Exploratory, at 10k msg/s: polling median 20.7 us vs 112 ns for shared memory on real data (185x), p99 26.5 vs 2.6 us; about 100,000 of 100,000 datagrams were released more than 1 us late (printed rounded as 1e+05), so the send call itself takes over 1 us |
| H38 | Parsing a FIX 4.2 NewOrderSingle: (a) a full parse that validates BodyLength and CheckSum and indexes every field costs at least 2x the cycles of a selective parse of the five fields the order needs; (b) finding field delimiters with NEON compares lowers the full parse cost by at least 1.3x; both feeds | (a) refuted; (b) supported | (a) Full vs selective parse: 464.6 vs 298.5 cycles per message real (1.56x), 460.2 vs 295.1 synthetic (1.56x), below 2x. (b) NEON delimiters: full parse 464.6 to 301.1 cycles real (1.54x), 460.2 to 296.6 synthetic (1.55x); 5 of 5 epochs, p = 0.008. With NEON the validated full parse costs within 1% of the unvalidated selective one (301.1 vs 298.5 cycles real; not tested). 1,936,537 real messages, all parsed back exactly |
| H39 | Audit trail of every risk decision: (a) a 32-byte binary record per order adds under 10 cycles per order to the checks; (b) a formatted text record adds at least 10x the binary record's cost, (median audittext - median checks) / (median auditbin - median checks) >= 10, with auditbin above checks (p < 0.05); both feeds, median `msg_cycles` over epochs | supported | (a) Binary record: 25.98 vs 20.75 cycles per order real (+5.2), 20.71 vs 17.13 synthetic (+3.6), p = 0.008. (b) Text record adds 302.8 cycles real and 299.7 synthetic: 58x and 84x the binary record, and 15x (real) to 17x (synthetic) the cost of the eight checks themselves |
| H40 | An online VPIN update in volume time, with each execution signed exactly from the resting order's side in the L3 book, costs under 30 cycles per signed execution (median `msg_cycles` of vpin minus that of vpinfloor), both feeds | supported | vpin minus vpinfloor: 23.81 - 1.59 = 22.2 cycles per signed execution real (681,693 executions, whole day), 17.12 - 1.52 = 15.6 synthetic (medians over 5 epochs) |
| H41 | Replication on BX 2019-07-30 (instruments with at least 100 signed executions): bulk volume classification (1-minute bars, normal CDF, per-instrument standard deviation of bar price changes) misclassifies a larger share of volume than the tick rule, both measured per 1-minute bar | supported (replication) | 1,440 instruments, 71.1 million shares compared. Bar-level misclassified volume: BVC 36.7%, tick rule 28.3%; BVC worse in 1,153 of 1,440 instruments. Tick rule trade by trade: 30.1% of volume wrongly signed; 333 executions unresolved. Excluded: 243,778 trades on non-displayed orders; non-printable C executions: 0 on BX that day |
| H42 | Private copies (`titan-fanout work=replay`, one ring per reader): X(copy3) >= 2 X(copy1), and copy3 above copy1, both feeds | pending | |
| H43 | One shared seqlock snapshot per instrument (`work=replay`): (a) the first reader costs the writer, snap1 above snap0; (b) more readers add little, X(snap3) <= 1.5 X(snap1); both feeds | pending | |
| H44 | At the highest update rate (`work=replay`) and 3 readers, the shared snapshot costs the writer less than private copies: snap3 below copy3, both feeds | pending | |
| H45 | At the pace of the real feed (`work=book`, BX) and 3 readers, the order reverses: snap3 above copy3 | pending | |
| H46 | At the pace of the feed (`work=book`), the median staleness of the updates readers take differs between the designs by at most 42 ns (one counter step) at 1, 2 and 3 readers, both feeds | pending | |
| H47 | At the pace of the feed (`work=book`, 1 reader), a snapshot reader misses at least 1% of the versions published on the real feed and under 1% on the synthetic feed | pending | |
| H48 | Profile-guided optimisation of `titan-t2t`, stage `full`: (a) a profile from the same feed lowers cycles per message by at least 3% against the default build; (b) a profile from the other feed gives at least half of that reduction; (c) the same-feed profile changes cycles more than aligning functions and non-fall-through blocks to 64 bytes (the H17 layout) does; both feeds | pending | |

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

## Pre-registered 2026-11-12, fifth run

**H34 to H36 (where the low-rate tail lives).** H32 and H33 rule out the
hand-off, the wake-up, branch prediction and most of the L1D story. H34
splits each message's latency into the delay to pop and the service time
(`split`, one more counter read per message). H35 and H36 are the
intervention test: while waiting, the consumer runs read-only keep-warm
units every 20 us, polling the ring between units so a message waits for
at most one unit. `book` touches the best levels of the 64 most recently
touched instruments and decodes the last message; `mem` loads from a
128 MiB scratch buffer with a stride that defeats prefetching and never
touches the book. Reading of the outcomes: book helps and mem does not,
book state going cold (caches or TLB); both help, the memory system's power
state; neither helps, something else (for example core or fabric clocks).
Design: waits=isb, rates 1M and 6,627 msg/s, first 100,000 messages of
each feed (synthetic; BX from 09:30:00), 5 epochs, arms off, book and mem.
Verdicts on medians over epochs of the p99. The machine is kept awake for
the whole script (`caffeinate`).

### Note on the fifth run

`split` adds a counter read per message, so its end-to-end values are
compared only within the run. With keep-warm off, the synthetic p99 at
6,627 msg/s is 3,102 ns here and 2,153 ns in the third run (isb, no
`split`); the real one is 3,073 and 2,961 ns. The cause of the synthetic
difference (the extra read or run-to-run variation) is not separated; the
H34 to H36 verdicts compare arms of the same run only.

### Errata, 2026-11-12 (independent audit of H17 to H36)

* H30: the pre-registration above calls 6,627 msg/s the mean rate of the
  100,000-message window; it is the mean of the earlier 30 s window. The
  window's own mean is 13,271 msg/s. H30 is marked invalid and will be
  rerun. H28 does not depend on this number. H29 used 6,627 msg/s where its
  pre-registration says the real window's mean rate; it remains a valid
  low-rate test, at half the rate the text implies.
* H17: status changed from supported to partly supported (the rule is met,
  the stated direction is not).
* H21: the 5-instruction overhead applies to the synthetic feed only.
* Pre-registration texts above are left as written; corrections live in the
  table and here.

## Pre-registered 2026-11-12, sixth run

**H30b.** As H30, with the window's own mean rate (13,271 msg/s) instead of
6,627 msg/s. `titan-pipeline file=BX from=34200 limit=100000`, once with
`rates=13271` and once with `speed=1`, waits spin, isb and wfe, 5 epochs,
no `split`, no keep-warm. Verdict on the median over epochs of each wait's
p99.

## Pre-registered 2026-11-13, seventh run

These complete the measurable parts of the first source brief (see the
ledger): the network receive path, inbound FIX parsing, an asynchronous
audit trail, and an order-flow toxicity engine. None of the brief's numbers
are used.

**H30c.** H30b unchanged, on a quiet machine.

**H37 (network receive path).** `titan-pipeline waits=udpblock,udppoll`
(same binary, same schedule and trading path as the shared-memory runs). A
producer thread
releases each message at its scheduled time (same open-loop schedule as
`titan-pipeline`) by sending one MoldUDP64 downstream packet to a UDP socket
on 127.0.0.1 (built before the scheduled time, sent at it; the `send`
call counts in the latency and in `feed_late_over_1us`): 20-byte header (session, 10 bytes; sequence number of the
first message, 8 bytes; message count, 2 bytes) and one message block
(2-byte length plus the ITCH message), as in the NASDAQ MoldUDP64 v1.00
specification. The consumer checks the session and sequence number, counts
gaps, then runs the same trading path (decode, book, signal) on the
received bytes and records completion minus schedule. Receivers: `udpblock`
(blocking `recv`, 1 s timeout) and `udppoll` (non-blocking `recv` in a
loop). Rates 10k, 100k and 1M
msg/s; windows as H28 (BX from 09:30:00, first 100,000 messages) and the
synthetic feed (first 100,000); 5 epochs. The shared-memory baseline for
H37a is `titan-pipeline` with `waits=isb` on the same windows and rates,
in the same script. Receive buffer: 8 MiB requested (`SO_RCVBUF`), halved
until the kernel accepts it; the granted size is printed with every run
(`udp_rcvbuf`). Lost datagrams appear as gaps and are reported; a run with
loss beyond deliberate drops, or ending on the 1 s receive timeout, is
marked saturated and left out of comparisons. A correctness check (not a
hypothesis), in its own output file, holds back every 1,000th datagram and
requires the gaps detected to be at least the datagrams held back (any
excess is real loopback loss and is printed). The cost of `sendto` sits inside the measured latency,
as it does for a real feed handler's upstream.

**H38 (inbound FIX).** `titan-t2t stages=fixfull,fixsel,fixneon pmu`.
Messages are FIX 4.2 NewOrderSingle built (untimed) from the recorded orders
with the H31 encoder. `fixfull` validates BeginString, BodyLength and CheckSum,
indexes every field (tag and value span) and reads the five order fields
(35 and 54 as characters, 55 as a string, 38 and 44 as integers, 44 with
up to 4 decimals); `fixsel` scans only for those five (tags must be digits),
with no BeginString, BodyLength or CheckSum validation; `fixneon` is `fixfull` with delimiter (SOH) positions found
16 bytes at a time with NEON compares. All three must return the recorded
order exactly (the harness aborts otherwise). Per message, PMU cycles.

**H39 (audit trail).** Stages `auditbin` and `audittext` run the checks of
H21b with open limits and append one record per order: `auditbin` writes 32
bytes (feed time, sequence, locate, side, price, shares, decision) into a
preallocated ring; `audittext` formats the same fields as one text line with
`core::fmt` into a reusable buffer (no allocation after warm-up). Added cost
= stage minus `checks`, PMU cycles per order; the audit stages fold the
same fields into the checksum as `checks`, so the difference is the record. A real system drains the ring
on another core; only the hot-path cost is measured.

**H40 (toxicity).** An untimed pass records every displayed execution (`E`,
and `C` with Printable = 'Y') with its aggressor side, the opposite of the
resting order's side, read from the L3 book before the execution is
applied. Excluded and counted: `P` trades on non-displayed orders (no
usable side) and non-printable `C` executions (crosses, which have no
aggressor). The timed stage `vpin` feeds these
records to a per-instrument VPIN in volume time (bucket size: 1/50 of the
instrument's executed volume that day, from the untimed pass; 50 buckets;
a large execution may fill several buckets). Floor stage `vpinfloor` runs
the same loop, reading each record (`black_box(*e)`), with no update. Per signed execution,
PMU cycles. Easley, Lopez de Prado and O'Hara, Review of Financial Studies
25(5), 2012, doi:10.1093/rfs/hhs053.

**H41 (classification accuracy, replication).** From the same untimed pass
on the real feed: true buy volume per instrument and 1-minute bar from the
exact signs; BVC estimate V x Phi(dP / sigma), dP the change of the last
price from the previous bar that had executions (not necessarily the
previous minute) and sigma the population standard deviation of those
changes for the instrument over the day; instruments with a single bar are
dropped; both methods see displayed executions only; tick rule per execution (uptick buy, downtick sell, zero tick
keeps the previous sign; an execution with no earlier non-zero tick counts
half buy and half sell). Each instrument's first 1-minute bar has no price
change for BVC and is excluded from both methods. The real feed is read in
full (`limit=100000000`) for H40 and H41.
Misclassified volume, the same measure for both: sum over bars of
|estimated minus true buy volume|, divided by total signed volume, with the
tick rule's buy volume aggregated into the same bars (errors inside a bar
can cancel for both). The tick rule's trade-level error (volume of wrongly
signed executions over total) is reported as well, but is not part of the
verdict. Chakrabarty, Pascual and Shkilko,
Journal of Financial Markets 25, 2015, doi:10.1016/j.finmar.2015.06.001.

### Note on the 2026-11-13 run

The gap-detection check (H37 correctness, every 1,000th datagram held back)
ran without aborting, but its summary failed to print (BSD `paste` needs a
file argument), and at 100k msg/s real loss alone satisfies its assertion
(missing at least held back). Gap detection is therefore validated only on
the development machine (exact equality of missing and held back) and
needs a rerun on the M4 at 10k msg/s with the held-back count printed. The
per-epoch UDP lines in the results file are from the measured runs.

## Pre-registered 2026-11-13, eighth run

These are the last two measurable items of the first source brief: its
multi-strategy deployment (several strategies reading one shared order
book, Day 13) and its profile-guided optimisation item. None of the brief's
numbers are used. All verdicts of this run are preliminary in the sense of
the Protocol (epochs inside one process, not 30 separate launches).

**H42 to H47 (one book, several strategy threads).** `titan-fanout`, new in
this batch. The writer is the thread that owns the L3 book (main thread, QoS
USER_INTERACTIVE). It subscribes the K = 8 instruments with the most book
messages in the window and publishes their best bid and ask (price and size,
one word per side) whenever either changes. N reader threads (QoS
USER_INTERACTIVE, busy polling) take the updates; for every update it takes,
each reader reads the counter, records the staleness in a histogram and
folds the content, in both designs; the bookkeeping that differs is the
version check (copy) and the conflation count (snap).
Designs: `none` (update built and discarded with `black_box`, one 32-byte
stack store: the floor); `copy` (one titan-spsc ring per reader, AcqRel,
128-byte index padding, cached indices, 4,096 slots of 32 bytes; the writer
pushes every update into every ring and spins while one is full, so copies
are lossless and ordered); `snap` (one seqlock slot per instrument in its
own 128-byte line, fence-based form of Boehm, MSPC 2012,
doi:10.1145/2247684.2247688; the writer overwrites it, every reader polls
all K slots, and versions a reader never sees are counted as conflated).
Every design, `none` included, reads the counter once per update for the
staleness stamp, so that cost is part of the floor. The writer's state
struct and the readers' flags each sit in their own 128-byte lines (their
small heap buffers are ordinary allocations), and every page of each
reader's histogram is touched before the pass starts.
Configs per epoch, order rotated: none, snap0, copy1, snap1, copy2, snap2,
copy3, snap3. Three readers plus the writer is the number of performance
cores (4); macOS offers no thread pinning, so QoS is the only placement
control. `invol_switches` counts the whole process (writer, readers and
thread start-up) from before the readers start until they are joined.
Two kinds of work: `work=book` replays the feed (decode, book, publish) as
fast as the writer can, values per feed message; `work=replay` publishes
the recorded updates back to back, repeated to at least 2,000,000 per
pass, with no feed and no book, values per update. In `work=replay` a copy
writer is limited by its slowest reader once a ring fills, so H42 and H44
measure the writer including that wait; `full_waits` (pushes that found a
ring full) and `wait_spins` (spin iterations while waiting) are reported
to show how much of the cost is waiting.
Feeds: BX 2019-07-30, first 20,000,000 messages, and the synthetic feed
(5,000,000 messages, 500 instruments). 10 epochs in one process per feed
and work. Writer metric: PMU cycles on the writer thread over the whole
pass (`msg_cycles`); the writer hypotheses are evaluated only from PMU
cycles, never from nanoseconds, and are not evaluated if the PMU is
unavailable. Cost per published update above the floor:
X(d, N) = (median msg_cycles of the config minus median of `none`) / q,
with q the updates per unit (quotes over messages; 1 in `work=replay`).
Staleness: the counter when a reader holds a consistent copy minus the
writer's counter just before publishing (counter shared by all cores,
41.67 ns step, printed values quantised to 41 or 42 ns); the staleness
values of all readers of a run are pooled, and a snap reader's staleness
covers only the versions it took, never the ones it skipped.
Decision rules. "A above B" and "A below B" mean a two-sided Mann-Whitney U
test on the per-epoch `msg_cycles` of the two configs (10 against 10) with
p < 0.05 and the medians in the stated direction. A ratio of X values is
evaluated on the medians as defined above and is undefined (hypothesis not
evaluated) if the bootstrap 95% interval of the denominator reaches zero.
A hypothesis stated for both feeds is supported if it holds on both,
partly supported if on one, refuted if on neither. H46 is supported if all
six differences of the medians over epochs of `stale_p50_ns` (N = 1, 2, 3
on two feeds) are at most 42 ns, partly supported if three to five are,
refuted otherwise. H47 uses the median over epochs of `conflated_frac` at
snap1; it is supported if both parts hold, partly supported if one does.
The harness aborts unless: the recorded updates match the reference pass
of the feed (count, versions and final top of book of every slot); every
run publishes that count, those versions and that final top of book; every
copy reader receives every update in version order with content equal to
the recorded sequence (fold over all updates); every snap reader's taken
plus conflated versions equal those published; every reader ends holding
the final top of book of every slot; and, in the unmeasured warm-up pass of
every config, every reader checks the content of every update it takes
against the recorded sequence (switched off in measured passes; a snap
reader takes only some versions, so for snap the check covers the versions
taken). Mutation tests on the development machine: one corrupted bid at
version 3 makes the copy warm-up abort; for snap, where a single version is
usually skipped, a corrupted bid at every odd version from 3 on in one slot
makes the warm-up abort on the content check (8 of 8 runs, both kinds of
work).
The synthetic feed changes a subscribed top of book on well under 1% of
messages (0.02% to 0.2% on the development machine), so in `work=book` its
per-update writer cost is too diluted to estimate; that is why the writer
hypotheses use `work=replay`, and H45 uses the real feed only.
Reasoning recorded before the run: in `snap`, the release fence after the
odd sequence store (expected to compile to `dmb ish`) may have to wait for
that store to become visible, which needs the line back from every reader
that polls it; in `copy`, the stores go to the store buffer with no full
barrier, but the writer must wait for the slowest reader whenever a ring
is full, as it will be at the replay rate. `scripts/fanout_census.sh`
disassembles the six writer loops (`write_pass` and `replay_pass` for each
design) and counts `dmb`, `isb`, `stlr`, `ldar` and `ldapr` per loop with
each loop function's entry address, so the barrier claim is checked rather
than assumed. Code layout differs between the separately compiled loops
(H17 showed layout effects of up to 20%); the entry addresses are
reported, and no alignment control is run for these hypotheses.

**H48 (profile-guided optimisation).** `titan-t2t stages=decode,book,full
pmu`, built four times from the same commit with `-C target-cpu=native`
and the release profile, each in its own target directory, each binary
printing its build name (`build=`): `base` (default); `align64` (every
function and every non-fall-through block aligned to 64 bytes, the H17
flags); `pgo_syn` and `pgo_real` (`-C profile-use` with the profile of an
instrumented build, `-C profile-generate`, run once with stages decode,
book and full on the synthetic feed, 5,000,000 messages, or on the first
20,000,000 BX messages; raw profiles merged with the `llvm-profdata` of the
toolchain's own `llvm-tools` component). Training and measurement use the
same windows, so the same-feed profile is the in-sample case. Each build is
measured on both feeds in 2 processes of 5 epochs, builds interleaved
within each round: 10 epochs per build and feed. Verdicts on the median
over epochs of `msg_cycles` of stage `full`; `decode` and `book` are
reported. Gain G(b) = (median base minus median b) / median base. (a)
holds on a feed if G(same-feed build) >= 0.03 and the build is below base
(two-sided Mann-Whitney, p < 0.05). (b) is evaluated only on a feed where
(a) holds: G(other-feed build) >= G(same-feed build) / 2. (c) holds on a
feed if |median same-feed build minus median base| is larger than
|median align64 minus median base|. Each part is supported if it holds on
both feeds, partly supported on one. If the `llvm-tools` component cannot
be installed on the measurement machine, H48 is not run and stays pending.

**Gap-detection check (H37 correctness, rerun).** `titan-pipeline
limit=100000 rates=10000 waits=udppoll drop_every=1000 epochs=1` on the
synthetic feed and on BX from 09:30:00: every 1,000th datagram held back,
so 100 per run. Passes if each receiver reports exactly 100 missing
(`udp_missing`), with no receive timeout; any excess is real loopback loss
and is printed.
