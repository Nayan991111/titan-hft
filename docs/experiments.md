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
| H1 | Ping-pong: AcqRel and SeqCst within 5% | supported | -1.7% to +3.9%. PRELIMINARY PROTOCOL: measured with 3 to 5 epochs inside one process, not the independent-launch protocol, so the launch-to-launch component is unmeasured. Data in results/2026-11-11_m4_spsc.txt. Quotable only with the protocol stated; re-measure before any claim smaller than the noise floor. REPRODUCED: H64 re-measured this case over 30 independent launches on wall power and agrees to within 3.5 percent, so the preliminary figure stands. RUN LENGTH DEPENDENT: measured with titan-stream at 200,000 messages, where H67 showed the performance core runs at 1.085 GHz because the run finishes before the frequency governor responds. The cached conclusions hold at every run length tested. Any UNCACHED ordering comparison from this row does not generalise: at 5,000,000 and 10,000,000 messages SeqCst and AcqRel cost the same to within 1.3 percent and Fenced becomes 2.2x cheaper than AcqRel rather than 3.6x more expensive. See results/2026-11-16_m4_runlength.txt. |
| H1b | Streaming: SeqCst more than 5% slower than AcqRel | refuted | 8.34 vs 8.35 cycles per message. PRELIMINARY PROTOCOL: measured with 3 to 5 epochs inside one process, not the independent-launch protocol, so the launch-to-launch component is unmeasured. Data in results/2026-11-11_m4_spsc.txt. Quotable only with the protocol stated; re-measure before any claim smaller than the noise floor. SUPERSEDED for the streaming case by H61, which measured 1.07x cached and 6.79x uncached over 30 launches; the preliminary run used 10,000,000 messages against H61's 200,000 and never measured SeqCst uncached at all, so the two are not a contradiction but different experiments. H64 shows the ordering is invisible cross-core (0.955 to 1.041) regardless of caching. RUN LENGTH DEPENDENT: measured with titan-stream at 200,000 messages, where H67 showed the performance core runs at 1.085 GHz because the run finishes before the frequency governor responds. The cached conclusions hold at every run length tested. Any UNCACHED ordering comparison from this row does not generalise: at 5,000,000 and 10,000,000 messages SeqCst and AcqRel cost the same to within 1.3 percent and Fenced becomes 2.2x cheaper than AcqRel rather than 3.6x more expensive. See results/2026-11-16_m4_runlength.txt. |
| H2 | Fenced (`dmb ish`) is the slowest ring variant | supported | +7% to +32% ping-pong, 2.6x cycles streaming. PRELIMINARY PROTOCOL: measured with 3 to 5 epochs inside one process, not the independent-launch protocol, so the launch-to-launch component is unmeasured. Data in results/2026-11-11_m4_spsc.txt. Quotable only with the protocol stated; re-measure before any claim smaller than the noise floor. REPRODUCED: H64 re-measured this case over 30 independent launches on wall power and agrees to within 3.5 percent, so the preliminary figure stands. RUN LENGTH DEPENDENT: measured with titan-stream at 200,000 messages, where H67 showed the performance core runs at 1.085 GHz because the run finishes before the frequency governor responds. The cached conclusions hold at every run length tested. Any UNCACHED ordering comparison from this row does not generalise: at 5,000,000 and 10,000,000 messages SeqCst and AcqRel cost the same to within 1.3 percent and Fenced becomes 2.2x cheaper than AcqRel rather than 3.6x more expensive. See results/2026-11-16_m4_runlength.txt. |
| H3 | Ping-pong, uncached: 64-byte padding slower than 128 | not supported | -2.3% to +0.6%. results/2026-11-15_m4_launch_protocol.txt |
| H3b | Streaming, uncached: 64-byte padding slower than 128 | refuted (reversed); settled per ordering by H61 | 185 vs 219 cycles per message; index placement was not controlled in that run. results/2026-11-15_m4_launch_protocol.txt RUN LENGTH DEPENDENT: measured with titan-stream at 200,000 messages, where H67 showed the performance core runs at 1.085 GHz because the run finishes before the frequency governor responds. The cached conclusions hold at every run length tested. Any UNCACHED ordering comparison from this row does not generalise: at 5,000,000 and 10,000,000 messages SeqCst and AcqRel cost the same to within 1.3 percent and Fenced becomes 2.2x cheaper than AcqRel rather than 3.6x more expensive. See results/2026-11-16_m4_runlength.txt. |
| H4 | `wfe` has a higher median round trip than `isb` spinning | refuted | `wfe` 272 vs `isb` 347 cycles per round trip. PRELIMINARY PROTOCOL: measured with 3 to 5 epochs inside one process, not the independent-launch protocol, so the launch-to-launch component is unmeasured. Data in results/2026-11-11_m4_spsc.txt. Quotable only with the protocol stated; re-measure before any claim smaller than the noise floor. REPRODUCED: H64 re-measured this case over 30 independent launches on wall power and agrees to within 3.5 percent, so the preliminary figure stands. |
| H5 | kperf flag value 1 is not a user-only filter | confirmed | identical `getppid` instruction count. NO RESULTS FILE, and correctly so: this is a finding about the kperf counter interface itself, not an experiment with arms and statistics. The row is the record. |
| H6 | Sub-step counter deltas (17 or 18 units) below 1 per 10,000 | refuted | 5.70 per 10,000. NO RESULTS FILE, and correctly so: this is a finding about counter sub-step behaviour, not an experiment with arms and statistics. The row is the record. |
| H7 | `wfe` wins because polling steals the line | refuted | plain spin misses L1D 10x more than `isb` yet is faster. PRELIMINARY PROTOCOL: measured with 3 to 5 epochs inside one process, not the independent-launch protocol, so the launch-to-launch component is unmeasured. Data in results/2026-11-11_m4_spsc.txt. Quotable only with the protocol stated; re-measure before any claim smaller than the noise floor. REPRODUCED: H64 re-measured this case over 30 independent launches on wall power and agrees to within 3.5 percent, so the preliminary figure stands. |
| H8 | E-core QoS round trip at least 3x P-core | refuted | 2.05x. PRELIMINARY PROTOCOL: measured with 3 to 5 epochs inside one process, not the independent-launch protocol, so the launch-to-launch component is unmeasured. Data in results/2026-11-11_m4_spsc.txt. Quotable only with the protocol stated; re-measure before any claim smaller than the noise floor. DID NOT REPRODUCE: the preliminary ratio was 2.05x over 5 epochs; H64 measures 3.057x median wall time over 30 launches, which reverses this row's refutation of the 3x claim. Use H64, not this row. |
| H9 | Core frequency within 5% of 4.4 GHz during ping-pong | refuted | 3.3 to 3.7 GHz. PRELIMINARY PROTOCOL: measured with 3 to 5 epochs inside one process, not the independent-launch protocol, so the launch-to-launch component is unmeasured. Data in results/2026-11-11_m4_spsc.txt. Quotable only with the protocol stated; re-measure before any claim smaller than the noise floor. REPRODUCED: H64 measures 3.506 to 3.628 GHz over 30 launches against this row's 3.3 to 3.7 GHz. |
| H10 | L3 book at least 2x faster than the reference book | supported | 3.1x to 3.3x. PRELIMINARY PROTOCOL: measured with 3 to 5 epochs inside one process, not the independent-launch protocol, so the launch-to-launch component is unmeasured. Data in results/2026-11-11_m4.txt. Quotable only with the protocol stated; re-measure before any claim smaller than the noise floor. |
| H11 | L3 book allocates nothing in measured epochs | supported | 0 vs 21k to 27k. PRELIMINARY PROTOCOL: measured with 3 to 5 epochs inside one process, not the independent-launch protocol, so the launch-to-launch component is unmeasured. Data in results/2026-11-11_m4.txt. Quotable only with the protocol stated; re-measure before any claim smaller than the noise floor. |
| H12 | L3 book takes at least 1 L1D miss per message | supported | 13.1 to 16.0. PRELIMINARY PROTOCOL: measured with 3 to 5 epochs inside one process, not the independent-launch protocol, so the launch-to-launch component is unmeasured. Data in results/2026-11-11_m4.txt. Quotable only with the protocol stated; re-measure before any claim smaller than the noise floor. |
| H13 | Decoding is under 20% of L3 per-message cycles | supported | 14.6%. PRELIMINARY PROTOCOL: measured with 3 to 5 epochs inside one process, not the independent-launch protocol, so the launch-to-launch component is unmeasured. Data in results/2026-11-11_m4.txt. Quotable only with the protocol stated; re-measure before any claim smaller than the noise floor. |
| H14 | With head and tail forced into one 128-byte line, 64-byte padding is faster than 128-byte padding in uncached streaming | inconclusive; settled per ordering by H61 | cycles 169 vs 220 per message, lower in 3 of 3 PMU epochs; timing epochs p = 0.69; retest at N = 30. results/2026-11-11_m4_spsc.txt results/2026-11-15_m4_launch_protocol.txt RUN LENGTH DEPENDENT: measured with titan-stream at 200,000 messages, where H67 showed the performance core runs at 1.085 GHz because the run finishes before the frequency governor responds. The cached conclusions hold at every run length tested. Any UNCACHED ordering comparison from this row does not generalise: at 5,000,000 and 10,000,000 messages SeqCst and AcqRel cost the same to within 1.3 percent and Fenced becomes 2.2x cheaper than AcqRel rather than 3.6x more expensive. See results/2026-11-16_m4_runlength.txt. |
| H15 | On a real NASDAQ BX day the L3 book matches the reference book after every message, with no unknown order references | supported | BX 2019-07-30, 28,734,686 messages: 0 top-of-book mismatches, 0 unknown references. results/2026-11-11_m4.txt |
| H16 | On real data the L3 book takes fewer L1D misses per message than on the synthetic default mix (13.8) | supported | 5.98 per message (98.3 cycles, 24.2 ns); peak 67,283 live orders vs 982,360 synthetic. results/2026-11-11_m4.txt |
| H17 | SeqCst at or below AcqRel cost (seen in three runs) is a code placement effect, not an ordering effect: it disappears when loop alignment is fixed | partly supported | Retest 2026-11-12 on wall power, 10 PMU epochs per layout, identical instruction census. Cached: no difference in any layout (SeqCst/AcqRel 1.013, 0.977, 0.997; p 0.65 to 0.91). Uncached: no difference in the default layout (1.008, p = 0.33) but SeqCst 11.9% and 20.3% cheaper with 64 and 128-byte alignment (p = 0.0001 and 0.0005). The effect depends on layout, so the pre-registered rule (size change beyond the within-layout interval) is met and the effect is not a fixed cost of the ordering instructions. The stated prediction failed: the advantage does not disappear with fixed alignment, it appears and grows with it. SeqCst uncached epochs spread from 162 to 222 cycles in the aligned layouts while AcqRel stays at 215 to 224. In the first pass SeqCst also executed fewer instructions per message (28.4 and 28.7 vs 29.9), which points at the wait loop; mechanism open. First pass (3 epochs, battery) was inconclusive. results/2026-11-12_m4.txt results/2026-11-12_m4_ac.txt |
| H18 | Single-core software tick-to-trade (`titan-t2t` stage `full`, synthetic default mix) costs more than 30 ns per message; predicted 45 to 60 ns | supported; predicted range missed (low) | 42.7 ns (bootstrap 95% CI 42.7 to 42.9), 189 cycles per message; real BX data 31.8 ns (31.8 to 32.2), 135 cycles. results/2026-11-11_m4.txt |
| H19 | Ring operations add less than 10% to the trading path: `full` within 10% of `direct` in cycles per message | supported | +7.8% cycles synthetic (189 vs 176), +6.7% real (135 vs 127). PRELIMINARY PROTOCOL: measured with 3 to 5 epochs inside one process, not the independent-launch protocol, so the launch-to-launch component is unmeasured. Data in results/2026-11-11_m4.txt. Quotable only with the protocol stated; re-measure before any claim smaller than the noise floor. |
| H20 | Component ablation is subtractive: stage `book` costs no more cycles per message than stage `full` | supported on M4 | 169 vs 189 cycles synthetic, 119 vs 135 real, 5 of 5 epochs each. The inversion seen first on the x86 development VM did not reproduce: on the VM `book` was slower than `full` (115 to 142 vs 94 to 123 ns per message) while executing fewer instructions (223 vs 274 per message, callgrind). results/2026-11-11_m4.txt |
| H21 | With limits no order can breach, the eight pre-trade checks (`risk` minus `full`) cost under 60 cycles per checked order on both feeds | not supported (method flawed on the synthetic feed) | Real data 28 cycles per order, synthetic 1,196. On the synthetic feed (0.5% of messages emit an order) the subtraction charges the checks with about 5 instructions of code-generation overhead on every message (236 to 241 per message), most of which never reach the checks. On real data (9.7% emit an order) the difference, 11 instructions per message (188 to 199), matches the checks own cost (115 instructions x 0.0968 = 11.1), and 28 cycles per order is close to the isolated 21.1 of H21b. Whole-path cost: +2.0% cycles per message on real data, +3.3% synthetic (5 of 5 epochs, p = 0.008). Replaced by H21b. results/2026-11-12_m4.txt |
| H22 | Two-core pipeline at 1M msg/s offered (synthetic): median scheduled-arrival-to-decision latency is between 80 and 250 ns for spin, isb and wfe | supported | 179 (spin), 174 (isb), 171 (wfe) ns. results/2026-11-12_m4.txt |
| H23 | At 1M msg/s the p99.99 exceeds 1 us for every wait strategy: interrupts and scheduling, not the data path, set the extreme tail | supported (threshold only) | 10.2 to 10.6 us for all three waits. The mechanism in the claim (interrupts and scheduling) was not tested. PRELIMINARY PROTOCOL: measured with 3 to 5 epochs inside one process, not the independent-launch protocol, so the launch-to-launch component is unmeasured. Data in results/2026-11-12_m4.txt. Quotable only with the protocol stated; re-measure before any claim smaller than the noise floor. |
| H24 | The median latency with wfe is within one counter step (41.67 ns) of spin at every load that does not saturate | supported | 8, 5 and 0 ns apart at 1M, 5M and 10M msg/s; wfe and isb are slightly faster than spin at 1M and 5M (p about 0.01). PRELIMINARY PROTOCOL: measured with 3 to 5 epochs inside one process, not the independent-launch protocol, so the launch-to-launch component is unmeasured. Data in results/2026-11-12_m4.txt. Quotable only with the protocol stated; re-measure before any claim smaller than the noise floor. |
| H25 | The p99 at the highest offered load that does not saturate is more than 5 times the p99 at 1M msg/s | refuted | p99 at 10M msg/s is 1.13x (spin), 1.27x (isb), 1.46x (wfe) the p99 at 1M. Flat to 10M, saturated at 15M with capacity about 12.7M msg/s; loads between 10M and 15M were not tested, so the shape of the transition is not resolved. results/2026-11-12_m4.txt |
| H21b | Measured alone (recorded orders replayed through the checks, PMU over batches), the eight checks cost under 60 cycles per order on both feeds | supported | Checks alone, 5 of 5 epochs: 21.1 cycles (5.15 ns) per order on real data, 17.0 cycles (3.80 ns) synthetic; replay floor 4.5 and 4.3 cycles; 115 to 117 instructions per order at an IPC of 5.5 to 6.9; 0.18 and 0.05 branch mispredicts per order. Over 1,936,537 real orders. Wall power. results/2026-11-12_m4_ac.txt |
| H26 | On the real BX open, messages that arrive within 100 ns of the previous one (bursts) have a higher median latency than messages that arrive 1 to 10 us after it: queueing inside bursts sets the p99 | partly supported | Messages under 100 ns after the previous one: median 165 to 177 ns vs 107 to 115 ns for 1 to 10 us gaps, all three waits, 5 of 5 epochs (p about 0.008). But the class holds 193 of 198,807 messages (0.1%), too few to set the p99; the p99 is 1.9 to 3.6 us in every recorded gap class (the 10 to 100 us class, about 44% of messages, was not recorded). Bursts are slower, but they are not the cause of the real-pacing tail. results/2026-11-12_m4_ac.txt |
| H27 | Messages that arrive more than 100 us after the previous one have a higher median latency than messages arriving 1 to 10 us after it: an idle core pays to wake (cold caches, predictors or clock) | refuted | Over 100 us gaps: median 98 to 112 ns vs 107 to 115 ns for 1 to 10 us gaps, 0 of 5 epochs higher. Not at the median; the p99 of that class is 1.30 to 1.35x the 1 to 10 us p99 (5 of 5 epochs, p = 0.008), so waking from idle shows in the tail only. results/2026-11-12_m4_ac.txt |
| H28 | The first 100,000 messages of the BX open replayed at a constant 1M msg/s have a p99 below 1 us for all three waits: the real book is not what makes the real-pacing tail | supported | p99 183 (isb), 206 (wfe), 258 (spin) ns at a constant 1M msg/s on the real messages; median 103 to 109 ns. The real book does not make the tail. results/2026-11-12_m4_ac2.txt |
| H29 | The synthetic feed at a constant 6,627 msg/s (the mean rate of the real 30 s window) has a p99 above 1 us for all three waits: low utilisation alone produces the tail | supported | Synthetic feed at 6,627 msg/s: p99 1,888 (wfe), 2,064 (spin), 2,153 (isb) ns. For comparison (different run, battery): 333 to 360 ns at 1M msg/s and median 171 to 179 ns; here the median is 574 to 926 ns. results/2026-11-12_m4_ac2.txt |
| H30 | The same 100,000 real messages at a constant 6,627 msg/s have a p99 within 1.5x of their p99 under the real timestamps: the arrival structure adds little beyond the rate | invalid (design error) | The rate used, 6,627 msg/s, is the mean of the earlier 30 s window, not of these 100,000 messages: they span 7.535 s of feed time, a mean of 13,271 msg/s. The comparison therefore mixes a 2x rate difference with the arrival structure and cannot test the claim. Recorded values: p99 at a constant 6,627 msg/s 2,173 (wfe), 2,331 (spin), 2,961 (isb) ns; under the real timestamps 1,873, 1,931, 1,886 ns. To be rerun at 13,271 msg/s. results/2026-11-12_m4_ac2.txt |
| H31 | Encoding an order as a FIX 4.2 NewOrderSingle costs at least 3x the cycles of an OUCH 4.2 Enter Order, on both feeds; predicted 4 to 7x | supported | FIX 4.2 vs OUCH 4.2 per order: 124.7 vs 28.7 cycles (4.34x) real, 114.6 vs 22.2 (5.17x) synthetic, 5 of 5 epochs; FIX 832 instructions and 0.87 branch mispredicts per order, OUCH 209 and 0.0003; replay floor 4.5 cycles. results/2026-11-12_m4_ac2.txt |
| H32 | At 6,627 msg/s the trading path's own L1D load misses per message (`full` minus `none`, same wait and rate, consumer PMU) are at least 1.5x those at 1M msg/s, for wfe and spin, on the synthetic feed: cache contents decay between sparse messages | refuted | Synthetic, path L1D load misses per message (full minus none): wfe 9.8 at 1M vs 11.2 at 6,627 msg/s (1.14x), spin 9.5 vs 12.3 (1.29x). Real (not pre-registered): wfe 3.7 vs 6.3 (1.71x), spin 6.0 vs 6.9 (1.15x). Branch mispredicts per message within 0.1 at both rates (synthetic about 3.6, real 2.89 vs 2.95). The few extra L1D misses cannot by themselves add about 2 us; the cost may sit deeper (L2, system cache, memory, TLB), which these counters do not see. results/2026-11-12_m4_ac3.txt |
| H33 | With no trading work at all (`path=none`), the p99 at 6,627 msg/s is above 1 us for wfe and spin on both feeds: the hand-off and wake-up, not the data path, set the low-rate tail | refuted | path=none at 6,627 msg/s: p99 94 (wfe) and 123 (spin) ns synthetic, 83 and 122 ns real, at or below the same at 1M (123 to 124 ns). The hand-off and wake-up alone are fast. One real epoch (wfe and spin, path=none) shows a stall of about 164 s and a 64 us p99; the section ran about 2.5 min longer than planned, consistent with the machine sleeping; medians over epochs are unaffected, and later scripts keep the machine awake. results/2026-11-12_m4_ac3.txt |
| H34 | At 6,627 msg/s the tail is in the service time: pop-to-decision p99 above 1 us while schedule-to-pop p99 stays below 300 ns, isb wait, both feeds | supported | At 6,627 msg/s, keep-warm off: service p99 3,000 ns (synthetic) and 2,958 ns (real); delay-to-pop p99 125 and 115 ns. At 1M msg/s the service p99 is 250 and 125 ns. Millisecond pop stalls occurred in at least 3 of 5 synthetic epochs (median of the per-epoch maximum 2.37 ms); one raised that epoch's pop p99 to about 1.3 ms. The median over epochs is 125 ns. results/2026-11-12_m4_ac4.txt |
| H35 | Keeping the book warm while waiting (`warm=book`) lowers the full-path p99 at 6,627 msg/s by at least 2x, both feeds | refuted | warm=book, end-to-end p99 at 6,627 msg/s: 3,050 vs 3,102 ns synthetic (1.02x lower), 2,491 vs 3,073 ns real (1.23x); service p99 2,959 and 2,208 ns. PRELIMINARY PROTOCOL: measured with 3 to 5 epochs inside one process, not the independent-launch protocol, so the launch-to-launch component is unmeasured. Data in results/2026-11-12_m4_ac4.txt. Quotable only with the protocol stated; re-measure before any claim smaller than the noise floor. |
| H36 | Keeping memory active while waiting without touching the book (`warm=mem`) lowers the full-path p99 at 6,627 msg/s by at least 2x, both feeds | supported | warm=mem, end-to-end p99 at 6,627 msg/s: 618 vs 3,102 ns synthetic (5.0x lower), 626 vs 3,073 ns real (4.9x); service p99 541 and 542 ns. Costs: the delay-to-pop p99 rises to 284 and 285 ns (a message can wait for one unit), and the real-feed median rises from 173 to 289 ns (synthetic falls from 527 to 312). The outcome, memory helps and the book does not, was not one of the three pre-registered readings. By design the mem units are loads meant to miss the system cache, so they probably keep the core busy longer than the book units (unit duration was not recorded); memory activity and core activity are therefore not separated; neither is identified by these counters. results/2026-11-12_m4_ac4.txt |
| H30b | The first 100,000 messages of the BX open at a constant 13,271 msg/s (their own mean rate: 100,000 over 7.535 s) have a p99 within 1.5x of their p99 under the real timestamps, for all three waits: the arrival structure adds little beyond the rate | within the noise floor, not established (weakened by H62) | Median p99, real timestamps vs constant 13,271 msg/s: wfe 2,598 vs 1,867 ns (1.39x), spin 2,843 vs 2,533 (1.12x), isb 2,896 vs 2,779 (1.04x); all within 1.5x. Medians 112 to 122 ns in all arms. The direction is not established: the identical real-timestamp arms were 1.39 to 1.54x higher here than in the third run (1,873, 1,931, 1,886 ns), a run-to-run change as large as the effect. The run had more browser activity than earlier runs: the three real-timestamp arms and the constant wfe arm show millisecond stalls (p99.99 1.3 to 2.2 ms) and wide intervals (p99 upper bounds 7.3 to 24.8 us), and more messages released over 1 us late (341 to 594 vs 121 to 180). To be repeated in the final protocol on a quiet machine. results/2026-11-12_m4_ac5.txt results/2026-11-13_m4.txt results/2026-11-15_m4_launch_protocol.txt |
| H30c | Repeat of H30b on a quiet machine (one browser window, no other applications): p99 under the real timestamps within 1.5x of the p99 at a constant 13,271 msg/s, for all three waits | supported | Quiet machine (VS Code and one browser window). Median p99, real timestamps vs constant 13,271 msg/s: wfe 1,904 vs 1,865 ns (1.02x), spin 2,157 vs 2,780 (constant 1.29x higher), isb 2,214 vs 2,397 (constant 1.08x higher); all within 1.5x, direction mixed. Medians 103 to 115 ns. Together with H30b: at the same mean rate the arrival pattern changes the p99 by less than 1.5x. results/2026-11-13_m4.txt |
| H37 | One ITCH message per MoldUDP64 datagram over UDP loopback: (a) at 100k msg/s the median latency with a polling receiver is at least 5x the shared-memory pipeline median (isb) at the same rate and window; (b) at 10k msg/s a blocking receiver's median is at least 1.5x the polling receiver's; (c) at 1M msg/s both UDP receivers saturate (achieved below 97% of offered, or messages lost) | (a) not testable; (b) refuted; (c) supported | (a) Every measured UDP run at 100k msg/s was invalid: 5,536 to 7,008 of 100,000 messages never received (no datagrams held back in these runs), 16 of 20 runs ending on the receive timeout, achieved 58k to 59k msg/s. The comparison at 100k msg/s cannot be made; the UDP path with one message per datagram does not sustain 100k msg/s on this machine. (b) At 10k msg/s, median blocking vs polling: 23,256 vs 20,720 ns real (1.12x), 23,280 vs 20,456 synthetic (1.14x), below 1.5x; blocking is slower in 5 of 5 epochs (p = 0.008). (c) At 1M msg/s every UDP run saturated (achieved 70k to 72k msg/s). Exploratory, at 10k msg/s: polling median 20.7 us vs 112 ns for shared memory on real data (185x), p99 26.5 vs 2.6 us; about 100,000 of 100,000 datagrams were released more than 1 us late (printed rounded as 1e+05), so the send call itself takes over 1 us. results/2026-11-13_m4.txt results/2026-11-14_m4.txt |
| H38 | Parsing a FIX 4.2 NewOrderSingle: (a) a full parse that validates BodyLength and CheckSum and indexes every field costs at least 2x the cycles of a selective parse of the five fields the order needs; (b) finding field delimiters with NEON compares lowers the full parse cost by at least 1.3x; both feeds | (a) refuted; (b) supported | (a) Full vs selective parse: 464.6 vs 298.5 cycles per message real (1.56x), 460.2 vs 295.1 synthetic (1.56x), below 2x. (b) NEON delimiters: full parse 464.6 to 301.1 cycles real (1.54x), 460.2 to 296.6 synthetic (1.55x); 5 of 5 epochs, p = 0.008. With NEON the validated full parse costs within 1% of the unvalidated selective one (301.1 vs 298.5 cycles real; not tested). 1,936,537 real messages, all parsed back exactly. results/2026-11-13_m4.txt |
| H39 | Audit trail of every risk decision: (a) a 32-byte binary record per order adds under 10 cycles per order to the checks; (b) a formatted text record adds at least 10x the binary record's cost, (median audittext - median checks) / (median auditbin - median checks) >= 10, with auditbin above checks (p < 0.05); both feeds, median `msg_cycles` over epochs | supported | (a) Binary record: 25.98 vs 20.75 cycles per order real (+5.2), 20.71 vs 17.13 synthetic (+3.6), p = 0.008. (b) Text record adds 302.8 cycles real and 299.7 synthetic: 58x and 84x the binary record, and 15x (real) to 17x (synthetic) the cost of the eight checks themselves. results/2026-11-13_m4.txt |
| H40 | An online VPIN update in volume time, with each execution signed exactly from the resting order's side in the L3 book, costs under 30 cycles per signed execution (median `msg_cycles` of vpin minus that of vpinfloor), both feeds | supported | vpin minus vpinfloor: 23.81 - 1.59 = 22.2 cycles per signed execution real (681,693 executions, whole day), 17.12 - 1.52 = 15.6 synthetic (medians over 5 epochs). results/2026-11-13_m4.txt |
| H41 | Replication on BX 2019-07-30 (instruments with at least 100 signed executions): bulk volume classification (1-minute bars, normal CDF, per-instrument standard deviation of bar price changes) misclassifies a larger share of volume than the tick rule, both measured per 1-minute bar | supported (replication) | 1,440 instruments, 71.1 million shares compared. Bar-level misclassified volume: BVC 36.7%, tick rule 28.3%; BVC worse in 1,153 of 1,440 instruments. Tick rule trade by trade: 30.1% of volume wrongly signed; 333 executions unresolved. Excluded: 243,778 trades on non-displayed orders; non-printable C executions: 0 on BX that day. results/2026-11-13_m4.txt |
| H42 | Private copies (`titan-fanout work=replay`, one ring per reader): X(copy3) >= 2 X(copy1), and copy3 above copy1, both feeds | supported | work=replay, writer PMU cycles per update, medians over 10 epochs (results/2026-11-14_m4.txt). BX: copy3 290.16 vs copy1 98.15 (p = 1e-5, exact), X(copy3)/X(copy1) = 4.28 [3.18, 4.98]. Synthetic: 316.62 vs 111.91 (p = 1e-5), ratio 3.82 [3.49, 4.29]. Exploratory, not tested: on BX copy2 cost the writer less than copy1 (72.2 vs 98.2 cycles, p = 1e-5); on synthetic the copy2 epochs split into two groups (six at 70 to 85 cycles, four at 200 to 233; p = 0.48 against copy1); not explained here |
| H43 | One shared seqlock snapshot per instrument (`work=replay`): (a) the first reader costs the writer, snap1 above snap0; (b) more readers add little, X(snap3) <= 1.5 X(snap1); both feeds | (a) supported; (b) partly supported | work=replay. (a) snap1 vs snap0: 105.73 vs 42.78 cycles BX, 109.38 vs 42.29 synthetic (p = 1e-5 both). (b) X(snap3)/X(snap1) = 1.519 [1.461, 1.566] BX, above the 1.5 bound by a margin inside its interval; 1.212 [1.166, 1.249] synthetic, within it. snap0 vs none: +3.3 cycles BX, +3.0 synthetic (the seqlock write with no reader). PRELIMINARY PROTOCOL: measured with 3 to 5 epochs inside one process, not the independent-launch protocol, so the launch-to-launch component is unmeasured. Data in results/2026-11-14_m4.txt. Quotable only with the protocol stated; re-measure before any claim smaller than the noise floor. |
| H44 | At the highest update rate (`work=replay`) and 3 readers, the shared snapshot costs the writer less than private copies: snap3 below copy3, both feeds | supported | work=replay, 3 readers: snap3 vs copy3 140.07 vs 290.16 cycles BX (0.48x), 124.23 vs 316.62 synthetic (0.39x), p = 1e-5 both. Copy readers fall behind at this rate: at 3 readers the 99th percentile staleness of a copy update is 135 microseconds BX and 64 microseconds synthetic (ring queueing; median 83 to 84 ns), against 84 ns for snap (median 83 ns BX, 42 ns synthetic); at 1 reader the copy median is 84 microseconds BX. PRELIMINARY PROTOCOL: measured with 3 to 5 epochs inside one process, not the independent-launch protocol, so the launch-to-launch component is unmeasured. Data in results/2026-11-14_m4.txt. Quotable only with the protocol stated; re-measure before any claim smaller than the noise floor. |
| H45 | At the pace of the real feed (`work=book`, BX) and 3 readers, the order reverses: snap3 above copy3 | refuted | work=book, BX, 3 readers: snap3 114.76 vs copy3 116.00 cycles per feed message, p = 0.002 (exact), but in the opposite direction: the snapshot cost the writer less than three copies even at feed pace. The census confirms one `dmb ish` per seqlock write and none in the copy loop; at this update rate (2.9% of messages publish) the fence did not make snap the more expensive design. PRELIMINARY PROTOCOL: measured with 3 to 5 epochs inside one process, not the independent-launch protocol, so the launch-to-launch component is unmeasured. Data in results/2026-11-14_m4.txt. Quotable only with the protocol stated; re-measure before any claim smaller than the noise floor. |
| H46 | At the pace of the feed (`work=book`), the median staleness of the updates readers take differs between the designs by at most 42 ns (one counter step) at 1, 2 and 3 readers, both feeds | partly supported | work=book, medians over epochs of stale_p50_ns, snap vs copy: BX 42 vs 83 (1 reader), 42 vs 84 (2), 42 vs 125 (3); synthetic 42 vs 83, 42 vs 125, 42 vs 125. Within 42 ns in 3 of 6 conditions (BX 1 and 2 readers, synthetic 1 reader). Snap was never slower; its staleness covers only the versions readers took. PRELIMINARY PROTOCOL: measured with 3 to 5 epochs inside one process, not the independent-launch protocol, so the launch-to-launch component is unmeasured. Data in results/2026-11-14_m4.txt. Quotable only with the protocol stated; re-measure before any claim smaller than the noise floor. |
| H47 | At the pace of the feed (`work=book`, 1 reader), a snapshot reader misses at least 1% of the versions published on the real feed and under 1% on the synthetic feed | partly supported | work=book, 1 reader, median conflated_frac: BX 0.00205 (range 0.00172 to 0.00290 over 10 epochs), below the predicted 1%; synthetic 0 (8 of 10 epochs 0, 2 at 0.00092), under 1% as predicted. results/2026-11-14_m4.txt |
| H48 | Profile-guided optimisation of `titan-t2t`, stage `full`: (a) a profile from the same feed lowers cycles per message by at least 3% against the default build; (b) a profile from the other feed gives at least half of that reduction; (c) the same-feed profile changes cycles more than aligning functions and non-fall-through blocks to 64 bytes (the H17 layout) does; both feeds | (a) refuted; (b) not evaluated; (c) supported by the rule as written, not interpreted | Stage full, medians of msg_cycles over 10 epochs. Same-feed profile against base: synthetic 188.84 vs 190.87 (gain 1.06%, p = 0.35), BX 134.66 vs 134.31 (gain -0.26%, p = 0.80); both below the 3% threshold, so (b) is not evaluated. (c):. results/2026-11-14_m4.txt results/2026-11-14_m4_heap_pruned.txt |same-feed - base| 2.03 vs |align64 - base| 1.45 cycles synthetic, 0.35 vs 0.04 BX, but no one of these four differences is significant. Exploratory: both profiles slowed stage decode on synthetic by 8 to 9% (p = 0.0002) and sped up stage book on BX by 1.7% and 2.1% (p < 0.0001); on BX, stage decode was 1.0% slower with the synthetic profile (p = 0.0002) and 0.5% faster with the BX profile (p = 0.025); stage full changed significantly in one of the four build and feed pairs only: the synthetic profile on BX, 133.34 vs 134.31 cycles (0.7% lower, p = 0.009). Training printed the LLVM warning that value-profile counters ran out (20 times); branch and block counts are not affected |
| H49 | One exposure limit shared by several strategy threads (`titan-agg`, strategy 0's PMU cycles per order): (a) a shared total costs more than a private one without contention, faa1 above local1; (b) contention on one shared total, faa3 above faa1; (c) an exact limit (compare-and-swap loop) costs more than the two-step check under contention, cas3 above faa3; (d) per-strategy totals cost less than one shared total under contention, shard3 below faa3 | (a) supported; (b) supported; (c) refuted; (d) supported | Strategy 0's PMU cycles per order, medians over 10 epochs, no epoch starved. (a) faa1 14.28 vs local1 8.21 (1.74x, p = 1e-5). (b) faa3 217.88 vs faa1 14.28 (15.3x, p = 1e-5). (c) cas3 221.23 vs faa3 217.88, p = 1.0: no difference; strategy 0's failed compare-and-swap attempts 0.535 per order (`cas_retries_s0`; 0.58 over all strategies). (d) shard3 19.72 vs faa3 217.88 (0.09x, p = 1e-5). Census: faa compiles to `ldadd`, cas to one LSE `cas`, no exclusive pairs, no barriers. Exploratory, not tested: with 2 strategies cas cost about half of faa (51.55 vs 103.38 cycles). results/2026-11-14_m4.txt |
| H50 | Inbound wire format, per price-level update (`titan-wire`, PMU cycles): (a) above the floor of each format, the validated FIX 4.4 decode costs at least 5x the fixed binary decode; (b) above the JSON floor, `serde_json` costs at least 2x a scanner specialised to the key order of the documented example; (c) in total cycles, that specialised JSON scanner costs less than the validated FIX decode; both feeds; thresholds set after development-machine runs (see the tenth run) | supported (both feeds) | X(fix)/X(bin) 55.5 BX, 44.3 synthetic; X(serde)/X(fixed) 4.57 BX, 4.33 synthetic; json_fixed below fix p=0.0000 both feeds; serde allocates 1.0 per update, the others 0; results/2026-11-14_m4_wire_arb.txt |
| H51 | A/B line arbitration (`titan-arb`, PMU cycles per delivered message): (a) above the floor, taking the same stream from two lossless lines costs at most 1.5x one line (the duplicate line costs less than half the delivery itself; 1.5x is the registered rule); (b) 3% independent packet loss per line (registered as 1%, raised before any timed run; see the eleventh run), recovered from the other line and the re-request server, costs at most 2x the lossless two-line cost; (c) delivery is exact (every message once, in order) in every condition on both feeds, by assertion; thresholds set before any run of the program anywhere | (a) partly supported, (b) supported, (c) supported | (a) X(two0)/X(one0) 1.429 BX (interval reaches 1.586), 1.670 synthetic, so BX only; (b) 0.892 BX, 0.851 synthetic, but a weak test: loss removes arrivals, so the lossy conditions are cheaper than lossless two-line; (c) one delivery checksum across all conditions and epochs of each feed, 50 and 47 requests, 2,236 and 2,351 resent, 0 partial, 0 foreign, allocations_pass 0 throughout; results/2026-11-14_m4_wire_arb.txt |
| H52 | Best-effort FIX decode (`titan-wire` stage `fix_simd`: the NEON delimiter search of `soh_neon` and a NEON checksum, validating identically to `fix`): (a) it costs less than the byte-at-a-time `fix`, both feeds; (b) X(fix_simd)/X(fix) lies between 0.5 and 0.9, both feeds; (c) X(fix_simd)/X(bin) is at least 10, both feeds | supported (both feeds) | X(fix_simd)/X(fix) 0.723 BX, 0.732 synthetic, inside the registered 0.5 to 0.9; X(fix_simd)/X(bin) 40.3 BX, 32.6 synthetic; instructions per update 3026.7 to 2517.2 on BX while cycles fall 27.3%, so IPC rises 5.77 to 6.61. The FIX figure to quote is now 372.8 cycles above the FIX floor on BX, not H50's 514.1; results/2026-11-14_m4_fix_cross.txt |
| H53 | Consolidated book across BX and PSX (`titan-cross`, 2019-07-30, PMU cycles per merged message): (a) keeping a consolidated top incrementally costs at most 0.5x the two venue books alone; (b) re-reading every venue's top for every shared instrument every 1,000 messages costs at least 2x the incremental design, above that floor; (c) the two venues' displayed books lock and cross during continuous trading, both at least once; (d) at least half of the crossed episodes begin and end between two rebuilds, so the periodic design never sees them | (a), (b), (c) supported, (d) refuted | (a) X(incr) 23.45 against a floor of 153.33, so 0.153; (b) X(rebuild)/X(incr) 4.70 at a rebuild every 1,000 messages, with 782.4 instructions and 37.6 L1D misses per message against 253.4 and 7.1; (c) 360 locked and 33 crossed episodes over 17 instruments inside the window; (d) 15 of 33 invisible, 0.4545, just under the registered 0.5. The episodes are long (mean 18.2 s crossed) and are a statement about every displayed order, odd lots included, not about protected quotations: see H54; results/2026-11-14_m4_fix_cross.txt |
| H54 | The same day over round-lot-sized levels (`titan-cross min_lot=100`, a level holding at least 100 shares, as a proxy for a protected quotation): (a) crossed episodes fall to at most half of the all-orders count of 33; (b) locked episodes fall to at most half of 360; (c) replicated over the whole day rather than the first 4,000,000 merged messages, H53(a) and H53(b) keep their outcomes | supported | (a) 2 crossed episodes against 33, (b) 82 locked against 360; the longest cross falls from 212.8 s to 0.110 ms and the longest lock from 1,208 s to 43.8 ms, so the long episodes were odd lots resting inside the other venue's quote; (c) over the whole day X(incr) 22.50 against a floor of 157.71 (0.143) and X(rebuild)/X(incr) 5.305, both outcomes replicated. Reported: the periodic design missed both round-lot crossings (share 1.000), being 224 ms behind on average against episodes of 110 us; results/2026-11-14_m4_roundlot.txt |
| H55 | The third brief's order book against this one (`titan-heap`, cycles per message above decoding alone, X): (a) X(heap_indexed, top) / X(array, top) is at least 1.2; (b) X(heap_indexed, depth) / X(array, depth) is at least 2; (c) the lazy heap costs more than the indexed one for depth; (d) harness check: the two array stages, which run identical code, differ by at most 0.10 of the total; both feeds | supported (all four parts, both feeds; (b) and (c) superseded) | (d) the harness check passed first: the two array stages differ by 0.007 BX and 0.010 synthetic against a bound of 0.10; (a) 1.654 BX, 2.023 synthetic; (b) 2.253 BX, 11.563 synthetic; (c) lazy above indexed for depth, p=0.0000 both feeds. L1D misses per message 6.45 array, 17.3 indexed heap at the top, 158 lazy heap at depth. (b) and (c) were measured against a depth query that scanned the whole heap instead of pruning at the first smaller key; the sixteenth run re-measured them at 2.299 BX and 2.690 synthetic, and those are the figures to quote for depth. (a) and (d) never scan and stand as measured; results/2026-11-14_m4_heap.txt |
| H56 | The heap's depth query given its best form (`HeapBook::best_depth` pruned at the first smaller key) and the fleeting quotes the third brief recommends: (a) X(heap_indexed, depth) / X(array, depth) is still at least 1.5 on both feeds; (b) on the synthetic feed that ratio falls to at most half of H55's 11.563, so at most 5.78; (c) on BX it stays within a quarter of H55's 2.253, between 1.69 and 2.82; (d) on the real feed at least half of the orders that arrive at the best price are gone within one second | supported (all four parts) | (a) 2.299 BX, 2.690 synthetic; (b) the synthetic ratio fell from 11.563 to 2.690, 23% of it; (c) BX moved 2.0%, from 2.253 to 2.299, so the explanation held on both sides; (d) 0.5388 of orders arriving at the best price gone within 1 s, and 0.5321 even if every still-resting order would have outlived a second. With the query at its best the array book is 1.70x cheaper for the best price and 2.30x for depth on BX, and the two array stages differ by 0.000. New finding: with the pruned query the lazy and indexed delete policies reverse between feeds (lazy/indexed depth 1.532 BX, 0.792 synthetic), because the indexed policy pays per sift step and the lazy one per stale pop: 4,200,776 stale pops on BX against 50,547 on the synthetic feed; results/2026-11-14_m4_heap_pruned.txt |
| H57 | Queue position, which the fourth brief makes a headline task, and the cancellation rate it states: on the real feed (a) the share of orders that see any execution is non-increasing across the five queue-position buckets (0, 1 to 99, 100 to 999, 1000 to 9999, 10000 or more shares ahead on arrival); (b) an order arriving with nothing ahead of it sees an execution at least twice as often as one arriving with 10000 or more shares ahead; (c) more than half of the orders that end inside the window end without any execution, which is the brief's stated 50% cancellation rate or more; (d) knowing the queue position of every arriving order costs less than 1.35x the book alone above the decode floor | (a) refuted, (b), (c), (d) supported | (a) refuted by a reversal at the first step: orders arriving with nothing ahead execute at 0.0266 against 0.0961 for orders with 1 to 99 shares ahead, 3.61x higher; from the second bucket on the series is monotone (0.0961, 0.0158, 0.0046, 0.0030), and the synthetic feed shows no reversal at all (0.1950, 0.1878, 0.1858, 0.1493, 0.0802). Shares ahead conflates queue position with whether the price has a queue: 71.6% of BX arrivals opened a level rather than joined one, and being alone at a price the market is not trading beats nothing. (b) 0.0266 against 0.0030, a ratio of 8.87 against a threshold of 2. (c) 4,284,968 of 4,386,147 ended orders never executed, 0.9769, against the brief's 50%; only 0.0210 filled in full. (d) X(queue)/X(book) = 1.092 [1.082, 1.101] BX, 1.205 synthetic, against a ceiling of 1.35; instructions rose 168.73 to 187.30 while cycles rose 9.2% and IPC rose 1.486 to 1.545. results/2026-11-14_m4_queue_walk.txt |
| H58 | The fourth brief's claim that the change in price grows as size raised to a power above one, and its worked example of a tidy ladder of depth, tested on displayed depth: on the real feed (a) the quote move needed to fill a size, per share, rises across 100, 500, 1000, 5000 and 10000 shares; (b) the average price paid per share, in basis points from the best price, also rises across those sizes; (c) at least 1 in 100 attempts to fill 10000 shares cannot be filled from the displayed side at all; (d) walking the book to fill 10000 shares costs at least 1.5x the best-price query above the decode floor | (a), (b) refuted (confounded, see note), (c), (d) supported | (a) and (b) refuted on the measured numbers: move per 100 shares 4.188, 31.538, 17.026, 1.666, 1.258 and cost per 100 shares 2.402, 11.314, 3.844, 0.525, 0.591, each rising once then falling. Confound recorded, not argued away: a size the side cannot fill is excluded, and the exclusion runs 0.08%, 18.50%, 38.86%, 78.89%, 87.01%, so the mean at 10000 shares is taken over a far more liquid population than the mean at 100; this refutes the registered statement, not the brief's exponent, and the conditioned test is the open item. What survives: at 100 shares, fillable 99.92% of the time, immediacy costs 2.40 bps and the quote moves 4.19 bps over 15,618,175 sides. (c) 13,600,453 of 15,630,442 attempts to fill 10000 shares could not be filled at all, 0.8701 against a threshold of 0.01, and 1000 shares fails 38.86% of the time; the synthetic feed fails only 2.36%. (d) X(walk_10000)/X(top) = 1.667 [1.663, 1.683] BX against a floor of 1.5. New finding: the walk is cheaper on the real feed than the synthetic one (1.667 against 2.140) because it visits 4.512 levels against 10.772; on a real venue the walk is cheap because there is little to walk, which is the same thinness that refutes the brief's ladder. results/2026-11-14_m4_queue_walk.txt |
| H59 | The three defects in how H57 and H58 were measured, corrected: the window was the first 10,000,000 of the session's 28,734,686 messages, and a replacement was counted as a new order. On the whole session, counting order chains rather than instances: (a) more than half of chains that end never execute at all, so the earlier figure is not an artifact of counting replacements; (b) the chain rate is below the instance rate on the same data by less than 8 percentage points; (c) the displayed side cannot fill 10000 shares in more than half of attempts; (d) every headline rate in (a) and (c) moves by less than 5 percentage points between the first 10,000,000 messages and the whole session, so the earlier window was representative | all four supported | (a) 10,110,283 of 10,629,593 chains that began and ended within the session never executed a share, 0.9511, against a threshold of 0.50. (b) chain 0.9511 against instance 0.9590, a gap of 0.79 points inside the 8 point ceiling; the accounting closes exactly, 12,676,036 instances minus 10,629,593 chains equals the 2,046,443 replacements; mean chain 1.1925 instances, only 3.21% ever replaced, longest chain 13,248 instances. (c) 40,367,075 of 46,884,746 attempts to fill 10000 shares could not be filled at all, 0.8610 against 0.50; 1000 shares fails 0.4390. (d) every rate moved under the 5 point threshold between the opening 10,000,000 messages and the whole session: chain 0.9700 to 0.9511 (1.89), instance 0.9769 to 0.9590 (1.79), unfillable at 10000 0.8701 to 0.8610 (0.91), so the earlier window was representative and the seventeenth run was not biased by it. still_resting=0: every order that arrived in the session also ended in it, so the population is complete with no truncation at either edge. H57's first-step reversal survives and grows: 0.1514 for 1 to 99 shares ahead against 0.0462 for nothing ahead, 3.28x, with the extremes 7.00x apart and the synthetic feed still showing no reversal. Cost ratios replicate on a feed 2.87x larger: queue/book 1.093 [1.076, 1.122] against 1.092, walk_10000/top 1.715 [1.699, 1.724] against 1.667. H58's survivorship confound is larger here, not smaller, and stays open. Supersedes the H57 and H58 rates for quotation. results/2026-11-15_m4_wholeday_chains.txt |
| H60 | The survivorship confound H58 recorded, closed by conditioning on one fixed population: the sides that can fill every size, which by monotonicity are the sides that can fill the largest. On the real feed, over that population: (a) the quote move per share rises across 100, 500, 1000, 5000 and 10000 shares, which is the fourth brief's power above one; (b) the average price paid per share rises across those sizes; (c) mean shares per level does not fall with distance from the touch, that being the shape which makes cost per share fall rather than rise; (d) conditioning changes the shape of the per-share series against the unconditioned one, so the confound was material; if the shapes match, H58's refutation stands as measured rather than as confounded | (a), (b), (c) refuted, (d) supported | Fixed population 6,517,671 sides, 13.90% of quotable. (a) quote move per 100 shares 0.0743, 1.0391, 1.3571, 0.6732, 0.9337: reversal at 1000 to 5000. (b) paid per 100 shares 0.0488, 0.5252, 0.6100, 0.3922, 0.3673: reversals at 1000 to 5000 and 5000 to 10000. This refutes the brief's functional form, not just its exponent: under a power law the cost per share is monotone in size for every exponent, so a series that rises then falls admits no exponent at all. Peak near 1000 shares. (c) mean shares per level from the touch 5103.6, 6297.5, 5809.0, 4799.1, 3554.6, 2668.1, 1990.1, 1671.8: depth peaks at the second level then thins at every further level, down 73.45% to the eighth, a factor of 3.767; the registered claim was that it does not fall. The synthetic feed is inverted, thickening 502.5 to 1789.7, a factor of 3.562, and a 10000 share walk visits 4.344 levels on the real feed against 10.772 on the synthetic one, so a synthetic generator of this kind does not reproduce real depth shape. Second correction against my own registered reasoning: a mean profile does not fix the cost shape, since mean first-level depth is 5103.6 shares yet filling 1000 still takes 1.478 levels, so the distribution of depth at the touch governs it, not the average; the non-monotone cost is left unexplained rather than attributed. (d) supported: unconditioned series peak at 500 and fall thereafter, conditioned peak at 1000 and rise twice first, so the confound was material and H58's caveat was warranted. H58 (a) and (b) keep their refuted and confounded verdicts and are superseded for quotation by H60 (a) and (b); the open item H58 created is closed. results/2026-11-15_m4_conditioned_impact.txt |
| H61 | The padding question H3b and H14 left unsettled, under the independent-launch protocol: 30 launches of `titan-stream`, AC power, PMU, paired within each launch so code layout and allocator placement cancel. Index placement is already controlled in the code (`Indices` is 128-byte aligned, so pad=64 puts both cells in one line and pad=128 does not, verified at run time by `same_line`). On the M4: (a) in uncached AcqRel streaming, pad=64 is cheaper in cycles per message than pad=128 in at least 21 of 30 launches (two-sided sign test p = 0.0428 at that count); (b) the same for cached AcqRel streaming; (c) the median paired difference in the uncached AcqRel case is at least 10% of the median pad=128 cycles per message; (d) L1D load misses per message are lower for pad=64 in at least 21 of 30 launches, uncached AcqRel, which would be the mechanism: one 128-byte line touched instead of two | (a), (b), (c) refuted, (d) supported | Placement verified in output: pad=64 reported same_line=1 and pad=128 same_line=0 in every case. (a) uncached AcqRel, pad=64 cheaper in 16 of 30 launches, p = 0.8555, median paired difference +0.055 cycles [-0.495, +0.625]: no effect. (b) cached AcqRel, 14 of 30, p = 0.8555, -0.090 cycles [-0.480, +0.355]: no effect. (c) 0.055 against a pad=128 median of 24.630 cycles is 0.22%, not the registered 10%. (d) uncached AcqRel L1D load misses lower for pad=64 in 30 of 30 launches, p < 0.0001, +0.311 [+0.239, +0.387]. BUT (d) is confirmed as a fact and REJECTED AS THE EXPLANATION for (a) to (c): the two are decoupled in both directions. Where the miss difference is cleanest (AcqRel) there is no cycle difference; where the cycle difference is largest (SeqCst) there is no miss difference (+0.042, 17 of 30, p = 0.5847). Second mechanism in two days that measured true and explained nothing, after H60 (c). THE UNREGISTERED RESULT: the padding answer depends on the memory ordering, uncached cycles per message, pad=128 against pad=64, paired over 30 launches: acqrel 24.630 vs 24.790 (+0.055, 0.2%, 16 of 30, p = 0.8555); fenced 89.110 vs 86.485 (+4.335, 4.9%, 23 of 30, p = 0.0052); seqcst 167.280 vs 93.920 (+71.215, 42.6%, 29 of 30, p < 0.0001). Cached, the Fenced sign flips to pad=128 (8 of 30, p = 0.0161). So padding to the coherence granule is correct for some orderings and inverted for others on this core. Coherence traffic rather than cache fills is offered as a candidate explanation and not claimed, since testing it needs a snoop event rather than L1D_LOAD_MISS. results/2026-11-15_m4_launch_protocol.txt RUN LENGTH DEPENDENT: measured with titan-stream at 200,000 messages, where H67 showed the performance core runs at 1.085 GHz because the run finishes before the frequency governor responds. The cached conclusions hold at every run length tested. Any UNCACHED ordering comparison from this row does not generalise: at 5,000,000 and 10,000,000 messages SeqCst and AcqRel cost the same to within 1.3 percent and Fenced becomes 2.2x cheaper than AcqRel rather than 3.6x more expensive. See results/2026-11-16_m4_runlength.txt. |
| H62 | H30b repeated under controlled conditions, since its own record says the run was noisy and the direction not established: `titan-pipeline file=BX from=34200 limit=100000`, once `rates=13271` and once `speed=1`, waits wfe, spin and isb, 5 epochs, 5 independent launches, AC power, nothing else running. (a) the p99 under the real timestamps is within 1.5x the p99 at a constant 13,271 msg/s for all three waits, as H30b found; (b) for each wait the real-timestamp p99 varies by less than 1.25x between the highest and lowest launch, so the run-to-run instability that weakened H30b is absent; (c) the constant-rate arm varies by less than 1.25x across launches for each wait, so any instability found in (b) belongs to the burst structure and not to the harness | (a) supported but uninformative, (b), (c) refuted | (a) median p99 over 5 launches, real timestamps against constant 13,271 msg/s: isb 2,083 vs 1,889 ns (1.103x), spin 2,088 vs 1,913 (1.091x), wfe 2,018 vs 1,870 (1.079x), all inside 1.5x. (b) refuted: real-timestamp isb spans 1,843 to 2,514 ns, 1.364x, above the 1.25x registered for all three waits (spin 1.135x and wfe 1.103x pass). (c) refuted too, and in a different wait: constant-rate spin spans 1,877 to 2,521 ns, 1.343x (isb 1.150x, wfe 1.019x pass). Both failing in different waits rules out the burst structure as the cause, which is what (c) was written to detect, and locates the instability in the machine and harness at this latency scale. CONSEQUENCE, as pre-registered: the arrival-structure effect (1.079x to 1.103x) is smaller than the launch-to-launch noise floor on the same quantity (1.364x), so the direction cannot be established on this machine and H30b is weakened accordingly. (a) is recorded as supported because its threshold was met, while noting a 1.5x threshold cannot discriminate against a 1.364x noise floor. Gained instead, measured rather than assumed: at a p99 near 2 us on this fanless machine, five independent launches of identical work differ by up to 1.36x. No later claim in this work about a smaller effect is admissible without more launches. results/2026-11-15_m4_launch_protocol.txt |
| H63 | What waiting costs in energy on a fanless chip, the half of the wait-strategy question no counter in this work has touched. `titan-pipeline` with no PMU, so `powermetrics` can own the counters, 3 independent launches of 7 arms each: idle, then wfe, spin and isb at 13,271 msg/s and at a saturating offered load. (a) at 13,271 msg/s, mean CPU power above the idle baseline is lower for wfe than for both spin and isb, in all 3 launches; (b) the median ratio of spin to wfe power above idle is at least 2; (c) at saturating load, where the trading thread never waits, the three waits fall within 1.25x of each other, so any difference in (a) belongs to waiting and not to the work; (d) P-cluster active residency at 13,271 msg/s is lower for wfe than for spin and isb in all 3 launches, which would be the mechanism the OS can see; (e) each arm's power varies by less than 1.25x across the 3 launches, so the comparison sits above the launch-to-launch noise floor H62 measured | (a), (b), (c) supported, (d), (e) refuted | First energy measurement anywhere in this work. 3 launches, 21 arms, no PMU in the binary since powermetrics owns the counters. (a) at 13,271 msg/s wfe draws less than spin and isb in all 3 launches above that launch's own idle: wfe 2734.4/1859.0/2702.2, spin 6572.5/5275.3/6194.7, isb 4655.4/3667.6/4601.9 mW; ordering also holds unsubtracted. (b) median of per-launch spin/wfe ratios 2.404 against threshold 2 (2.403, 2.838, 2.292); robust, ratio of medians 2.292 and of raw means 2.232. (c) at 20M msg/s offered the three waits fall within 1.0021x, 1.0225x, 1.0153x raw against 1.25x, so the difference in (a) belongs to waiting and not the work. (d) REFUTED: launch 2 has wfe residency highest (90.6 against 90.1 and 90.2). The real result is larger than the refutation: mean P-cluster active residency is 90.0% wfe, 90.3% spin, 90.4% isb, identical to within half a point while power differs 2.26x, and wfe runs at a HIGHER clock than spin (3965.2 against 3730.3 MHz). Parking the core saves energy without the core becoming idle in anything macOS reports and without clocking down; the saving is in what the core executes while waiting. RESIDENCY IS THE WRONG PROXY FOR ENERGY ON THIS CORE. Third mechanism in three days that measured true and explained nothing, after H60 (c) and H61 (d). (e) REFUTED by the baseline arm alone: idle spread 26.062x (Chrome was running and was not quit; launch 2's idle window shows P-residency 8.9% against launch 1's 0.0%). All six comparison arms are stable to within 4.9%, the baseline is a 1% to 16% correction rather than the signal, and every verdict was checked with and without subtraction and does not change. Absolute energy per message is not trustworthy from this run; the ratios between waits at the same load are. Usable for the paper: the trading core draws 2.84 W on wfe, 4.72 W on isb and 6.42 W on bare spin at feed rate, and the three are indistinguishable within 3% at saturation. results/2026-11-15_m4_energy.txt |
| H64 | The cross-core handoff under the independent-launch protocol, re-measuring the four research questions that currently rest on rows with no results file. 30 launches of `titan-pingpong 200000 3 ui pmu` and 30 of the same with `bg`, AC power, paired within launch. (a) RQ1: the median across launches of the per-launch SeqCst to AcqRel ratio of round-trip cycles lies inside 0.90 to 1.10 with the consumer caching the producer's index, and above 1.10 without it; (b) RQ2: wfe has a lower median round trip than isb spinning in at least 21 of 30 launches (two-sided sign p = 0.0428 at that count); (c) RQ3: the median across launches of the per-launch BACKGROUND to USER_INTERACTIVE round-trip ratio is below 3; (d) RQ4: under SeqCst without index caching, pad=64 is cheaper than pad=128 in at least 21 of 30 launches, as it is in streaming; (e) Fenced is the slowest of the three orderings in at least 21 of 30 launches | (a) refuted, (b) supported on P-cores and reversed on E-cores, (c) registration ambiguous, not verdicted, (d) supported, (e) supported in 7 of 8 configurations | Census on the measurement machine first: pop_acqrel ldapr + stlur, pop_seqcst ldar + stlur, pop_fenced dmb x2, push_acqrel ldapur + stlr, push_seqcst ldar + stlr, wait_wfe ldxr + sevl + wfe x2. RQ1 is provably measuring ldapr against ldar against dmb ish. MY REGISTRATION WAS DEFECTIVE: parts (b), (c) and (e) name a count or bound without naming the configuration or metric, and the answer differs by both. Reported under every reading the wording admits. (a) refuted: seqcst/acqrel cycles ratios are 1.000, 0.998, 1.041, 1.001 cached and 0.999, 0.997, 1.014, 0.955 uncached, all inside 0.90 to 1.10, so the second half fails. THE RESULT IS THE CONTRAST: the same orderings cost 6.79x and 3.79x in the streaming harness uncached and 0.955 to 1.041 here. The cross-core round trip is 452 to 510 cycles of coherence transfer, which swamps the ordering instruction; streaming has no such floor. Ordering cost is visible in a throughput-bound pipeline and invisible in a latency-bound handoff. (b) on USER_INTERACTIVE isb costs more than wfe in 30/30, p < 0.0001, 351.55 against 271.15 cycles, wfe 1.30x cheaper; on BACKGROUND the direction flips, 0/30, wfe 1.06x more expensive. (c) not verdicted: bg/ui ratio is 0.932 median in cycles (below 3) and 3.057 median in wall time (above 3), from the same 60 launches. The E-core runs at 1.080 GHz with interval [1.080, 1.080] over 30 launches while the P-core runs at 3.506 to 3.628 GHz, a 3.28x clock ratio, so the E-core finishes the round trip in FEWER cycles (0.808 to 0.984) and about 3x the wall time. Strongest case in the project for reporting both units. Also answers H8's question, whose own run is unquotable: the wall-time ratio is 3.057, so that claim holds in time and fails in cycles. (d) supported: pad=64 cheaper under SeqCst uncached in 28/30 on E-cores and 29/30 on P-cores, and in six further configurations at 24 to 30 of 30; two reverse, fenced uncached on P-cores (5/30) and SeqCst cached on E-cores (6/30). (e) fenced costs more than acqrel at 24 to 30 of 30 in seven configurations, worst 1.250x at pad=128 cached on P-cores; the eighth, pad=128 uncached on P-cores, shows no effect at 12/30, p = 0.3616. RQ3 answered directly: involuntary switches per 200,000 round trips, median 0 for the lock-free ring on P-cores (max 89) and 1 to 3 on E-cores (max 516), against 36 on P-cores (max 603,073) and 53,658 on E-cores (max 1,229,115) for the mutex baseline. XNU never preempts the spinning lock-free threads on P-cores in the median launch. The P-core runs at 3.506 to 3.628 GHz during ping-pong, not 4.4, so every cycles-to-nanoseconds conversion must use the measured clock of its own arm. results/2026-11-16_m4_crosscore.txt |
| H65 | A pricing kernel is the decision an options market maker makes, so its cost decides whether the trading path measured here is representative: (a) the six separate sensitivity functions cost at least 2.0x the fused kernel in cycles per contract; (b) the cumulative normal is at least 20 percent of the fused kernel; (c) scalar single precision does not cut cycles per contract by more than 10 percent, so the ratio is above 0.90; (d) hoisting the strike-independent terms by hand cuts cycles per contract below 0.90 of the per-contract loop; (e) the fused kernel exceeds 135 cycles per contract, the cycles per message of the whole single-core trading path, in at least 21 of 30 launches; (f) the straddling strike ladder costs at least 2.0x the out-of-the-money ladder in branch mispredicts per contract | (a) supported, (b) supported, (c) refuted, (d) refuted on P-cores and reversed on E-cores, (e) refuted and it refutes this run's premise, (f) supported but the magnitude is negligible | Validated before measured: cumulative normal 7.448e-8 worst against compensated Simpson integration of the density, every sensitivity within 0.352 of a tolerance whose absolute part is the central difference's own cancellation floor, put-call parity 4.441e-16, fused against separate 2.165e-16, ladder against scalar 5.684e-16. ABSOLUTE COST, P-cores, straddling ladder, median of 30 launches, per contract: cdf 15.87 cycles, price 45.37, fused 58.73, separate 134.11, f32 44.06, chain 58.04, loop 59.53; IPC 4.93 to 5.78 and L1D misses under 0.017 everywhere, so the kernel is arithmetic-bound. (e) REFUTED 0/30 against a registered 21/30: the fused kernel is 58.733 cycles [58.668, 58.800], not above 135. A full Black-Scholes price with all five sensitivities costs 43 percent of the whole feed-to-decision path, so the premise that a real options decision would blow the per-message budget is wrong by 2.3x in the opposite direction, far outside H62's 1.364x noise floor. A 64-strike ladder reprices in 3715 cycles, about 832 ns. The 135 is a PRELIMINARY PROTOCOL figure, so this is unpaired against a fixed number and is to be redone when the path is re-measured. (a) supported: separate costs 2.284x fused, 30/30, p<0.0001, +75.343 cycles [+75.275, +75.445], 748.19 instructions against 298.78; 2.426x on E-cores; the two agree to 2.165e-16 so the reuse is free. (b) supported: the cumulative normal alone is 0.270 of the fused kernel, 0/30 above, p<0.0001, 15.87 cycles per evaluation. (c) REFUTED and my registered mechanism was wrong: scalar f32 is 0.750 of f64, not above 0.90, 0/30, p<0.0001. The saving is not in arithmetic width but in shorter library routines, 254.17 instructions against 298.78, plus IPC 5.77 against 5.09. 0.826 on E-cores. Against the validated accuracy loss (3.3e-7 on price, 2.5e-5 on gamma) this is a quotable tradeoff. (d) REFUTED as the registration predicted: hand-hoisting gives 0.976, not below 0.90, saving 1.427 of 59.5 cycles, certain (27/30, p<0.0001) and practically nothing, because LLVM already hoists the loop-invariant terms at opt-level 3 with fat LTO; the hand-hoisted form issues MORE instructions (303.59 against 296.78) for the chunk bookkeeping. Reversed on E-cores at 1.021, 28/30. A finding about the compiler, not the kernel. (f) supported at 64.4x against a 2.0x threshold, 30/30, p<0.0001, but 0.0322 mispredicts per contract against 0.0005 costs 0.31 cycles of 58.73, half of one percent. Reported with its magnitude because the ratio alone would mislead. OPEN: the price arm mispredicts twice as often as the fused arm on the same two cumulative normals, and its ladder-to-ladder cycle gap is 3.12 against 0.31; if-conversion would explain it but was not measured. THE CORE CLOCK IS NOT A MACHINE CONSTANT: P-cores ran this compute kernel at 4.05 to 4.61 GHz against 3.506 to 3.628 for H64's cross-core round trip, 27 percent apart, so every cycles-to-ns conversion must use its own arm's measured clock. E-cores held 1.088 GHz except the heaviest arm, which boosted unevenly to 1.542 and whose E-core wall time must not be quoted as a point estimate. Cycles and nanoseconds agree throughout here (2.284 against 2.300, 0.750 against 0.747, 0.976 against 0.976), so H64's divergence between the units was specific to a latency-bound handoff, not general. results/2026-11-16_m4_greeks.txt |
| H66 | Path dependence is accumulated one step at a time, so what it costs per step decides whether an exotic payoff is affordable at feed rate: (a) geometric averaging, additive in logarithms, costs below 0.70 of arithmetic averaging in cycles per step; (b) a barrier monitored in log space costs below 0.70 of the same barrier monitored in price space; (c) branchless extremes are NOT faster than branchy ones, ratio above 0.95, because the theory of records puts a new extreme at about 1/k on step k; (d) stopping at the first touch costs between 0.55 and 0.95 of running to maturity in cycles per path; (e) a stored path costs above 1.10 of rolling scalars with L1D misses per step below 0.05 in both; (f) tracking every statistic costs below 1.50 of generating the path alone | (a) refuted, (b) refuted, (c) supported and statistically null as predicted, (d) supported, (e) refuted in the opposite direction, (f) refuted by 3 percent | AC power confirmed at the start AND end of the run. Validated first by ten oracles sharing none of the engine's machinery: chi-square 57.98 on 63 df, KS 0.001624 against 0.002150 critical, exact process moments within 1.66 se, European against Black-Scholes within 1.12 se, geometric Asian against an exact DISCRETE closed form within 1.92 se, in-out parity below 5.2e-15, unreachable barrier reproducing the plain generator. ABSOLUTE COST, P-cores, per step: terminal 23.14 cycles, asian_geom 26.77, asian_arith 34.41, lookback_free 27.02, lookback_branchy 27.08, barrier_log 27.18, barrier_price 34.87, barrier_log_exit 18.09, rolling 35.74, stored 32.98. L1D misses 0.0000 in EVERY arm including the one writing 2 KiB per path, so the engine is arithmetic-bound. THE NUMBER THIS RUN CONTRIBUTES is not a registered part: one exponential costs 7.647 and 7.693 cycles measured from two independent pairs of arms, agreeing to 0.60 percent, both 30/30 with p<0.0001. That decomposes H65's 15.87-cycle cumulative normal into 7.67 for the exponential and 8.20 for the polynomial, density and sign branch. Two runs registered three days apart on different harnesses cross-validate. (a) refuted: 0.7778 against a registered 0.70, 30/30, p<0.0001, 0.598 on E-cores. My mechanism was right and my magnitude wrong: I read an exponential across from H65 as 13 to 16 cycles and it is 7.67, so removing it from a 34.41-cycle step leaves 0.778. (b) refuted at 0.7794 for the identical reason, 30/30; the two refutations are one, both registered against the same overestimate. (c) SUPPORTED and the registration's best result, because it predicted an optimization would fail and named the mechanism first: the theory of records puts a new extreme at 1/k on step k, so about ln(250)=5.5 records in 250 steps, 2.2 percent of steps, well predicted. Ratio 0.9980, and on P-cores the effect is statistically absent, 11/30, sign p = 0.2005, bootstrap interval [-0.109, +0.005] containing zero. Small real effect on E-cores (0.985, 30/30, 0.627 of 41.67 cycles). (d) supported: 0.6657 in the registered 0.55 to 0.95 band, 30/30. Observations fall 250 to 187.8 (24.9 percent) against a 33.4 percent cycle saving, above the 0.538 floor a free saving would reach, as registered. Side effect: the early-exit arm mispredicts LESS with a near barrier (0.295 against 0.385, 30/30) because knocked-out paths run fewer loop-condition branches. (e) REFUTED IN THE OPPOSITE DIRECTION and the most useful finding: a stored path is 7.7 percent CHEAPER than rolling scalars (0.9227, 30/30) where above 1.10 was registered. The registered L1D sub-part is supported, 0.0000 misses in both, 30/30, which was registered precisely so the result could not be blamed on misses afterwards. Instructions are within 1.3 percent (158.32 against 156.31) and IPC differs 7.0 percent (4.429 fused against 4.740 split): in the fused loop the exponential sits on the dependency chain of the path recurrence, and splitting it lets independent exponentials pipeline. 27.5 percent cheaper on E-cores. The advice that prompted this measurement, to avoid storing paths for memory efficiency, is measured here as advice to pay 7.7 percent for memory that was free. (f) refuted by 3.0 percent at 1.5447, 30/30, with the mechanism closing to the cycle: rolling adds 12.604 cycles, of which the exponential is 7.647 (60.7 percent) and the log sum, min, max and barrier compare are 4.957, about 1.24 each. The exponential dominates as registered; it is not large enough relative to the other four to hold the total under 1.50. POWER CONDITIONS DID NOT MATTER: this repeats a battery run from four hours earlier and the two agree within 0.16 percent on every absolute and 0.13 percent on every ratio, with P-core clock 4.045 to 4.062 GHz on AC against 4.044 to 4.049 on battery. Compatible with H61, whose reversals were latency-bound cross-core work where the clock differed between arms. Limits stated: these runs are milliseconds long and never reach sustained-load throttling on a fanless chassis, and this is two runs four hours apart rather than a controlled power experiment. The AC requirement stays. OPEN: branch mispredicts sit at 0.390 per step in every arm including bare generation, differing between arms by at most 0.010. Common to all arms so it confounds nothing, but at roughly 10 cycles a mispredict it would be about 4 of bare generation's 23.14 cycles and its source is not established. results/2026-11-16_m4_paths.txt |
| H67 | Two claims this work has published without closing, and one it rests on: (a) the index-caching benefit does not depend on how many messages pass through the ring, so its uncached-to-cached cycle ratio varies by less than 1.5x across 200,000 to 10,000,000 messages; (b) the 135 cycles per message of the full trading path, measured with five epochs in one process, reproduces within 10 percent over 30 independent launches; (c) the ring's share of that path is at most 1.10x, as H19 measured at 1.067 under the preliminary protocol | (a) refuted by a factor of 11 and it invalidates RQ1's published uncached figures, (b) supported within 0.6 percent, (c) supported within 0.4 percent | AC power confirmed at the start AND end. MY OWN DESIGN DEFECT FIRST: I registered epochs=2, whose median is the MEAN of two epochs and so averages the cold first one in, where H61 used 3 epochs whose median selects the warm middle. At 200k, epoch 0 is 217.190 cycles and epoch 1 is 21.930, so the median of two is 119.560 and is a figure for neither. Every streaming number is therefore read from epoch 1, the quantity H61's median of 3 selected. (b) SUPPORTED within 0.6 percent and it is the part that mattered most: stage=full 134.178 cycles [134.045, 134.332] over 30 launches against the original 135; direct 126.143 against 126.97; book 118.692 against 119.05; full 32.756 ns [32.639, 33.101] against 32.34 ns. Three PRELIMINARY PROTOCOL figures reproduce within 0.6, 0.7 and 0.3 percent, so H65's headline (a pricing kernel is 43 percent of the whole path) is sound as stated. (c) SUPPORTED: the ring's share is 1.063, +7.952 cycles [+7.572, +8.438], 30/30, p<0.0001, against a 1.10 ceiling; H19's 1.067 reproduces within 0.4 percent. (a) REFUTED by 11x: the uncached-to-cached ratio is 1.66, 6.91, 26.41, 27.17 at 200k, 1M, 5M, 10M, a 16.33x spread against a registered 1.50x. My mechanism, that the ring's loop body does not depend on how many messages pass through it, was wrong. Both published figures are vindicated: H61's 1.69x at 200k reproduces at 1.66 (2 percent) and the preliminary 26.2x at 10M at 27.17 (4 percent). The README caveat I wrote was right and is now controlled. THE FINDING THAT MATTERS MORE THAN THE HYPOTHESIS: RQ1'S PUBLISHED UNCACHED ANSWER IS A 200,000-MESSAGE ARTIFACT. The warm-epoch core clock is 1.085 GHz at 200k, 2.790 at 1M, 4.332 at 5M, 4.315 at 10M: a 200k run finishes before the frequency governor responds. The streaming harness has ALWAYS been run at 200k; the tick-to-trade harness has always been run at 20M, which is why its figures reproduce and streaming's do not. SeqCst/AcqRel uncached is 8.153, 1.530, 0.987, 0.997 across the four counts, so at 5M and 10M the two orderings cost the SAME to within 1.3 percent in cycles and in nanoseconds; H61's 6.79x exists only at an unramped clock. The cached half survives: 1.187, 1.366, 1.015, 1.030, so ordering being invisible with the index cached holds at every length. AND THE RANKING INVERTS: uncached cycles per message are AcqRel 21.930/133.860/216.195/221.990 against Fenced 86.280/99.750/99.415/99.185, so AcqRel grows 10.1x with run length while Fenced grows 1.15x and is flat past 1M. Fenced costs 3.93x AcqRel at 200k and 0.447x at 10M, two and a quarter times CHEAPER. In nanoseconds, not a clock artifact: AcqRel uncached 20.068 to 51.007 ns rising, Fenced uncached 45.075 to 22.726 ns falling. MECHANISM, one component established and one not: the clock ramp is quantified (3.99x from 200k to 5M) and distorts any cycles figure for a memory-waiting workload, and the uncached arms carry 2.66 to 3.04 L1D misses per message against the cached arms' 0.91 to 1.30. The nanosecond divergence is NOT explained; L1D misses move only 14 percent across the range, which cannot account for a 2.5x slowdown and a 2.0x speedup. Recorded as the project's largest open question and not claimed. NOTHING IS WITHDRAWN: H61's numbers are correct for the run length they were measured at, which is now stated beside them; what changes is the generality claimed. results/2026-11-16_m4_runlength.txt |
| H68 | RQ1 re-measured at a run length the feed implies, because H67 showed its published uncached answer is a 200,000-message artifact measured on a core at 1.085 GHz: at 10,000,000 messages and 3 epochs, (a) SeqCst and AcqRel cost within 10 percent of each other per message uncached, in cycles and in nanoseconds; (b) Fenced costs below 0.60 of AcqRel uncached, inverting the published ranking; (c) the ordering stays invisible with the index cached, all three within 10 percent of each other; (d) the warm-epoch core clock is above 4.0 GHz in every arm, confirming the run is long enough to have ramped | (a) supported, (b) supported and it inverts the published ranking, (c) refuted and it corrects an overstatement in H67, (d) failed as registered and the check was defective | AC power at the start AND end. 30 launches of titan-stream 10000000 3 pmu, 1170 lines, 3 epochs so each launch reduces to its warm middle epoch as the paper's protocol states. (d) FAILED AS WRITTEN and THE CHECK WAS DEFECTIVE: I required every arm above 4.0 GHz and the lowest is 3.537. The clock is a STEADY STATE not a ramp (spread 1.000x to 1.135x across epochs, 11 of 13 arms inside 1.02x) and it tracks epoch DURATION not message count: arms running 218 to 510 ms sit at 4.27 to 4.40 GHz, arms running 22 to 52 ms at 3.54 to 3.99. Every arm processes 10M messages; the cached arms finish in 23 ms because they are 22x faster per message. A per-message threshold cannot control a per-duration phenomenon. The question it was for is answered yes and not closely: the lowest arm is 3.26x the 1.085 GHz idle clock and every UNCACHED arm, where the disputed figure lives, is 4.271 to 4.396 GHz. Corrected check for any future run: every arm's epoch duration above 200 ms. (a) SUPPORTED: uncached pad=128, AcqRel 221.070 [219.890, 221.880] against SeqCst 220.445 [217.030, 221.760], ratio 0.997 in cycles and 1.007 in nanoseconds (50.701 against 51.032). The RCsc and RCpc acquire loads cost the same to three parts in a thousand. The 6.79x this work published is confined to run lengths where the core has not ramped. RQ1's uncached answer is that the ordering costs nothing. (b) SUPPORTED and it is the result a reviewer will disbelieve: Fenced costs 0.4475 of AcqRel uncached in cycles (98.935 against 221.070) and 0.4464 in nanoseconds. H67's two-epoch sweep gave 0.447 at the same count, so two runs with different epoch counts agree to one part in a thousand. Adding two dmb ish per ring operation makes the uncached loop 2.23x FASTER, and Fenced is the only ordering whose cost does not grow with run length (98.9 cycles here against 86.3 at 200k, while AcqRel goes 21.9 to 221.1). (c) REFUTED, and it corrects an overstatement I made in H67 hours earlier: cached pad=128, AcqRel 8.465, SeqCst 8.290 (0.979, inside 10 percent), Fenced 20.735 (2.449, far outside). H67's file says the cached half of RQ1 survives on the strength of the SeqCst to AcqRel ratio alone; the third ordering in the same table costs 2.45x AcqRel. The data to catch it was in H67's own sweep (Fenced cached 20.480 against AcqRel 8.170 at 10M, 2.507x). I had it and did not check it. THE CORRECTED RQ1 ANSWER: between the two acquire forms there is no difference worth measuring at either caching setting (0.979 cached, 0.997 uncached), so the ldapr against ldar distinction the census proved is in the binary does not show up in cost. The explicit double barrier FLIPS SIGN: 2.45x more expensive than AcqRel when the index is cached and 0.45x when it is not. Mechanism NOT established, and the obvious explanation is ruled out: Fenced carries MORE L1D misses than AcqRel in every configuration (3.785 against 3.031 uncached, 1.418 against 0.913 cached) and is faster uncached anyway. PADDING ALSO DEPENDS ON RUN LENGTH and two of three per-ordering answers REVERSE against H61: uncached pad=64 cheaper for AcqRel 26/30 p=0.0001 (+36.865 cycles) where H61 found 16/30 none, SeqCst 27/30 p<0.0001 (+35.820) where H61 found 29/30, Fenced 14/30 p=0.8555 none where H61 found 23/30 strong. Cached, pad=128 wins instead: AcqRel 27/30, SeqCst 24/30, Fenced none. Sharing one granule is worth about 17 percent uncached for both acquire forms and nothing for the barrier. Counterintuitive detail ruling out the easy explanation: pad=64 carries MORE L1D misses than pad=128 in every case (0.009 to 0.512 per message) and is faster uncached anyway. OPEN: why AcqRel's uncached cost grows tenfold with run length while Fenced's grows 15 percent. L1D misses are ruled out in both directions. Needs a counter sweep reading store-buffer and coherence events, and whether the M4 PMU exposes them is not known here. results/2026-11-16_m4_rq1.txt |
| H69 | Which half of the acquire-release pair carries the Fenced inversion, by crossing the two sides: FencedLoad is `ldr`+`dmb ish` with `stlr`, FencedStore is `ldapr` with `dmb ish`+`str`, and Unordered is a relaxed cost floor that is not a correct ring. At 10,000,000 messages, 3 epochs, pad=128, performance cores, cached and uncached: (a) the uncached saving is store-dominant, store-side share at or above 0.70 with 21 of 30; (b) it is load-dominant instead, load-side share at or above 0.70 with 21 of 30, which at these thresholds cannot both hold with (a); (c) the two sides are additive, shares summing to 1 within 0.15; (d) each side alone reaches 21 of 30 in the same direction; (e) the cached sign change reproduces above 2.0 and localises to the same side; (f) the Unordered arm is a usable floor at all, that is (A-U)/A is positive, REVISED 2026-11-17 from an 0.30 size threshold after the x86 control showed the quantity is negative and the hypothesis malformed; (g) every UNCACHED arm's epoch duration is above 200 ms with the cached arms recorded without a pass or fail, REVISED 2026-11-17 from a global threshold, which is the fourth version of this check | (a) SUPPORTED, store-side share 1.0702 [1.0621, 1.0866], 30/30, sign test 30/30 p<0.00001. (b) REFUTED, load-side share 0.2797, 0/30; the thresholds were set so (a) and (b) could not both hold. (c) REFUTED uncached, S+L = 1.3587, inside 0.15 of 1 in 1/30, strongly sub-additive (168.691 cycles saved separately, 124.544 together). SPLIT cached: SUPPORTED on cycles (1.1133, 28/30) and REFUTED on nanoseconds (1.2333, 1/30), because the cached arms ran across a 1.122x clock spread, so a ratio of time differences is not a ratio of work differences. Cycles are primary, so cached (c) is SUPPORTED on cycles with the disagreement recorded. (d) SUPPORTED, both sides 30/30 same direction. (e) SUPPORTED, cached F/A = 2.554 against the registered 2.0 and H68's 2.449, and S is 1.1207 cached against 1.0702 uncached, so one mechanism spans the sign change, not two. (f) SUPPORTED, (A-U)/A = +0.9534, positive 30/30; ORDERING IS 95.3 PERCENT OF THE UNCACHED LOOP, 218.880 cycles against 10.185. (g) FAILED AND THE CHECK IS DEFECTIVE FOR THE FIFTH TIME: 99/150 uncached arm-launches above 200 ms, failing on fencedstore at a 194.6 ms median whose clock was 4.368 GHz, squarely with the other three. The revision of (g) named the recurring error in writing, as one global threshold for a per-arm quantity, and then committed it again across five arms that differ in cost by 21x by design. The verdicts above do not depend on it: the four ordered uncached arms ran within 1.094x of each other, 4.035 to 4.414 GHz, over all 540 of their lines, which answers directly what (g) was built to protect | HEADLINE: the effect is ENTIRELY STORE-SIDE. Replacing `stlr` with `dmb ish` plus a plain `str` accounts for 133.611 of the 124.544 cycles saved; replacing `ldapr` with `ldr` plus `dmb ish` saves 35.080, real at 30/30 but a quarter the size. The order of the four ordered arms REVERSES COMPLETELY between configurations: uncached fencedstore 85.269 < fenced 94.336 < fencedload 183.800 < acqrel 218.880, cached fencedload 8.021 < acqrel 8.053 < fenced 20.567 < fencedstore 22.120. INTEGRITY: all 180 sum_ok=0 lines are the unordered arm, 90 per configuration; the four ordered arms were exact in 180 of 180 lines each. The unordered arm lost data in 180 of 180 epochs here and in 0 of 30 launches on the x86 control, so weak ordering breaks this ring observably on AArch64 while total store order masks the same bug. POST HOC, NOT REGISTERED: fencedstore is 8.829 cycles cheaper than fenced uncached at 29/30, so adding the load-side barrier on top of the store-side barrier costs 10.6 percent, and cached it helps by 7.0 percent. MECHANISM REMAINS A HYPOTHESIS: a release store must wait for prior accesses to be observable, which with a coherence miss in flight is consistent with a stall, where a barrier plus a plain store pays once and retires into the store buffer. Not established; a queue-occupancy explanation is not excluded | results/2026-11-17_m4_barrier.txt |
| H70 | What arrival clustering alone costs the tail, with the mean rate, the message content, the book and the code all held fixed and only the arrival times differing. Hawkes branching ratio as the dial, measured Fano 0.900, 3.241 and 13.920 at 0.0, 0.5 and 0.8 with the realised rate inside 0.8 percent of the requested one at every setting. Reported as a CCDF of 65 log-spaced tail quantiles per run rather than a seven-point percentile ladder, each line carrying the number of observations beyond it. At 10,000,000 messages, 3 epochs, 30 launches, rate 5,000,000 per second: (a) p99.9 at branching 0.8 is at or above 2.0 times p99.9 at branching 0.0, 21 of 30; (b) median p99.9 is ordered 0.0 below 0.5 below 0.8 in at or above 24 of 30 launches; (c) p50 at branching 0.8 is within 1.25 times p50 at branching 0.0, so the arms differ in tail and not in throughput; (d) p99.9 at branching 0.0 is within 1.5 times the periodic rates= arm, separating the cost of random arrival times from the cost of clustered ones; (e) every quantile read carries at least 100 observations beyond it; (f) no arm reports saturated=1, and a saturating arm is dropped and reported rather than the rate being lowered until it passes | (a) REFUTED and not by a margin: p99.9 ratio 0.8 against 0.0 is 1.0060 spin, 0.9872 isb, 1.0285 wfe against a registered 2.0, at or above 2.0 in 1/30, 2/30 and 1/30, and the sign test reaches 16/30 p=0.856, 17/30 p=0.585 and 20/30 p=0.099, so no wait strategy reaches significance either. (b) REFUTED: ordered in 5 of 30 launches for all three waits against a registered 24, which is what chance produces from three interchangeable arms. (c) SUPPORTED and this is what makes (a) mean anything: p50 ratio 1.0113, 1.0000, 1.0081 against 1.25, so the arms are doing identical work at identical speed and the clustering still does not move the tail. (d) SUPPORTED AT THE REGISTERED DEPTH, AND THE REGISTRATION CHOSE THE WRONG DEPTH: 1.0517, 1.0574, 1.0311 at p99.9, but the same two arms over the same launches give 2.122 at the 95.73rd percentile at 30/30, p=1.9e-09. The control passed only because p99.9 is the one depth where the curves have converged. (e) SUPPORTED with two orders of margin: minimum 10,656 observations beyond the p99.9 level against a registered 100. (f) FAILED on 1 line of 1080, launch 25, hawkes_0.5, isb, epoch 2, offered 5.002 against achieved 0.421. Each launch is the median of three epochs so one outlying epoch cannot move it; recomputing (a) for isb with launch 25 removed gives 0.9853 against 0.9872 and 17/29 against 17/30 | WHAT WAS FOUND IS NOT WHAT WAS REGISTERED. Clustering is absorbed completely at every depth: the 0.8 against 0.0 ratio stays between 0.98 and 1.07 across the whole curve and is statistically null past p99.9, so Fano 0.900 and Fano 13.920 produce the same latency distribution. The large effect is RANDOMNESS, not clustering: a Poisson schedule against a perfectly periodic one at the same mean rate costs 1.29x at the median, 1.76x at p85, 2.12x at p96 and 2.60x at p99.64, then collapses to 1.05x at p99.9 and 1.02x in the far tail, at 30/30 or 29/30 with p below 1e-5, reproduced under all three wait strategies within 0.1 of each other. SCOPE: the full path saturates between 10,000,000 and 15,000,000 messages per second on this machine, so 5,000,000 is one third to one half of capacity and every verdict is a statement about that operating point. The same sweep at 10,000,000 is a new registration, not a re-reading of this one | results/2026-11-18_m4_burst.txt |
| E2 | EXPLORATORY, NOT PRE-REGISTERED. The barrier-placement cross run on virtualized x86-64, as a control for H69: on a machine whose fences are a pure cost and whose acquire-release is a plain move, does the harness show the two barrier sides as symmetric and the inversion as absent | Harness and statistics behave. Uncached the two barrier sides cost within 0.7 percent of each other (64.637 against 65.053 ns) and SATURATE rather than add (shares 0.7098 and 0.7102, sum 1.4145, within 0.15 of 1 in only 3 of 30). THREE THINGS THE CONTROL CAUGHT, ALL BEFORE ANY M4 TIME WAS SPENT. (1) A WEAK INVERSION EXISTS HERE TOO: cached, FencedLoad is 1.142x CHEAPER than AcqRel, 23 of 30, p=0.0052, although on x86 an acquire load is a plain move so adding a fence should strictly cost. The paper may therefore NOT claim the inversion is Apple-specific; it may claim it is an order of magnitude larger on the M4 and in the opposite caching configuration. Not trustworthy without the preemption caveat: up to 1133 involuntary switches per epoch on 2 vCPUs with no spare core. (2) H69 (f) WAS MALFORMED, not merely wrong: (A-U)/A is -0.3596 [-0.7430, -0.2321] with AcqRel cheaper than Unordered 24 of 30, p=0.0014, so removing all ordering makes the loop 36 percent SLOWER and the Unordered arm is not a cost floor at all. Rewritten before the M4 run as a check on the arm's sign. (3) H69 (g) WOULD HAVE FAILED ON A CORRECT RUN for the fourth time: cached arms are ~20x faster per message by construction and always run short (shortest 86.559 ms here, ~23 ms expected on the M4), so a global duration threshold across all arms cannot hold. Rewritten to bind on the uncached arms only. Every ordered arm sum_ok=1 in all 30 launches | 30 launches of titan-barrier 10000000 2 in a KVM-virtualized Docker container, 2 vCPUs of an Intel Xeon (family 6 model 207, Emerald Rapids), rustc 1.97.0, no PMU so nanoseconds only. Numbered E because nothing about it was registered and because the host is not the paper's machine | results/2026-11-17_x86_barrier.txt (pending) |
| H71 | What arrival clustering costs the tail AT THE EDGE OF CAPACITY, which is the one regime H70 could not reach. Identical to H70 in every respect but one: the mean rate is 10,000,000 messages per second instead of 5,000,000, which earlier sweeps record as the highest rate this path sustains without saturating. H70 found clustering free at a third to a half of capacity and found the large effect to be randomness rather than clustering; both of those are retested here, and the second is a REGISTERED REPLICATION of a finding H70 made after the fact. Same four arms (periodic rates=10000000, hawkes 0.0, 0.5, 0.8 at burstrate=10000000), same 10,000,000 messages per epoch so the tail is resolved to the same depth, same 3 epochs, same 30 launches, same three wait strategies, same 65-quantile CCDF. (a) PRIMARY, SATURATION ASYMMETRY: clustering drives the system into saturation at a mean rate the periodic schedule sustains, so saturated=1 in at or above 20 percent of the 270 cells of hawkes_0.8 and in at most 2 percent of the 270 cells of rates=10000000. Both halves must hold. (b) TAIL RATIO WHERE BOTH ARMS SURVIVE, conditional on at least 15 of 30 launches yielding a non-saturated CCDF for hawkes_0.8 and hawkes_0.0 at the same wait: p99.9 at branching 0.8 is at or above 1.50 times p99.9 at branching 0.0, 21 of 30. The threshold is 1.50 and not H70's 2.0 because 2.0 was refuted at half this rate and 1.50 is the smallest ratio that would still matter to anyone sizing a queue. (c) MEDIAN CONTROL: p50 at branching 0.8 is within 1.25 times p50 at branching 0.0, so any tail effect is not a throughput artefact. (d) REGISTERED REPLICATION OF H70's POST HOC FINDING, hawkes_0.0 against the periodic arm, paired within launch: the ratio reaches at or above 2.00 somewhere between the 95th and the 99.7th percentile, with at or above 27 of 30 on the sign test at that peak, AND falls below 1.15 at p99.9. All three parts must hold; H70 measured 2.122 at p95.73 and 1.052 at p99.9 at half this rate, so this is a prediction with a number attached and not a look. CORRECTION ADDED 2026-11-19: the H70 figures quoted in this project as 2.604 at p99.64 were the maximum of a six-row tabulation, not of the curve, and were described as the peak. Recomputed over all 65 quantiles the H70 peak is 2.7765 at p99.5524 for spin, 2.7505 at p99.6354 for isb and 2.5430 at p99.5524 for wfe. No quoted number was wrong at the quantile it names; calling it the peak was an overstatement, and the manuscript uses the recomputed values. (e) RESOLUTION: every quantile compared carries at least 100 observations beyond it. (f) FAILURE BRANCH NAMED IN ADVANCE: if every clustered arm saturates in every cell, (b) and (c) are recorded as UNTESTABLE AT THIS RATE rather than retested at a lower rate, (a) stands as the finding, and (d) still runs because hawkes_0.0 and the periodic arm are the two arms likeliest to survive. The rate is not lowered until something passes | (a) REFUTED, and refuted on its second half rather than its first. Registered: saturated=1 in at or above 20 percent of hawkes_0.8's cells AND at most 2 percent of the periodic arm's. Measured over 1,080 cells: hawkes_0.8 saturated in 270 of 270, which satisfies the first half, and the periodic arm ALSO saturated in 270 of 270, which fails the second half completely. The predicted asymmetry is absent because there is nothing to be asymmetric about: at this run length every arm is over capacity. (b), (c) and (d) UNTESTABLE AT THIS RATE, exactly as (f) said they would be recorded. The instrument emits no CCDF for a saturated run by design, so zero of the 65-quantile ladders exist and not one of the three can be read. (d) in particular, the registered replication of H70's post hoc randomness finding, is not refuted and not supported; it was not measured. (e) NOT APPLICABLE: no quantile was read. (f) FIRED AS WRITTEN, and this is the row's main result. The run script stopped, committed nothing, kept the raw file, and did not lower the rate until something passed | THE PRE-FLIGHT IS THE FINDING, AND IT CONTRADICTS THE SWEEP. The same binary, the same rate of 10,000,000 and the same three arms were probed at 2,000,000 messages per epoch immediately before the sweep: the periodic arm achieved 9.848 of 10.000 offered and passed, hawkes_0.0 achieved 10.001 and passed, and hawkes_0.8 achieved 8.488 and saturated. That IS (a)'s predicted asymmetry, present and clean. Five times the messages per epoch and every arm saturates, periodic included. The periodic arm was sitting 1.5 percentage points above the 97 percent gate in the probe, so the gate was crossed by run length and not by rate. THE CEILING IS RUN-LENGTH DEPENDENT, which is this project's signature finding arriving somewhere it was not expected: H63 and the RQ1 family established that duration changes the clock and therefore the conclusion, and here duration changes the sustainable throughput. H70's scope note put capacity between 10,000,000 and 15,000,000; that was measured at a shorter run length and is too generous for a 10,000,000-message epoch. Also recorded: 4 of 30 launches ran long at 110s, 158s, 63s and 100s against a 46s baseline, in a 28-minute continuous sweep on a fanless chassis. The asymmetry observation is ONE pre-flight cell per arm and is POST HOC with respect to this registration; E3 measures it properly | results/2026-11-19_m4_burst10_saturated.txt |
| H72 | WHAT CLUSTERING COSTS AT THE MEASURED CEILING, and the registered replication of H70's post hoc finding, at the rate E3 measured rather than the rate H70 guessed. E3 found the sustainable offered rate is 10,000,000 messages per second at 2,000,000 messages per epoch and 7,000,000 at 10,000,000 messages per epoch, so H71 at 10,000,000 was 43 percent over capacity for its own run length, which is why all 1,080 of its cells saturated. This runs at 7,000,000 offered with 10,000,000 messages per epoch: the highest rate at which every arm was clean in all 9 E3 cells. Same four arms, same 3 epochs, same 30 launches, same three waits, same 65-quantile CCDF. (a) p99.9 at branching 0.8 is at or above 1.50 times p99.9 at branching 0.0, 21 of 30. (b) p50 at branching 0.8 is within 1.25 times p50 at branching 0.0, so any tail effect is not a throughput artefact. (c) THE REPLICATION, and the reason this run matters most, hawkes_0.0 against the periodic arm paired within launch: the ratio reaches at or above 2.00 somewhere between the 95th and the 99.7th percentile, with at or above 27 of 30 on the sign test at that peak, AND falls below 1.15 at p99.9. All three parts must hold. H70 measured 2.122 at p95.73, 2.604 at p99.64 and 1.052 at p99.9 at 5,000,000, found after the fact; this states it as a number in advance at 7,000,000. H71 could not test it because every arm saturated. (d) every quantile compared carries at least 100 observations beyond it. (e) at most 2 percent of the 1,080 cells report saturated=1. E3 measured 0 of 9 for every arm at this rate and run length. (f) FAILURE BRANCH, NAMED IN ADVANCE AND TIGHTER THAN H71's: if more than 2 percent saturate, the rate is NOT lowered again. H72 is recorded as failing its own operating-point gate, and the envelope is remeasured with a finer ladder between 6,000,000 and 8,000,000. Two rate guesses have now failed and a third would be tuning until something passes | CONFOUND RECORDED 2026-11-20, BEFORE ANY READER FOUND IT: this run is the only measurement in the project taken on macOS 27.2.1 build 26B131. Everything it is compared against, H70 included, was taken on 27.2 build 26B123, which the machine left between 12:49 and 17:12 IST on 19 November while unattended. The registration claims a single variable changed from H70. Two did. RESOLVED 2026-11-20 BY H75: H70's configuration was re-run on 26B131 and reproduced to within 0.4 to 2.4 percent on the peak and 0.7 percent on the median, so the update did not move the measurement and the cross-rate comparison varies one thing after all. The within-run verdicts (a) through (e) are unaffected, because every arm in this file ran on the same build. | (c) SUPPORTED ON ALL THREE PARTS AT ALL THREE WAIT STRATEGIES, and it is the most important verdict in this file. Registered before the run: peak ratio at or above 2.00 somewhere between p95 and p99.7, at or above 27 of 30 on the sign test at that peak, AND below 1.15 at p99.9. Measured: peak 3.4436 at p98.1197 spin, 3.1874 at p98.4683 isb, 3.2159 at p98.4683 wfe, every one at 30 of 30 with p = 1.86e-09; and at p99.9 the ratio is 1.0689, 1.0371 and 1.0805, all below 1.15. H70 found this effect AFTER the fact; H72 predicted it with numbers, at a rate chosen by measurement, and it came in LARGER than predicted. (a) REFUTED on magnitude, AND CLUSTERING HAS APPEARED FOR THE FIRST TIME IN THIS PROJECT. Registered: p99.9 ratio of branching 0.8 to 0.0 at or above 1.50 with 21 of 30. Measured 1.1682 spin, 1.1260 isb, 1.1197 wfe, so the threshold is missed by a wide margin and (a) is refuted. But the sign test is 26 of 30 at p = 5.95e-05 at ALL THREE waits, against H70's 16 of 30 at p = 0.856, 13 of 30 at p = 1 and 20 of 30 at p = 0.0987. At 5,000,000 clustering was statistically indistinguishable from nothing. At 7,000,000 it is statistically certain and small, 12 to 17 percent. The registered threshold of 1.50 was wrong, not the direction. (b) SUPPORTED: p50 ratio 1.0108 isb, 1.0216 spin, 1.0175 wfe against a registered 1.25, so neither (a)'s small effect nor (c)'s large one is a throughput artefact. (d) SUPPORTED AS SCOPED, WITH A WORDING FAULT IN MY REGISTRATION THAT I AM RECORDING RATHER THAN QUIETLY READING PAST. Registered: every quantile compared carries at least 100 observations beyond it. Every quantile used in a registered comparison does, by two orders of magnitude or more: 5,000,000 beyond p50, 29,704 to 426,986 across the p95 to p99.7 window, 10,656 beyond p99.9. The emitted ladder however continues past p99.99 to points carrying as few as 10 observations, because the instrument's gate is 10 and not 100. Those deeper points are reported and are not compared in (a), (b) or (c). The registration should have said every quantile USED IN A REGISTERED COMPARISON; read literally it fails on display-only points. (e) SUPPORTED exactly: 0 of 1,080 cells saturated against a registered ceiling of 2 percent, and the pre-flight showed 0 of 4 arms saturated at the identical cell beforehand | THE REPLICATION HOLDS AND THE EFFECT SCALES WITH UTILISATION. Matched 65-quantile analysis of both runs, random against perfectly periodic at the same mean rate: peak ratio rises from 2.7765 / 2.7505 / 2.5430 at 5,000,000 to 3.4436 / 3.1874 / 3.2159 at 7,000,000 for spin / isb / wfe, the peak MOVES EARLIER from p99.5524, p99.6354 and p99.5524 to p98.1197, p98.4683 and p98.4683, and the median ratio rises from 1.29, 1.25 and 1.24 to 1.69, 1.60 and 1.58. As utilisation goes from roughly half of the measured ceiling to all of it, the randomness penalty grows and migrates out of the tail and into the body. Both runs agree that it is gone by p99.9 (1.0534 to 1.0541 at 5,000,000; 1.0371 to 1.0805 at 7,000,000) and gone in the far tail. This is consistent with a coincidence-queue account, which remains untested: a periodic schedule never presents two messages at once and a Poisson schedule does so constantly, so a rising fraction of messages meets a queue as utilisation rises. Also recorded: all 30 launches took 55 or 56 seconds with no anomaly, against 4 anomalous launches of 30 in H71, which ran 43 percent over capacity. That is suggestive that H71's long launches were queueing rather than thermal, and it is not tested here | results/2026-11-19_m4_burst7.txt |
| E3 | EXPLORATORY CALIBRATION, NOT A HYPOTHESIS TEST, and numbered E so it cannot be mistaken for one. H71 refuted its own primary because every arm was over capacity, and its pre-flight suggested the sustainable rate falls as the epoch lengthens. This measures the operating envelope directly instead of inferring it: offered rates 6,000,000 through 10,000,000 in steps of 1,000,000, crossed with two run lengths, 2,000,000 and 10,000,000 messages per epoch, over all four arms (periodic, hawkes 0.0, 0.5, 0.8) and all three wait strategies, 1 epoch, 3 launches. Reported by `scripts/capacity_envelope.py`, which reads the `pl` lines a saturated run still emits and prints, per run length, the highest offered rate at which EVERY arm stays unsaturated. There is no threshold to pass because nothing is being tested: the output is the number a later registration needs in order to name a rate honestly | the envelope, and whether it moves with run length | see the file |
| E4 | EXPLORATORY CALIBRATION, NOT A HYPOTHESIS TEST. Interarrival variability of every replay schedule this project uses, reported as BOTH the squared coefficient of variation of the interarrival times and the Fano factor of the event counts, computed on the identical schedule so the two can be compared directly. It exists because the project characterised burstiness by the Fano factor alone and drew a conclusion about Kingman's heavy-traffic approximation from it, which was wrong: the approximation's variability term consumes the coefficient of variation, not the Fano factor. See docs/defects.md D8. Three parts: the arms at the rate and sample length the manuscript uses; the same arms at a different sample length, to show that the Fano factor moves with the window and the coefficient of variation does not; and a sweep of the kernel decay rate at branching 0.8 from 1e3 to 2.5e7, which moves the excitation timescale from 1 ms down to 40 ns against a mean gap of 200 ns. No thresholds, because nothing is being tested | the two statistics, and whether they dissociate | results/2026-11-20_arrivals_cv.txt |
| H75 | REPLICATION OF H70 ON THE CURRENT OPERATING SYSTEM BUILD, registered because the headline cross-rate comparison is confounded and must be either cleaned or withdrawn. See docs/defects.md D9. H70 ran at 5,000,000 messages per second on macOS 27.2 build 26B123. H72 ran at 7,000,000 on 27.2.1 build 26B131. The project's strongest claim compares them, so it varies two things. This re-runs H70's configuration EXACTLY, same rate, same 10,000,000 messages per epoch, same 3 epochs, same 30 launches, same three waits, same four arms, same 65-quantile CCDF, on build 26B131. One variable differs from H70: the operating system. (a) PEAK RATIO REPRODUCES: for each wait strategy the peak of the random-over-periodic curve is within 15 percent of H70's, which measured 2.7765 spin, 2.7505 isb, 2.5430 wfe. (b) PEAK LOCATION REPRODUCES: the peak falls within half a decade of survival of H70's, which was p99.5524 spin, p99.6354 isb, p99.5524 wfe. (c) THE COLLAPSE REPRODUCES: the ratio at p99.9 is below 1.15, as H70's 1.0534 to 1.0541 was. (d) THE BODY REPRODUCES: the median ratio is within 15 percent of H70's 1.29 spin, 1.25 isb, 1.24 wfe. (e) NO ARM SATURATES: at most 2 percent of the 1,080 cells report saturated=1, as H70 recorded none. IF ALL FIVE HOLD, the operating system update did not move this measurement, the H70 against H72 comparison varies one thing again, and the paper may additionally state that the effect survives an OS update. IF ANY FAILS, the update moved the measurement and every cross-rate claim is withdrawn from the manuscript rather than defended. There is no third outcome in which the claim is kept with a caveat | ALL FIVE PARTS SUPPORTED, and the agreement is closer than the thresholds required. 30 fresh launches at 5,000,000 on build 26B131, two days after H70 ran the identical configuration on 26B123. (a) PEAK RATIO: 2.7873 spin against H70's 2.7765, a 0.4 percent difference; 2.6835 isb against 2.7505, 2.4 percent; 2.5706 wfe against 2.5430, 1.1 percent. Registered tolerance was 15 percent. Sign tests 29/30, 29/30 and 30/30, p below 6e-08. (b) PEAK LOCATION: p99.5524 spin and p99.5524 wfe, identical to H70 to the quantile; isb lands at p99.5524 against H70's p99.6354, which is 0.089 decades of survival against a registered half decade. (c) COLLAPSE AT p99.9: 1.0476, 1.0221 and 1.0291 against a registered 1.15, with H70 at 1.0541, 1.0538 and 1.0534. (d) BODY AT p50: 1.2985, 1.2513 and 1.2450 against H70's 1.2902, 1.2544 and 1.2362, differences of 0.7, 0.1 and 0.4 percent against a registered 15. (e) SATURATION: 0 of 1,080 cells, as H70 recorded. | THE OPERATING SYSTEM UPDATE DID NOT MOVE THE MEASUREMENT, so the cross-rate comparison between H70 and H72 varies one thing again and D9 is closed. The paper gains a claim it did not have: the randomness effect reproduces at 30 independent launches, two days later, across a macOS update, with the peak within 0.4 to 2.4 percent and the median within 0.7 percent. That is a reproduction across time and platform revision, not a re-analysis of the same bytes | results/2026-11-20_m4_burst5_rebuild.txt |
| H76 | REPLICATION OF THE H69 BARRIER CROSS WITH THE RAW RECORDS RETAINED. Registered because the paper's first three contributions rest on a results file that holds one record in 269 lines, so no median, interval or sign test behind them can be recomputed by anyone. See docs/defects.md D4. The binary always emitted a full record per arm per epoch; the delivery script aggregated them and discarded the raw. This re-runs the same sweep, 30 launches of 3 epochs over 10 arms, and keeps every record. It also lands on build 26B131 rather than 26B123, so it is a replication across an operating system update as well as an archival fix. (a) THE CONTENDED INVERSION REPRODUCES: the ratio of acqrel to fencedstore cycles per message when the cursor line is contended is within 15 percent of the published 2.57. (b) THE CACHED INVERSION REPRODUCES AND KEEPS ITS SIGN: the same substitution is slower when L1-resident, within 15 percent of the published 2.75. (c) THE RANKING REVERSES, as published: the contended order is fencedstore, fenced, fencedload, acqrel and the resident order is fencedload, acqrel, fenced, fencedstore. (d) CORRECTNESS IS UNCHANGED: every correct ring reports sum_ok=1 in every epoch, and the unordered arm reports sum_ok=0 in every epoch, as recorded before. (e) THE RAW IS COMPLETE: the committed file holds 30 launches times 10 arms times 3 epochs equals 900 records, each carrying ns_per_msg, epoch_ms and sum_ok, so every published statistic is recomputable from it. (f) FAILURE BRANCH: any part that misses its tolerance is reported as a disagreement between builds and is NOT reconciled by adjusting the published figure. The original summary stays in place and both numbers appear in the manuscript | ALL FIVE PARTS SUPPORTED. 900 records retained, which is the point of the run. (a) CONTENDED INVERSION: acqrel over fencedstore is 49.861/19.630 = 2.5401 against the published 2.57, a 1.2 percent difference against a registered 15. (b) RESIDENT INVERSION: fencedstore over acqrel is 5.864/2.332 = 2.5146 against the published 2.75, 8.6 percent, and the sign holds: the substitution that is 2.54 times faster contended is 2.51 times slower once the line is in L1. (c) RANKING: contended fencedstore < fenced < fencedload < acqrel, resident fencedload < acqrel < fenced < fencedstore. Both orders identical to the published ones, all four arms, both configurations. (d) CORRECTNESS: acqrel, fenced, fencedload and fencedstore each report sum_ok=1 in 180 of 180; unordered reports sum_ok=1 in 0 of 180. Identical to the original. (e) RECORDS: 900, being 30 launches times 10 arms times 3 epochs, each carrying ns_per_msg, epoch_ms, sum_ok and the switch counts. | D4 IS CLOSED: every median, interval and sign test behind the paper's first three contributions is now recomputable from committed data by anyone. LIMITATION STATED RATHER THAN HIDDEN: the original ran with the PMU and reported cycles; this ran without it and reports nanoseconds, because the PMU needs root and an unattended run must not contain a password prompt. Ratios are compared, which is valid where the arms run at similar frequency. The original verified that across all 540 of its lines at 4.035 to 4.414 GHz; this run could not verify it. A cycle-level replication needs an interactive run with sudo and is not yet done | results/2026-11-20_m4_barrier_raw.txt |
| E1 | EXPLORATORY, NOT PRE-REGISTERED, and numbered E rather than H so it cannot be mistaken for a tested hypothesis. Every registered run in this project is on one machine, which cannot distinguish a property of measurement from a property of this laptop, and that is the largest threat to the work's validity. The same titan-stream binary at the same commit was run on a virtualized x86-64 Linux host, 10 launches at 200,000 and 10 at 10,000,000 messages, 3 epochs, same analysis path, nanoseconds only because the host has no accessible PMU | two findings, the second larger than the question | FINDING 1, the run-length sensitivity does not appear off Apple silicon: SeqCst/AcqRel uncached goes 18.053 to 18.234 (1.01x) and Fenced/AcqRel uncached 11.387 to 11.714 (1.03x) between the two run lengths, against 4.326 to 1.006 (4.3x) and 3.934 to 0.447 (8.8x WITH A SIGN FLIP) on the M4. The two comparisons that reverse on the M4 move by 1 and 3 percent here. FINDING 2, not the question and larger than the answer: the cost of memory ordering differs by an order of magnitude between the architectures. On x86-64 SeqCst costs 18.2x AcqRel uncached and Fenced costs 11.7x; on the M4 at the same run length they cost 1.0x and 0.45x. The architectural explanation is standard and consistent with this project's own instruction census: x86-64 is total-store-order so acquire and release are plain moves and a seqcst store needs a fence, while AArch64 carries the ordering in ldapr and stlr so ldar costs little more. The architecture that makes weak ordering free makes strong ordering expensive. FOUR LIMITS, the first a real alternative explanation this run cannot exclude: the host is VIRTUALIZED, so the absence of the effect is equally consistent with the hypervisor masking frequency scaling, and only a bare-metal x86-64 or a fanned Apple machine separates those; 2 cores against 4+6, so only ratios within one host are compared and never an absolute from one against an absolute from the other; rustc 1.97.0 against 1.92.0; and no PMU, so wall time only. WHY IT MATTERS: the project's framing claim, that protocol decides microarchitectural conclusions, is Mytkowicz, Diwan, Hauswirth and Sweeney, ASPLOS 2009, which showed measurement bias from UNIX environment size and link order. This run supplies the difference: duration-driven frequency scaling and heterogeneous core residency did not exist on a general-purpose CPU in 2009, and their absence on an older architecture is evidence they belong to the new machine class rather than to measurement in general. | results/2026-11-16_x86_crossarch.txt |
| H74 | Which statistic predicts the tail cost of clustered arrivals: the squared coefficient of variation of interarrival times, which Kingman's heavy-traffic approximation consumes, or the Fano factor of event counts, which this project used in its place and drew a wrong conclusion from. The branching ratio is held at 0.8 and the exponential decay rate beta is swept, which separates the two. Measured on the schedules themselves at 10,000,000 messages, offered rate 5,000,000, seed 7: at beta 1e5, 1e6, 3e6 and 2.5e7 the cv squared is 1.1005, 2.3620, 4.7086 and 8.1921 while the Fano factor is 24.79, 25.12, 25.33 and 25.78. So cv squared varies by a factor of 7.4 while Fano varies by under 6 percent, and the two make opposite predictions for the first time in this project: cv squared predicts a rising tail penalty across the ladder, Fano predicts none. The schedule the pipeline replays uses a different seed from this calibration, so the realisation differs while the process parameters do not. At 10,000,000 messages, 3 epochs, 30 launches, arms periodic plus Poisson plus beta in {1e5, 1e6, 3e6, 2.5e7} at branching 0.8, waits spin, isb and wfe, every ratio formed within a launch over the 65-quantile CCDF and taken at the quantile where it is largest: (a) the ratio of beta 2.5e7 to beta 1e5 on spin exceeds 1.0 in at least 21 of 30 launches, which is the p < 0.05 threshold of the exact two-sided sign test and is the discriminating test, since the Fano account predicts 15 of 30; (b) the median of that ratio is at or above 1.25, reported whether or not it holds, because the direction and the magnitude are separate questions and only the direction distinguishes the two theories; (c) the median ratio against beta 1e5 is monotone non-decreasing across 1e6, 3e6 and 2.5e7 on spin; (d) (a) holds on isb and on wfe; (e) the achieved rate is within 1 percent of the offered rate at every beta and no cell reports saturation, so the comparison is not confounded by rate or by saturation; (f) control, the ratio of beta 1e6 to beta 1e5, a cv squared gap of 2.1, is smaller than the ratio of beta 2.5e7 to beta 1e5, a cv squared gap of 7.4, so the effect scales with the gap rather than being an artifact of comparing any two Hawkes arms. If (a) fails, cv squared is refuted as the predictor at this scale and the paper reports that neither statistic predicts the penalty, as a negative result. | (a) SUPPORTED, (b) SUPPORTED, (c) SUPPORTED, (d) SUPPORTED, (e) REFUTED, (f) SUPPORTED. Five of six. | results/2026-11-20_m4_h74_beta.txt, 1620 pl and 105170 CCDF records over 30 launches, 56m 13s on wall power, build 26B131. (a) the peak contrast of beta 2.5e7 against beta 1e5 on spin runs the same way in 30 of 30 launches, p=1.86e-09, against a threshold of 21 and against the 15 of 30 the Fano account predicts. (b) the median peak ratio is 13.5030 against a registered 1.25, and 841 ns becomes 11356 ns at that quantile. (c) the ladder is monotone, 8.4922 then 11.3744 then 13.5030 as cv squared goes 2.3620, 4.7086, 8.1921. (d) 30 of 30 on isb and on wfe as well, peak ratios 14.3415 and 14.4450. (e) refuted: the threshold was no saturated cell and two of 1620 saturated, worst achieved-against-offered gap 10.849 percent against a registered 1 percent. Both are transients: 1618 of 1620 cells show a gap of 0.000 percent, the two outliers are exactly the two saturated cells, and the contaminated one sits in the low-beta arm where a slowdown shrinks the ratio. Dropping both launches leaves the headline at 13.5030, 28 of 28, p=7.45e-09. (f) the smaller cv squared gap gives the smaller ratio, 8.4922 against 13.5030. Scale: the periodic and Poisson references against beta 1e5 give 0.9188 and 0.9742 at 8 and 9 of 30, so the reference arms show no effect. Quantitatively, Kingman's variability term implies 7.444 times at these cv squared values and the Fano account implies 1.040 times; the measured median ratio is 6.232, which is 0.837 of the first and 5.99 times the second. Verdicts computed by scripts/h74_verdicts.py after the data was committed. |
| H77 | The cycle-level replication of the barrier cross, which is the half of D4 that H76 could not close. H76 re-ran the cross with every raw record retained but without the PMU, because the counters need root and an unattended script must not carry a password prompt. It therefore reported nanoseconds and compared ratios, which holds only if the arms run at similar frequency. The original run asserted 4.035 to 4.414 GHz across its 540 lines; H76 could not check that, and the assumption has stood unverified underneath the paper's first three contributions ever since. This run reads the counters directly: `sudo titan-barrier 10000000 3 pmu`, 30 launches, 10 arms, 3 epochs, 900 records carrying msg_cycles, msg_instructions, msg_l1d_miss, msg_br_mispred and core_ghz per record. Registered: (a) contended, that is cached=0, the median msg_cycles of acqrel divided by that of fencedstore is at or above 2.0, and the per-launch ratio exceeds 1.0 in at least 21 of 30 launches; (b) resident, cached=1, the median msg_cycles of fencedstore divided by that of acqrel is at or above 2.0 with the per-launch sign test also at 21 of 30, so the comparison changes sign in cycles and not only in nanoseconds; (c) the ranking of the five orders by median msg_cycles is identical, arm for arm, to the ranking by median ns_per_msg in results/2026-11-20_m4_barrier_raw.txt, at both caching states; (d) the discriminating threshold, the spread of core_ghz across the five orders within each caching state, highest over lowest, is at or below 1.15, which is what makes a nanosecond ratio a fair comparison and is the specific thing H76 had to assume; (e) sum_ok is 1 in 180 of 180 records for each of acqrel, fenced, fencedload and fencedstore, and 0 of 180 for unordered. If (d) fails, the nanosecond ratios in the first three contributions are confounded by frequency and the paper must report cycle ratios instead, which this run would then supply. | registered, not yet run | none. results/2026-11-20_m4_barrier_raw.txt supplies the nanosecond rankings that (c) is checked against and contains no cycle counts. |

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
* Results of 2026-11-11 (commit 0d8b78d (tree b664c19; was c3b850d before the author-name rewrite of 2026-11-17)): `results/2026-11-11_m4.txt`. Code
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

## Pre-registered 2026-11-14, ninth run

**H49 (one exposure limit shared by several strategies).** The aggregate
risk item of the first source brief (Day 13: combined exposure across
strategies). Checked on 2026-11-14 against published work, the cost of
contended atomics between CPU cores has not been reported for Apple
silicon in these sources: Khayet and Morrison (ASPLOS 2026) study the
Apple M1 and M4 and leave the evaluation of TSO mode's impact on the
performance of atomic read-modify-write and memory barrier operations to
future work; Beischl et al. (DaMoN 2026, M4 Pro and M3 Ultra) measure CPU
to GPU coherence, not contended atomics between CPU cores; Schweizer,
Besta and Hoefler (PACT 2015, pp. 445-456) measure atomics on x86 only; a
2026 Lund University master's thesis (Bratthall, carried out at Nasdaq)
measures single-producer multi-reader queues, some built on `fetch_add`,
on Intel Sapphire Rapids and NVIDIA Grace, without Apple silicon and
without isolating the cost of contended atomics.
`titan-agg`, new in this batch: S strategy threads (S = 1, 2, 3; the main
thread is strategy 0; QoS USER_INTERACTIVE) each check the next order's
notional against the exposure limit, reserve it, keep it open for 16
orders and release it, so every order does one reservation and one
release. Designs: `local` (each strategy checks only its own total: the
floor); `faa` (one shared total, check with a load, reserve with
`fetch_add`: two steps, so concurrent reservations can pass the limit
together); `cas` (one shared total, check and reserve in one
`compare_exchange_weak` loop: the limit is exact); `shard` (one total per
strategy in its own 128-byte line; the check sums all S totals, the
strategy updates its own with a load and a store; the sum of S separate
loads is not a snapshot, so, as with `faa`, concurrent reservations can
pass the limit together). Releases use `fetch_sub` (faa, cas) or the
strategy's own total. All accesses `Relaxed`. Each strategy's open orders
sit in their own 128-byte lines. Notionals come from a fixed
pseudo-random sequence (the cost of these instructions does not depend on
the values) and the limit never binds, so every order is accepted in
every design; the harness aborts if any order is rejected or if any total
does not return to zero after every strategy releases its open orders
(mutation tests on the development machine: a wrong release amount and a
binding limit both abort). Strategy 0 runs 5,000,000 orders, measured with
its own PMU cycles; strategies 1 to S-1 run the same loop from before
strategy 0 starts until it is done. They can still be descheduled or run
on an efficiency core (macOS offers no thread pinning), so each one's
order count is reported, and an epoch in which any of them completed
fewer than 50,000 orders (ops/100) is marked `starved=1` and excluded from
the verdicts; the number excluded is reported. `invol_switches` counts
the whole process. Retries: `cas_retries_s0` is strategy 0's failed
compare-and-swap attempts per measured order; `cas_retries_per_order` is
all strategies' failed attempts over the whole run divided by all their
orders. Configs per epoch, order rotated: local1, faa1, cas1, shard1,
local2, faa2, cas2, shard2, local3, faa3, cas3, shard3; 10 epochs in one
process (preliminary, as for H42 to H48). Decision rule for each part:
two-sided Mann-Whitney U on the per-epoch `msg_cycles` of strategy 0
(10 against 10, fewer if epochs are excluded), p < 0.05 with the medians
in the stated direction; each part is supported or refuted on its own.
Not evaluated without the PMU. Reported, not tested: shard3 against
shard1 (what the other strategies' writes cost the check), `shard` and
`local` differ in their instruction count as well as their sharing.
`scripts/fanout_census.sh` also disassembles the eight `titan-agg` loops
and counts LSE atomics (`ldadd`, `stadd`, `swp` and their ordering
variants), `cas`, exclusive pairs (`ldxr`, `stxr`) and barriers per loop,
so the instructions behind each design are reported, not assumed.
Since the eighth-run text, each measured loop of both binaries is one
named function per design (`write_pass_snap`, `measured_faa` and so on,
each inlining the same generic loop as before), so the census labels name
the design under either symbol-mangling scheme; the loop source is
unchanged (the generated code is not guaranteed byte-identical to the
earlier build, which was never measured).

## Pre-registered 2026-11-14, tenth run

**H50 (inbound wire format).** The market-data normalisation item of the
second source brief (Part 2: a unified layer normalising order book data
from exchanges with different formats; lines 9, 319 and 379 of that brief).
`titan-wire`, new in this batch. Updates: the feed is replayed through the
reference book and every change of a price level's total quantity becomes
one update (an add, execution, cancel or delete touches one level, a replace
up to two); the first 2,000,000 updates of each feed are used (BX 2019-07-30
from its first message; the synthetic feed of 5,000,000 messages, 500
instruments). Each update is encoded three ways (`titan_itch::wire`): a
fixed 32-byte big-endian binary record; a FIX 4.4 MarketDataIncrementalRefresh
(35=X) with the required standard header (8, 9, 35, 49 SenderCompID,
56 TargetCompID, 34 MsgSeqNum, 52 SendingTime to the millisecond) and one
entry (268=1, then 279, 269, 55, 270, 271, size omitted on delete), with
BodyLength and CheckSum; and a JSON event in the shape of Binance's spot diff
depth stream (field set, string prices and quantities, quantity 0 removing
the level, as in Binance's spot WebSocket streams documentation; one level
per event, compact output). Decoders: `bin`; `fix` (BeginString, BodyLength,
CheckSum, 35 as the third field, every field at most once and every
required one present, header before body, SendingTime in millisecond form
with seconds 00 to 59, any other tag rejected; symbol to locate through an open-addressed table); `json_fixed` (a
scanner specialised to the key order of the documented example and compact
output, rejecting anything else, so a reordered stream fails safe; that page
shows the order in an example and does not promise it); `json_serde`
(`serde_json` into a typed struct that borrows its strings).
Before timing, the harness aborts unless every decoder returns exactly the
original record for every update. Floors: `floor_bin`, `floor_fix` and
`floor_json` fold the first and last byte and the length of every message of
that format. X(stage) = median `msg_cycles` of the stage minus the median of
the floor of its format. Decision rules: (a) and (b) are ratios of these X
values (undefined if the bootstrap 95% interval of the denominator reaches
zero); (c) compares total decoder cycles, because the two formats have
different floors: a two-sided Mann-Whitney U on the per-epoch `msg_cycles`
of `json_fixed` and `fix`, p < 0.05 with the medians in the stated direction;
both feeds supported, one partly. 10 epochs in one process per feed, stage order
rotated (preliminary). Reported, not tested: allocations per update (one
`Vec` per event is expected from `serde_json`, none from the others),
bytes per update. The formats do not carry identical content: JSON also
carries an event time and two update ids, FIX a SendingTime and two
CompIDs, and the binary decoder skips its time field. Disclosure: the thresholds
of (a) and (b) and the direction of (c) were set after development-machine
runs (x86, a virtual machine, nanoseconds, synthetic feed only, not
evidence) gave, over several runs, 26x to 32x for (a), 3.8x to 4x for (b)
and about 0.5x for (c) with the shorter FIX header used then, and 28x to
49x, 3.4x to 3.9x and 0.3x to 0.4x with the full header; the ratio (a)
varies most because X(bin) there was only about 5 ns. They are
therefore not discoveries: they test whether the order of magnitude seen
there carries over to the M4 and to the real BX feed.

## Pre-registered 2026-11-14, eleventh run

**H51 (A/B arbitration and gap recovery).** The feed arbitration and gap
recovery items of the second source brief (Part 3: consuming redundant feed
lines and recovering losses). `titan-arb`, new in this batch, over
`titan_itch::arb` and `titan_itch::mold`. Setup, untimed: the first
2,000,000 ITCH messages of each feed (BX 2019-07-30; the synthetic feed) are
framed into MoldUDP64 downstream packets of at most 1472 bytes of UDP
payload (a 1500-byte Ethernet frame less the IPv4 and UDP headers), the
NASDAQ MoldUDP64 Protocol Specification v1.00 layout (session 10 bytes,
sequence number 8, message count 2, blocks of a 2-byte length and the
message; count 0 a heartbeat, 0xFFFF end of session; the Request Packet is
session, first requested sequence number, requested message count; the
answer is a standard downstream packet holding only the requested messages
that completely fit, the rest to be requested again). Line B is a byte copy
of line A at a different address, arriving a fixed 4 packets behind A, with
independent per-packet loss by a seeded xorshift64* generator. A dry pass
records every Request Packet the arbiter sends and builds each answer
beforehand, so the timed pass allocates nothing (the arbiter's containers
are created before the timed region; every line reports
`allocations_pass`, which must be 0 outside the PMU read). The answer to a request is injected
64 events after it (event time, not waiting), so the conditions measure
processing cost per message, not recovery latency. Timed, per condition, one
pass over the whole event schedule: `floor` walks every packet of line A
and folds its first byte, last byte and length; `one0` one lossless line;
`two0` two lossless lines; `two_a` loss of 10,000 ppm on A only, so every
gap is filled by line B; `two_ab` loss of 10,000 ppm on each line
independently, so some messages are lost on both and come from the
re-request server (requests must be above 0, asserted). Every condition
must deliver every message exactly once and in order: the delivered fold
checksum must equal the store's, asserted in every pass. X(condition) =
median `msg_cycles` per delivered message minus the floor's median.
Decision rules: (a) X(two0) / X(one0) at most 1.5, (b) X(two_ab) / X(two0)
at most 2.0, ratios with bootstrap 95% intervals, undefined if the
denominator's interval reaches zero; (c) is the assertions holding in every
epoch of both feeds (each pass checks an order-sensitive fold of every
delivered message against the store's; byte-exact delivery under loss is
checked in the unit tests). Both feeds supported, one partly. 10 epochs in one
process per feed, condition order rotated, stats (duplicates, held,
max held, requests, resent, partial) printed per condition from the dry
pass. Disclosure: unlike H50, these thresholds are set before the program
has been run anywhere, on reasoning alone: a duplicate packet costs one
header parse and a comparison against the delivered messages it carries
(about forty-five: 2,000,000 messages over about 45,000 packets), and
recovery work is confined to the vicinity of gaps; written 2026-11-14
before any timing, development machine included.

Amendment, 2026-11-14, before any timed run anywhere: the loss is raised
from 10,000 to 30,000 ppm per line. A correctness-only development run at
200,000 messages (a development size, not the registered one) aborted on
the two_ab assertion (requests above 0): over its 4,477 packets at 10,000
ppm per line the expected number of packets lost on both lines is about
0.45, so an abort there was likely. At the registered 2,000,000 messages
(about 45,000 packets) the expected number is about 4.5; an untimed audit
run at that size passed with 5 such packets. Five recovery events over
2,000,000 messages would leave two_ab nearly indistinguishable from two_a,
and a run with none would still happen about once in a hundred. At 30,000
ppm about 40 to 50 packets are lost on both lines (the audit run counted
50 requests). The thresholds and every other rule are unchanged; no timing
was seen before this amendment (the aborted and audit runs printed counts,
not times).

## Pre-registered 2026-11-14, twelfth run

**H52 (best-effort FIX decode).** Why this exists: H50 compared a fixed
binary record against a FIX decoder that finds delimiters and sums the
checksum one byte at a time, while this repository already showed that a
NEON delimiter search cuts a validated FIX 4.2 NewOrderSingle parse from 465
to 301 cycles (0.647; results/2026-11-12_m4_ac2.txt). The binary decoder has
no such loop to accelerate, so H50's ratio is partly a statement about an
unoptimised text parser, and the honest headline needs the best effort on
both sides. `fix_simd` is added for that: the same validation code as `fix`
(one const-generic function; only the delimiter search and the checksum
differ), the NEON scan of `soh_neon`, and a checksum accumulated in `u8`
lanes, where the lane adds and the horizontal add are already modulo 256.
`both_fix_decoders_agree` requires the two decoders to return the same value
on a valid message, on every single-bit change of it, at every truncated
length, with 0 to 39 extra fields and on filler of every length to one step;
on a machine without NEON the scan falls back to the scalar one, so that
test only compares the two implementations on the measurement machine. The
harness also asserts that `fix_simd` returns the original record for all
2,000,000 updates before timing. Same feeds, floors, epochs, rotation and X
definition as H50 (`fix_simd` is measured above `floor_fix`). Decision
rules: (a) two-sided Mann-Whitney U on the per-epoch `msg_cycles` of
`fix_simd` and `fix`, p < 0.05 with the medians in the stated direction;
(b) and (c) ratios of the X values with bootstrap 95% intervals, read on the
ratio of the medians; both feeds supported, one partly. Disclosure: (b)'s
interval is centred on this repository's own earlier 0.647 for a different
FIX message on the same machine, and (c) is set knowing H50 measured 44.3
and 55.5 for the scalar decoder; (b) can fail in either direction and is the
part of this hypothesis that carries information. No NEON timing for this
decoder exists anywhere: on the development machine (x86) `soh_neon` falls
back to the scalar scan, so the stage there is the same code as `fix` and
tells nothing about the M4. Symmetry, so that the correction does not
simply reverse the bias: the binary decoder reads fixed offsets and has no
whole-message loop to vectorise; the specialised JSON scanner already walks
the documented key order once and computes no checksum; `serde_json` is a
library taken as it ships, and replacing it is a different question from
format cost. FIX is the one format whose rules impose two whole-message
passes, a delimiter index and a checksum over every byte, which is why it is
the one with a loop to accelerate. That those passes are inherent to the
protocol, not to our implementation, is itself part of what H50 and H52
measure.

## Pre-registered 2026-11-14, thirteenth run

**H53 (consolidated book across two exchanges).** The market-data
normalisation and cross-venue items of the second source brief (one book
built from venues that number their instruments independently, and the
arbitrage the brief looks for, which is exactly a crossed consolidated
book). `titan-cross` and `titan_itch::combine`, new in this batch. Data: the
public NASDAQ BX and PSX TotalView-ITCH 5.0 samples of 2019-07-30, the same
day on both venues. The two Stock Directories are intersected by symbol and
each shared symbol gets one identifier; the venues' stock locates differ, so
a view that matched on locate would be wrong, and `make_feeds` in the
end-to-end check gives the same symbol locate 1 on one venue and locate 7 on
the other. Both feeds are replayed into one stream ordered by the 6-byte
timestamp each ITCH message carries, each into its own L3 book. Designs:
`floor`, the two venue books alone; `incr`, which after each message
re-reads that instrument's top on that venue and refolds its consolidated
top; `rebuild`, which keeps no per-instrument state and re-reads every
venue's top for every shared instrument every 1,000 messages. X(design) =
median `msg_cycles` of the design minus the median of `floor`. Timed: 10
epochs of the first 4,000,000 merged messages, design order rotated, PMU on.
Untimed and once, over as much of both days as memory allows: the same
replay with the incremental design, accumulating locked and crossed
episodes and their durations inside 09:30:00 to 16:00:00, and what the
periodic design would have missed. After every incremental pass the harness
rebuilds the consolidated view from the books and requires it to equal the
view kept update by update. Decision rules: (a) and (b) ratios with
bootstrap 95% intervals, read on the ratio of the medians, undefined if the
denominator's interval reaches zero ((a) is X(incr) against the median of
`floor`, since the consolidation is being compared with the book work it is
added to); (c) counts above zero inside the window; (d) the share of crossed
episodes whose first and last message fall between the same two rebuilds, at
least 0.5. Reported, not tested: episode durations and their maxima, the
mean amount a crossed market was crossed by, how many instruments crossed at
all, how far behind the periodic design fell, and allocations per pass
(these are not zero: the L3 books grow as instruments first appear).

Limitation, stated before the run: a crossed consolidated book here means
the two venues' displayed books disagreed as each venue timestamped them. It
is not a claim that one participant could have seen both sides at that
instant, because the replay uses each venue's own clock and models no
propagation delay between them, and it is not the official NBBO: these are
the venues' own displayed depth feeds, without odd lots, hidden liquidity or
the SIP's processing. Whether such an episode was tradable is a different
question from whether it existed in the data, and only the second is
measured here. Thresholds: (a) and (b) rest on counting the work (two
constant-time top reads per message against a full book update, against
four reads for every shared instrument every 1,000 messages, which is about
thirty reads per message if the venues share some eight thousand symbols);
(d) rests on crossed episodes between venues being short, which this
repository has not measured. No part of this has been run on the real BX and
PSX data, on any machine: the only run so far is the end-to-end check on two
hand-built feeds of fourteen messages whose expected episode durations were
computed by hand first.

## Pre-registered 2026-11-14, fourteenth run

**H54 (round lots, and the cost over a whole day).** Why this exists: the
thirteenth run found 33 crossed and 360 locked episodes between BX and PSX
inside continuous trading, but they last a mean of 18.2 and 17.1 seconds,
with a longest lock of 20.1 minutes. Displayed protected quotations do not
behave that way under Rule 610(d), which requires an exchange to avoid
displaying a quotation that locks or crosses another's protected quotation.
TotalView-ITCH carries every displayed order, odd lots included, and an odd
lot is not a protected quotation, so the obvious candidate is that these
episodes are mostly odd-lot orders resting inside the other venue's quote.
`L3Book::best_sized` and `titan-cross min_lot=100` test that: the top of a
side becomes the best price whose level holds at least 100 shares, skipping
thinner levels nearer the top. This is a proxy and not the rule itself,
because a level of 100 shares may be two 50-share orders that the venue
would still display as a round lot, and because the round lot of a few
instruments was not 100 shares; it is stated as a proxy wherever it is used.
The run reports both views of the same day, `lots=all` and `lots=round`,
from the same replay code. Decision rules: (a) crossed episodes under
`lots=round` at most 16, which is half of 33; (b) locked episodes at most
180, half of 360; (c) with the timed passes over all 59,202,007 merged
messages rather than the first 4,000,000, X(incr) at most 0.5 times the
floor median and X(rebuild)/X(incr) at least 2, as in H53. Also changed for
this run, and reported rather than tested: the staleness accounting is
restricted to 09:30:00 to 16:00:00, as the episode accounting already was,
because the thirteenth run's figures were inflated by the quiet hours where
a thousand messages span tens of minutes.

Disclosure: (a) and (b) are set from the mechanism above and from the
all-orders counts, which are known (33 and 360); the round-lot counts are
not known on any machine, and the end-to-end check uses two hand-built
feeds in which one crossed episode is caused by a 50-share order and
disappears under `lots=round` while a 100-share lock survives both views.
A fall to at most half is a weak prediction if the odd-lot explanation is
right, since it would predict a fall to near zero; it is written at a half
so that a partial fall, which would mean round-lot quotes also cross, is
recorded as a refutation rather than argued about afterwards. If the
counts barely move, the episodes are between protected quotations and are
a far more interesting result than if they vanish.

## Pre-registered 2026-11-14, fifteenth run

**H55 (the order book the third brief specifies).** Part 2.1 of the third
source brief gives the order book as a priority queue per side, "ordered by
price (highest first), then time", with "Insert quote: O(log n)", "Remove
quote: O(log n)" and "Match order: O(1)", and its pseudocode pushes orders,
not aggregated price levels. `titan_itch::book::L3Book` instead keeps one
sorted array of price levels per instrument and side, where the best level
is the last element and the orders at a price are already summed into it.
`titan_itch::heapbook`, new in this batch, is the brief's structure: a
binary heap of orders per instrument and side, price then time, over an
order table that is deliberately the same open-addressed, linearly probed,
backward-shift-deleting table as the array book's, so that what is compared
is the priority structure and not the hash table around it.

A heap of orders cannot reach an arbitrary order, and every ITCH cancel,
execution and delete names one, so both ways out are implemented and both
are measured rather than one being assumed: `heap_lazy` drops the order from
the table and leaves the entry to be skipped when it reaches the top, and
`heap_indexed` keeps every order's heap position up to date through every
sift so a delete is a true O(log n) removal. Workloads: `top`, the best
price on both sides of the instrument a message touched, which is what a
market maker reads continuously, and `depth`, the best price and every share
resting at it, which the brief's own success criteria require ("Quote Depth:
Shares available at best bid/ask"). The array book answers both with one
operation, because a level is the sum of its orders; the heaps must visit
every order at the best price. That asymmetry is the structural claim under
test, not a handicap applied to the heap: a heap that cached the total at
each price would be the array book.

Floor: `none` decodes each message and keeps no book, so X(design, workload)
= median `msg_cycles` of that stage minus the median of `none`. Feeds: the
synthetic feed and the public NASDAQ BX sample of 2019-07-30, the first
10,000,000 messages of each, 10 epochs, stage order rotated, PMU on. Before
any timing, all three books are driven through the same messages and
required to report the same best price after every one, and the heaps the
same depth as the array book; the three must also end with the same number
of live orders. Decision rules: (a), (b) and (d) are ratios of medians with
bootstrap 95% intervals, undefined if the denominator's interval reaches
zero; (c) is a two-sided Mann-Whitney U on the per-epoch `msg_cycles`,
p < 0.05 with the medians in the stated direction; both feeds supported, one
partly. Reported, not tested: instructions and L1D load misses per message,
the mean number of shares resting at the best price (which is what the depth
scan walks), and how many stale entries the lazy heap pops.

Disclosure: these thresholds were set after development-machine runs (x86, a
virtual machine, 200,000 synthetic messages, 4 epochs, nanoseconds in place
of cycles, not evidence) gave 1.375 [1.170, 1.561] for (a), 2.329 [1.832,
2.867] for (b) and 1.652 for (c). Both thresholds sit inside those
intervals, so both can fail. The harness check (d) is the one place a
development number is not the basis: that machine showed 0.141, which is
noise it has shown throughout, and 0.10 is written because every M4 run so
far has held its stage medians within about one percent. If (d) fails on the
M4, the rest of this run is not to be read.

## Pre-registered 2026-11-14, sixteenth run

**H56 (the heap's depth query at its best, and fleeting quotes).** The
fifteenth run measured the depth workload against a query that scanned every
entry of the heap and kept those whose key matched the best. A heap can do
better: its ordering makes keys non-increasing from the root down, so every
entry holding the best key is reachable from the root through entries that
also hold it, and a traversal that stops descending at the first smaller key
visits exactly those and at most their immediate children. `best_depth` now
does that, over a reusable stack so the query allocates nothing, with a test
that it finds every order at the best price when all orders share that
price, when only the root holds it, and on the ask side where the key is
inverted. Keeping a running total at each price would remove the traversal
altogether, and would also be the array book, so it is not done.

Parts (a) to (c) re-measure the depth comparison with that query, everything
else unchanged from the fifteenth run. They are written as a test of whether
the fifteenth run's numbers are understood rather than merely recorded: at
the end of its window BX held 42,994 live orders across 8,850 instruments,
about five an instrument, so pruning should find almost nothing to skip and
the BX ratio should barely move; the synthetic feed held 982,360 across 501,
about two thousand an instrument, so its ratio should collapse. If BX moves
a lot, or the synthetic feed does not, the explanation is wrong.

Part (d) is the brief's own advice measured on real data. Part 1.4 of the
third source brief recommends posting quotes "for extremely short duration
(milliseconds)" and cancelling continuously. The untimed pass now records,
from each message's own timestamp, how long every order rests between
arriving and being cancelled or fully executed, and separately for the
orders that arrive at the best price on their side, where that advice would
bite. Reported as the share gone within 100 microseconds, 1 millisecond, 10
milliseconds, 100 milliseconds and 1 second, with the mean, and the count
still resting at the end of the window, which is excluded.

Decision rules: (a), (b) and (c) are ratios of medians with bootstrap 95%
intervals, undefined if the denominator's interval reaches zero; (d) is a
share of the orders that ended inside the window on the BX feed. Both feeds
supported, one partly, for (a). Disclosure: (b) and (c) are set from the
live-order counts above and from the fifteenth run's measured ratios, both
known. No timing of the pruned query exists on any machine: it has run only
on this development machine over a 400,000-message synthetic prefix without
the PMU, which produced the lifetime figures used to check the output format
and no cycle counts at all. (d) is set from nothing measured: this
repository has never measured an order lifetime.

## Pre-registered 2026-11-14, seventeenth run

**H57 (queue position, and the cancellation rate the brief states).** The
fourth source brief makes queue position one of its three headline tasks and
states the mechanism plainly: price priority first, then time priority, so
an order arriving earlier at the same price is filled first, and speed
therefore buys position. It separately asserts that high frequency firms
cancel half of the orders they send. Neither is measured anywhere in it.

Queue position, for an order arriving at a price, is the number of shares
already resting at that price on that side, since under price-time priority
every one of them is filled before any of the arriving order. The arriving
order joins the back of its level, so the count is the level's total after
the add less the order's own size. `L3Book::level_shares` reads it with the
same binary search an add already performs, and is tested against the
reference book at the price every message touches, at prices where nothing
rests, and at both ends of the range. The untimed pass follows every order
from arrival to end and records which of the two ways it ended: some of it
executed, or none of it did. A replacement is a new arrival, because ITCH
gives it a new reference and it loses its place, which is the brief's own
rule.

Parts (a) and (b) are not surprising in direction and are not registered as
if they were: under first in, first out some gradient is mechanical, since
the shares ahead must execute before the arriving order can. What is not
known is the size of it on a real venue, and whether it survives at all once
real order flow, which cancels most of what it sends, is doing the arriving.
The same measurement on the synthetic feed is reported beside it as a
contrast, where order flow is random by construction, with no threshold set
for the difference, because the generator's parameters are arbitrary and a
threshold on them would mean nothing.

Part (c) is a weaker test of the brief's claim than it looks, and is
recorded as such before it is run. The brief's fifty percent is about high
frequency firms; an ITCH feed names every order but no firm, so what is
measured is the venue aggregate across all participants. A venue number
above fifty percent is consistent with the brief's claim without
establishing it, and a number below it would refute the claim for the venue
as a whole but not for the firms the brief means.

Part (d) is the cost of knowing. It is a ceiling rather than a working cost:
the query runs on every arriving order, where a participant needs the
position of its own orders only. The queue design also pays one order lookup
per replacement, because the side of a replacement is not in the message.

**H58 (what immediacy costs, and the brief's power above one).** The same
brief states that the change in price grows as a constant times size raised
to a power above one, and works an example in which a large order is filled
down a tidy ladder of depth. Both are testable from displayed depth without
any model: at every moment the book holds what is actually resting, so the
cost of filling a size immediately is the sum over levels of price times the
shares taken there, until the size is covered or the side runs out.

Two distances from the best price are reported, because a claim about price
impact is a claim about one of them and not both: the average price paid,
which is what the trader pays, and the price of the last level consumed,
which is how far the quote has to move. Both in basis points of the best
price, so instruments of different price are comparable. `L3Book::walk`
returns all three numbers from one traversal and is tested against a
reference that sorts the whole side, at sizes that stop inside the first
level, cross several levels, and exceed the depth available.

This measures the instantaneous cost of consuming displayed depth. It is not
the realised impact of a trade, which would need a model of what the price
would otherwise have done, and this feed cannot identify that. The brief's
claim is tested in the only form the data can answer.

Parts (a) and (b) are registered in the brief's direction deliberately, and
not because it is the direction expected. A 300,000 message synthetic smoke
run on this development machine, without the PMU, produced both series
falling rather than rising, which is a power below one. Registering the
brief's direction makes the run a test of the brief rather than of a guess,
and if the real feed falls as well then both parts are refuted and reported
refuted.

Decision rules: the two cost parts are ratios of medians across epochs with
bootstrap 95% intervals, undefined if the denominator's interval reaches
zero. The shares and rates in the remaining parts are exact counts over the
whole window rather than samples, so they carry no interval; a part claiming
a direction across five sizes or buckets is refuted by a single reversal,
with the reversal named. Both feeds are run; the verdicts are set by the
real feed, with the synthetic feed reported as contrast.

Disclosure: the two cost thresholds, and the one in third part of H58, are
informed by that same 300,000 message synthetic smoke run on this
development machine, which showed the queue query near 1.2x the book, the
10000 share walk near 1.6x the best-price query, and 3.1% of 10000 share
fills unavailable. Nothing from either real feed has been seen: neither
binary has run on a real file, on any machine, and neither has run with the
PMU at all. Every other part is set from nothing measured.

## Pre-registered 2026-11-15, eighteenth run

**H59 (the three defects in the seventeenth run, corrected).** H57 and H58
measured what they said they measured, and their verdicts stand for that. But
three things about how they were measured would not survive review, and all
three were found by re-reading the method rather than by anything in the data.

First, the window. Both ran on the first 10,000,000 messages of the BX session,
which holds 28,734,686. That is 34.8% of the day and it is the opening third.
Cancellation, fill and depth behaviour at the open are not the same as at
midday, so a figure quoted for the session cannot come from the morning alone.
The corrected run uses the whole file.

Second, replacements. ITCH gives a replaced order a new reference, and the
seventeenth run therefore treated it as a new order that ended without
executing. On BX, 1,014,952 of 4,386,147 endings were replacements, so as much
as 23.7% of the orders counted as never executed may have been orders being
repriced rather than abandoned. The corrected run follows the chain: an order
repriced ten times counts once, with its executions summed across every
instance. Both numbers are reported, because they answer different questions,
and the instance figure is kept so the seventeenth run stays reproducible.

Third, wording. "Orders that ended" does not say what it counts. Everything
reported from this run names the population: order instances or order chains,
over the whole session, that both arrived and ended inside it.

Part (d) is the test that matters. It asks whether the first defect changed any
answer. If it passes, the seventeenth run's rates can be quoted for the session
and the window was harmless. If it fails, the window biased the result, the
seventeenth run's rates are quotable only for the opening third, and that is
what will be written.

Decision rules: every quantity here is an exact count over the stated
population, not a sample, so none carries an interval; a part is refuted by
failing its stated threshold, and part (d) is refuted if any one of the rates it
covers moves by 5 percentage points or more.

Disclosure: the threshold in (c) is informed by the seventeenth run's figure of
0.8701 on the opening third of the same file, and the gap in (b) by a 2.88
percentage point gap seen on a 300,000 message synthetic smoke run on the
development machine. Nothing from the whole session has been measured for any
of these quantities on any machine. Parts (a) and (d) are set from nothing
measured: (a) from the fourth brief's own figure of fifty percent, and (d) from
the question itself.

## Pre-registered 2026-11-15, nineteenth run

**H60 (the confound closed, and the shape that explains it).** H58 reported
its two impact series refuted and recorded the refutation as confounded. The
confound was real and was named before the verdict was written: a size the
displayed side cannot fill is excluded from the cost at that size, and on the
whole BX session the exclusion runs from 0.11% at 100 shares to 86.10% at
10000. The mean at the largest size is therefore taken over the deepest and
tightest books in the sample, and the mean at the smallest over nearly all of
them. Comparing those means across sizes compares different populations, so it
cannot test a claim about how cost grows with size.

The fix needs no new theory. Fillability rises with the side's total, so a side
that can fill 10000 shares can fill every smaller size; conditioning on the
largest size gives all five series one denominator and one population.
`L3Book::walk` already answers whether a side can fill a size, so the
conditioned measurement is the same traversal restricted to the sides that
pass, and it is reported beside the unconditioned one rather than replacing it.

Part (c) is the mechanism rather than another rate. If depth thins with
distance from the touch, filling a larger size reaches progressively emptier
levels and the cost per share rises, which is a power above one. If depth
thickens, the cost per share falls, which is a power below one. `depth_profile`
reports mean shares at each of the first eight levels from the touch, over all
quotable sides and over the conditioned population separately, and is checked
against the reference book at every message for buffers shorter than, equal to
and longer than the side, with the nearest level asserted equal to what `best`
reports.

Part (d) is a test of my own earlier caveat, and it can go against me. If the
conditioned series has the same pattern of rises and falls as the unconditioned
one, then conditioning changed nothing, the confound was immaterial, and H58's
refutation should be restated as a plain refutation rather than a confounded
one. That correction would be recorded.

Decision rules: every quantity here is an exact count or mean over the stated
population, not a sample, so none carries an interval. A part claiming a
direction across five sizes or eight levels is refuted by a single reversal,
with the reversal named. Both feeds are run; the verdicts are set by the real
feed, with the synthetic feed reported as contrast.

Disclosure: a 400,000 message synthetic smoke run on this development machine
was observed while building the measurement. On that feed, depth thickened from
494 shares at the first level to 1791 at the eighth, conditioning changed
almost nothing because 98.23% of sides could already fill the largest size, and
both per-share series fell. That is the opposite of the direction registered in
(a) and (b), which are again registered in the brief's direction deliberately,
so that the run tests the brief and not my expectation. The size of the
conditioned population on the real feed is also roughly known in advance, since
H59 measured that 13.90% of BX sides can fill 10000 shares; part (d) is
therefore about the shape of the series and not about how many sides qualify.
Nothing about the real feed's conditioned costs or its depth profile has been
measured, on any machine.

### Errata, 2026-11-16 (the clippy diagnosis, corrected twice, and two real lints)

* **My account of the clippy situation was wrong in its second half.** The
  first half stands: the figure of 1 reported from the measurement machine in
  runs 22, 23 and 24 was the string `error: 'cargo-clippy' is not installed
  for the toolchain 'stable-aarch64-apple-darwin'` matching a grep for lines
  beginning with `error`, so it meant absent rather than zero. I then said
  clippy had never run there and that installing it was the fix. By the time
  the twenty-fifth run checked, `cargo clippy --version` reported 0.1.92, so
  it is installed now; whether it was installed between the two runs or
  whether run 24's invocation failed for a reason other than absence, I do not
  know and should not assert. What is established is that the figure of 1 was
  not a lint count.
* **With clippy actually running there, it reports five real warnings, all in
  code my lint runs cannot reach.** Four `unnecessary_cast` in
  `titan-platform/src/rusage.rs`, casting `ru.ru_minflt` and three siblings
  `as i64` when the field already is `i64`, and one unused `use super::*` in
  `titan-platform/src/kperf.rs`. The container lints x86-64 Linux and reports
  zero for the workspace, even after touching both files to defeat caching, so
  neither warning is visible from where this work is written. The kperf one is
  structurally invisible: the only test in that module is gated
  `#[cfg(not(target_os = "macos"))]`, so the module-scope import is unused on
  macOS and used everywhere else.
* **Both are fixed, and the cast removal was checked rather than trusted.**
  Clippy's claim that the source type is already `i64` comes from one target.
  Removing the casts and rebuilding on Linux confirms the type is `i64` there
  too, so the removal is safe on both; had Linux failed to compile, the casts
  would have been load-bearing on one target and the fix would have had to be
  a cfg rather than a deletion. The kperf import moved inside the gated test,
  which removes the cfg asymmetry rather than adding a second cfg to match it.
* **The standing lesson, which is the one that matters for the paper.** A
  lint run on one target is not a lint run. Two of the five warnings exist
  only on the measurement machine, and the pricing and path suites each
  contained an assertion that passed on the container and failed on that
  machine. Three independent instances of the same blind spot in one day.
  Anything claimed clean is clean on the target it was checked on, and the
  table should say which.

### Errata, 2026-11-16 (four defects of mine in the twenty-fourth run, and a red CI)

* **I asserted bit-exact equality between differently shaped floating-point
  code, which is not a portable property.** Six assertions in the path
  engine's suite compared two arms with `assert_eq!` on doubles. They held on
  the x86-64 container the code was written in and failed on
  aarch64-apple-darwin by about eight units in the last place, 1.849e-15
  relative after 250 compounding steps. That failed two tests on the
  measurement machine and turned continuous integration red, because the CI
  matrix includes `macos-latest`, which is the same target. The arms now step
  through one `step` function so the stepping arithmetic exists in a single
  place and cannot be compiled two ways, and the cross-arm bound is 1e-12:
  four orders above that rounding and four below the smallest logic error
  this suite has caught, the 6.05e-2 window disagreement, so it still
  separates them. The mechanism is **not established**: this host emits no
  fused multiply-add for the expression, so contraction applied on one target
  and not another is a candidate rather than a finding, as is a different
  system library for the transcendentals. Recorded as open.
* **A run's entire statistics section was lost to a label change I made.**
  The twenty-fourth run's script tagged launches `ui1` and `bg1` so the two
  QoS sets could not collide, and `scripts/launches.py` sorted launch labels
  with `int`, which raises on `ui1`. The script crashed after all 60 launches
  had been measured, so the data survived and the analysis did not. The sort
  is now a natural key that accepts a prefix. The lesson is narrower than the
  bug: the harness and the analyzer agree on a wire format that nothing
  checks, and changing one end silently broke the other after the expensive
  part was already done.
* **I spent two scripts chasing a clippy warning that does not exist.**
  Runs 22 and 23 reported one clippy warning on the measurement machine and
  none in the container, and I treated it as an aarch64-specific lint worth
  diagnosing. The count came from `grep -cE "^(warning|error)"`, which
  matched the line `error: 'cargo-clippy' is not installed for the toolchain
  'stable-aarch64-apple-darwin'`. Clippy has never run on that machine. The
  lint count for it in those two runs means nothing and should be read as
  absent, not as zero. Installing it with `rustup component add clippy` is
  the fix, and until then no clippy figure from that machine is admissible.
* **The twenty-fourth run was measured on battery.** The protocol registered
  for it specifies AC power, and H61 demonstrated that the preliminary
  conditions can reverse an answer, so no part of H66 is verdicted from that
  run. Its figures are recorded below as provisional, because they are
  internally consistent, paired inside each launch, and carry 30 of 30 sign
  tests with tight bootstrap intervals. One part needs the repeat more than
  the others: part (f) came in at 1.5454 against a threshold of 1.50, a
  margin of three percent, which is the only comparison in the run that a
  change in conditions could plausibly move across its line.

### Errata, 2026-11-16 (two defects the path engine's own tests found)

* **A shared generator stream invalidated a comparison, and the test blamed
  the code.** The first version of the arm-agreement test drove the
  stop-at-first-touch monitor and the run-to-maturity monitor from one
  generator stream across a loop of paths. The early-stopping arm consumes
  fewer variates, so after the first path the two streams were out of step
  and every comparison afterwards was between two different paths. The test
  duly reported that early termination changed the touch verdict. It did not;
  the test was wrong. Every arm comparison now reseeds both arms per path.
  This is worth recording beyond the fix: any Monte Carlo comparison between
  arms that consume different numbers of variates desynchronizes a shared
  stream silently, and the symptom looks exactly like a logic error in the
  thing being measured.
* **The module disagreed with itself about its own observation window, by six
  percent.** `rolling` seeded its running extremes with the spot at time zero
  while `stored` seeded them from the first observation, so one tracked
  extremes over `t_0 .. t_n` and the other over `t_1 .. t_n`. Both were
  self-consistent and the two differed by up to 6.05 percent on the minimum.
  The window is now `t_1 .. t_n` everywhere, which is also the window the two
  averaging accumulators use, so one `Stats` has one meaning and a contract
  that averages and one that tracks an extreme observe the same dates. The
  two arms now agree bit for bit. Neither arm was wrong in isolation, which is
  why only a cross-check between them could find it.

### Errata, 2026-11-16 (H65: two mechanisms of mine were wrong, and one premise)

* **The premise of the run was wrong, not just a part of it.** H65 was
  motivated by the argument that the measured trading path is unrepresentative
  because a real options decision is a pricing kernel rather than a 22-cycle
  signal lookup, and that such a kernel would exceed the 135-cycle
  per-message budget. It costs 58.733 cycles, 43 percent of that budget, and
  exceeded 135 in 0 of 30 launches. The honest conclusion is the opposite of
  the one the run was built to support: on this machine, pricing on every
  message is affordable, and the path's cheap decision is not the objection it
  looked like. The result is more useful than the one I expected, but I did
  not predict it and the registration records that.
* **Part (c): the mechanism I gave was wrong even though the direction was
  disclosed.** I registered that scalar single precision would not cut cycles
  by more than 10 percent, reasoning that no vector width is gained and the
  operation count is identical. The ratio is 0.750. The instruction counts
  show the error: 254.17 against 298.78, a 15 percent reduction, so the saving
  is in shorter single-precision library routines for the exponential and
  logarithm, not in the arithmetic. Operation count was the thing I assumed
  constant and it was not.
* **Part (d): the prediction held and the reversal did not.** The
  registration named LLVM's own hoisting as the likely reason hand-hoisting
  would fail, and that is what the instruction counts show. It did not
  anticipate that the sign flips on efficiency cores, where the hand-hoisted
  form is 2.1 percent more expensive in 28 of 30 launches.
* **Part (f) is a lesson about reporting rather than about the machine.** A
  64.4x ratio with a 0.31-cycle cost is a true statement that would mislead
  any reader given without its magnitude. Every ratio in this table should
  carry the absolute difference it came from, and where one does not, that is
  a defect to fix before the paper quotes it.
* **A figure elsewhere in this work needs widening, not correcting.** H64
  recorded 3.506 to 3.628 GHz on performance cores and warned that
  conversions must use the measured clock. This run measures 4.05 to 4.61 GHz
  on the same cores for a compute-bound kernel. Both are right; the clock is a
  property of the workload. The warning stands and the range is wider than H64
  alone would suggest.
* **Unresolved from this run, recorded so it is not lost.** Clippy reports one
  warning on the measurement machine and none in the Linux container where
  this code was written, so the warning is specific to aarch64-apple-darwin
  and its text has not been seen. The next run captures it.

### Errata, 2026-11-16 (the traceability audit of 2026-11-16 was itself wrong)

The audit recorded earlier today concluded that twenty-one rows had no
results file anywhere and were therefore not quotable. That conclusion was
wrong on nineteen of the twenty-one. The audit searched each results file for
the literal hypothesis identifier, and it failed in two ways:

* The two earliest files, `results/2026-11-11_m4_spsc.txt` and
  `results/2026-11-11_m4.txt`, document their cases by name rather than by
  identifier. `pp acqrel/128/cached/wfe 81.1` is the record for H1, H2, H4 and
  H7, and `impl=l3 ... msg_cycles=98.3` against `impl=ref ... msg_cycles=326`
  is the record for H10 to H13. A string search for "H1" finds none of them.
* `results/2026-11-12_m4_ac4.txt` labels its section "H34 to H36", a range. A
  search for "H35" does not match a range.

The corrected position: sixty-seven of sixty-nine rows cite a results file.
The two that do not, H5 and H6, are findings about the kperf counter
interface rather than experiments with arms and statistics, so the row is
properly the record for both.

What survives the correction, and it is the part that mattered: nineteen rows
were measured with three to five epochs inside one process, several of them on
battery. That is the protocol H61 showed can reverse an answer, so those rows
are now flagged PRELIMINARY PROTOCOL rather than unquotable, with the file
named. They are quotable when the protocol is stated alongside them, and not
for any effect smaller than the measured noise floor.

H64 then tested how bad that protocol actually was, by re-measuring the same
ping-pong cases over thirty independent launches on wall power. Nine
independent quantities agree to within 3.5 percent: four wait strategies in
nanoseconds, three orderings, the uncached case, and two cycle counts. So the
preliminary ping-pong figures stand. Two do not. The efficiency to performance
wall-time ratio moved from 2.05x over five epochs to 3.057x over thirty
launches, which reverses H8's refutation of the 3x claim. And the streaming
figures are not comparable at all: the preliminary run used 10,000,000
messages where H61 used 200,000, a fifty-fold difference, and it never
measured SeqCst without index caching, which is the case H61 found to cost
6.79x. The index-caching benefit itself reads 26.2x at ten million messages and
1.69x at two hundred thousand, so run length is a variable in that measurement
and is not yet controlled.

I am recording my own audit's failure as prominently as its finding because an
audit that over-reports a gap is not harmless. Acting on it would have meant
discarding nineteen rows of sound data, and the paper would have been weaker
for it.

### Errata, 2026-11-16 (H64, and a defect in how I write registrations)

* Parts (b), (c) and (e) of H64 each state a count or a bound without naming
  the configuration or the unit it applies to. The harness measures sixteen
  cases across two QoS classes and reports both cycles and nanoseconds, and for
  all three parts the answer differs by configuration or by unit. Part (c) is
  not verdicted at all as a result: the efficiency to performance ratio is 0.932
  in cycles and 3.057 in wall time, from the same sixty launches. This is my
  error in writing the registration. Every affected part is reported under
  every reading its wording admits, with the full per-configuration counts, so
  nothing was selected after the data arrived. Future registrations name the
  configuration and the unit for every count and bound.
* The census closed a risk that would have invalidated RQ1 silently. The repo
  documented AcqRel as lowering to `ldapr`, and the build relies on
  `target-cpu=native` selecting a CPU with FEAT_LRCPC to make that true. It
  does: `pop_acqrel` emits `ldapr`, `push_acqrel` emits `ldapur`, both SeqCst
  symbols emit `ldar`, and both Fenced symbols emit two `dmb`. Had the build
  targeted baseline armv8-a, AcqRel and SeqCst would both have emitted `ldar`
  and the comparison would have been between identical instructions while
  still producing plausible-looking numbers.
* A figure used elsewhere in this work needs correcting wherever it appears
  outside this table. The performance core runs at 3.506 to 3.628 GHz during
  cross-core ping-pong, not at 4.4 GHz. Cycles-to-nanoseconds conversions must
  use the measured clock of the arm they came from, which every results table
  here reports per case.

### Errata, 2026-11-16 (traceability audit of the whole table)

* Every one of the 68 rows now carries either the results file that holds its
  data or an explicit flag saying there is none. Before today, 53 rows named
  no source. Thirty-two had exactly one results file that documents them and
  now cite it; five span more than one run and cite all of them.
* **SUPERSEDED. The bullet below is wrong on nineteen of its twenty-one rows
  and is kept only because an erratum that deletes its own mistakes is not an
  erratum. Read "Errata, 2026-11-16 (the traceability audit of 2026-11-16 was
  itself wrong)" further down this file for the corrected position: sixty-seven
  of sixty-nine rows cite a results file, and the two that do not are
  counter-interface findings where the row is properly the record. No row in
  this table is flagged NOT QUOTABLE any more; nineteen are flagged
  PRELIMINARY PROTOCOL instead, with the file named.**
* Twenty-one rows have no results file anywhere in the repo: H1, H1b, H2, H4,
  H5, H6, H7, H8, H9, H10, H11, H12, H13, H19, H23, H24, H35, H43, H44, H45
  and H46. Their runs predate the results/ convention, so the numbers in the
  table are the only record of them. Each is flagged NOT QUOTABLE in the paper
  until it is re-measured under the independent-launch protocol. This matters
  more than it looks: H1, H1b and H2 are RQ1, H4 and H7 are RQ2, H8 and H9 are
  RQ3, and H10 through H13 are RQ5, so four of the paper's five research
  questions currently rest on rows that cannot be cited.
* The reason for the flag is not bookkeeping. The SPSC results file states that
  those runs used three epochs inside one process, on battery, and H61 has
  since shown that exactly that protocol produced an answer which 30 launches
  on wall power reversed. Re-measurement is required, not optional.
* H1b is additionally contradicted by data already in hand. It states that
  SeqCst being more than 5% slower than AcqRel in streaming is refuted. The
  H61 launches give 1.07x and 0.98x with the consumer caching the producer's
  index, and 6.79x and 3.79x without it. The verdict is right for one
  configuration, wrong for the other, and names neither. Recorded in
  results/2026-11-15_m4_launch_protocol.txt as analysis of existing data, with
  no new verdict claimed, since H1b's own run has no results file.

### Errata, 2026-11-15 (energy, H63)

* A pattern now has three instances and is worth stating as one. H60 (c)
  proposed the depth profile as the mechanism for the impact shape and it
  measured true while explaining nothing. H61 (d) proposed L1D load misses
  as the mechanism for the padding effect and they were lower for pad=64 in
  30 of 30 launches in the case where the cycles showed no difference, and
  showed no difference in the case where the cycles differed by 42.6%. H63
  (d) proposed P-cluster residency as the mechanism for the energy saving
  and residency is identical to within half a percentage point across waits
  whose power differs by 2.26x. On this core the intuitive, counter-visible
  proxy has repeatedly failed to account for the measured effect. Every one
  of the three is recorded as confirmed-or-null as a fact and rejected as an
  explanation, and none was restated as support.
* The H63 run's idle baseline is contaminated: Chrome was running, the script
  warned by name, and it was not quit. The baseline spread across launches is
  26.062x. Every verdict was therefore checked both with and without baseline
  subtraction and none changes, because the comparison arms draw 2.8 to 6.6 W
  against a baseline of 0.04 to 1.05 W. Absolute energy per message from this
  run is not trustworthy and is reported as an upper bound only; the ratios
  between waits at the same load are unaffected.
* Two extraction defects in the H63 run script, both mine, neither affecting
  a verdict: the baseline arms were written as L1_idle, L2_idle and L3_idle
  while the parser looked for an arm named exactly `idle`, so the script's own
  summary printed no subtraction (the parser now matches per launch and
  subtracts inside each launch, which is the better design anyway); and the
  pattern used to pull the saturation flag matched an empty string, so
  saturation is not directly confirmed for the high-rate arms. The achieved
  rate of 19.42 to 20.00 against 20.00 million messages per second offered
  places the harness at or near its limit.

### Errata, 2026-11-15 (the launch protocol, H61 and H62)

* H30b: verdict weakened from supported to within the noise floor, not
  established. H62 measured the launch-to-launch spread on the same quantity
  at up to 1.364x while the effect H30b reports is 1.079x to 1.103x. The
  numbers H30b recorded are unchanged; what changes is that they are inside
  the noise floor of the machine, which was not known when H30b was judged.
  This weakening was pre-registered in H62 as the outcome to carry out if
  part (b) failed, and part (b) failed.
* H3b and H14: both are settled by H61, per ordering rather than in general.
  Their observation that 64-byte padding is faster in uncached streaming
  reproduces for SeqCst (29 of 30 launches) and Fenced (23 of 30) and not at
  all for AcqRel (16 of 30). Neither statement named an ordering, which is
  why neither could be settled as written. Their weakness was the protocol,
  three epochs inside one process on battery, and not the observation.
* A noise floor now exists in writing: at a p99 near 2 us on this fanless
  machine, five independent launches of identical work differ by up to
  1.36x. Any claim in this work about an effect smaller than that needs more
  launches before it is admissible.
* H61 part (d) is recorded as supported and simultaneously rejected as the
  explanation for parts (a) to (c), because the cycle cost and the L1D miss
  count are decoupled in both directions. The same pattern appeared in H60
  part (c) one day earlier. Two proposed mechanisms in two days measured
  true and explained nothing, and both are recorded that way rather than
  being quietly dropped or restated as support.

## Pre-registered 2026-11-15, twentieth run

**Why this run exists.** An audit of every non-clean verdict in the table
separated two kinds. Twenty-four are the machine or the venue disagreeing with
a prediction; those are findings and cannot be changed. Three were defects in
how I measured, and two of those three had already been fixed and re-run (H21
by H21b, H58 by H60). The remaining weakness is concentrated in two places,
and both have the same cause rather than two different ones.

**H61 (the padding question, under the independent-launch protocol).** H3 found
no effect, H3b found the opposite of the prediction, and H14 forced the two
index cells into one 128-byte line and came back inconclusive at p = 0.69 with
the record asking for a retest at thirty. The cause is written in the SPSC
results file itself: those runs used three epochs inside one process, on
battery. Three epochs in one process are not three samples of anything that
depends on placement, because they share a code layout, an allocator state and
a thermal state, and placement is precisely what this question is about. The
earlier finding that identical array code moved 3.8% between two builds is the
same effect seen elsewhere in this work.

So the unit of observation becomes the launch. Thirty independent launches on
wall power, and within each launch the pad=64 and pad=128 cases are compared
against each other, so whatever that process received from the loader and the
allocator cancels in the difference. The sign test over thirty paired
differences assumes no distribution at all. Twenty-one of thirty is registered
because that is the exact count at which the two-sided test reaches p = 0.0428;
the count and its p-value are both reported whatever the outcome.

No code changes were needed for this. `Indices` is already `#[repr(C,
align(128))]`, so pad=64 always places both cells in one line and pad=128 never
does, and `titan-stream` already prints `head_off128`, `tail_off128` and
`same_line` for every case, so the placement is verified in the output rather
than assumed from the type.

Direction. (a) through (c) are registered in the direction the M4 has already
shown twice, that pad=64 is cheaper, because that is the claim needing
confirmation or refutation; it is the reverse of the original H3b prediction,
which is already refuted and is not re-tested here. Part (d) is the mechanism:
if one 128-byte line is touched instead of two, the L1D load misses should fall
with it. If (a) holds and (d) fails, the effect is real and its cause is not
the one proposed, and that is what will be written.

**H62 (H30b under controlled conditions).** H30 was invalid by a design error,
paced at 6,627 msg/s when the window's own mean rate is 13,271. H30b corrected
the rate and was supported, but its own evidence records the run as noisy: the
identical real-timestamp arms came out 1.39 to 1.54 times higher than in the
third run, a run-to-run change as large as the effect being measured, on a
machine with more browser activity. A supported verdict resting on a run that
cannot reproduce its own control arm is not settled.

Part (b) is therefore the real test, and it can fail honestly. If the
real-timestamp p99 is stable across five launches, the earlier instability was
environmental and H30b stands. If it is not stable while the constant-rate arm
is, then the burst structure itself produces the instability, the direction
cannot be established on this machine, and H30b's verdict must be weakened to
say so rather than left as supported.

Decision rules: for H61, the unit is the launch and each launch is reduced to
the median of its three epochs per case before anything is compared; paired
differences are taken within a launch; the verdict on (a), (b) and (d) is the
count of launches favouring pad=64 against the registered 21 of 30, with the
exact sign-test p-value reported; (c) is a ratio of medians across launches
with a bootstrap interval. For H62, the unit is the launch, each reduced to the
median of its five epochs per arm, and every part is a ratio of launch
statistics with no interval, since the launches are the whole sample.

Disclosure: while building the analysis I ran thirty launches of
`titan-stream` on this development container, which is shared Linux hardware
and not the measurement machine. That run showed mixed directions and sign
p-values from 0.005 to 1.0 across the six paired cases, so it does not support
the direction registered above; it was run to validate the analysis code and
its numbers are not evidence about the M4. Nothing has been measured on the M4
for either hypothesis under this protocol. The 10% threshold in (c) is informed
by the two earlier M4 battery runs, which showed 185 against 219 and 169
against 220 cycles per message, differences of 15.5% and 23.2%.

## Pre-registered 2026-11-15, twenty-first run

**H63 (what waiting costs in energy).** Every wait-strategy result in this
work so far reports latency and its tail. None reports energy, and on a
fanless machine energy is not a secondary concern: a core that spins burns
power and heats a chip with no fan to move the heat, which is the mechanism
behind every thermal result here. The research note driving this work states
that no Apple Silicon latency-and-energy data for wait strategies has been
published, and a search of this log confirms the figure does not exist in it
either.

`wfe` parks the core on the exclusive monitor until the watched address is
written. `spin` reloads the index continuously and `isb` does the same with a
pipeline hint, so both keep retiring instructions. At 13,271 msg/s the gap
between messages is about 75 us and the work per message is around a tenth of
a microsecond, so more than 99.8% of the time is spent waiting rather than
working. If parking the core saves anything, that is where it shows.

Why `powermetrics` forces a separate run. It seizes the Apple Silicon PMU, so
a binary reading hardware counters through kperf cannot run at the same time
without failing on a busy counter. This batch therefore runs the pipeline with
no PMU at all and measures from outside the process. One `powermetrics`
process is started per arm and stopped when that arm ends, so each raw file
holds only that arm's samples and no alignment by timestamp is needed. The
first and last sample of every arm are dropped, because the first can begin
before the binary and the last can end after it.

Part (c) is the control that gives (a) its meaning. At a saturating offered
load the trading thread has a message waiting every time it looks, so it never
parks and never spins idly. If the three waits still differ there, the
difference is not about waiting and (a) measures something else.

Part (e) applies the lesson from H62 rather than ignoring it. H62 measured a
launch-to-launch spread of up to 1.364x on a p99 near 2 us, and it would be
inconsistent to establish that bound and then report an energy figure from a
single run. Three launches per arm is the minimum that lets the spread be seen
at all, and it is registered as a condition on the result rather than as a
footnote: if an arm's power moves more than 1.25x across launches, the
comparison is inside its own noise and will be reported that way.

Decision rules: each arm's statistic is the mean CPU power over its samples,
first and last dropped, minus the mean of the idle arm in the same launch.
Parts (a) and (d) are counts over the 3 launches and must hold in all 3. Part
(b) is the median ratio across launches. Parts (c) and (e) are ratios of arm
statistics. None carries a bootstrap interval, because three launches is too
few for one and saying so is more honest than printing a wide one.

Disclosure: nothing about energy has been measured on this machine or any
other, at any point in this work, so every threshold here comes from
reasoning rather than from data. The factor of 2 in (b) is the line between an
effect worth a paragraph in the paper and an effect worth a sentence; it is not
an estimate of the magnitude, which is unknown to me. It is possible that
`wfe` on this core holds its clock and saves nothing measurable, in which case
(a) and (b) are refuted and the finding is that parking the core does not pay
on Apple Silicon, which would be worth reporting exactly as loudly.

A measurement risk recorded in advance: the exact field names
`powermetrics` prints vary between macOS releases. The parser names any field
it cannot find, in which file, and reports no number for it rather than
substituting a zero, and every raw file is kept so it can be parsed by hand if
the parser misses. If the format differs on the measurement machine, the data
is not lost.

## Pre-registered 2026-11-16, twenty-second run

**Why this run exists, stated plainly.** Today's traceability audit found that
21 of 68 rows have no results file anywhere in the repo, and that among them
are H1, H1b and H2 (RQ1), H4 and H7 (RQ2), H8 and H9 (RQ3), and H10 through
H13 (RQ5). Four of the paper's five research questions therefore rest on rows
that cannot be cited. Worse, the SPSC results file records that those runs
used three epochs inside one process on battery, and H61 has since shown that
this exact protocol produced an answer which 30 launches on wall power
reversed. These are not bookkeeping gaps; they are measurements of a kind now
known to be unreliable, sitting under the paper's core claims.

`titan-pingpong` answers four of them in one harness, because it already
takes a QoS argument: `ui` requests USER_INTERACTIVE, which prefers
performance cores, and `bg` requests BACKGROUND, which XNU confines to
efficiency cores. One pass of 30 launches in each mode therefore covers the
ordering cost across cores, the wait strategies with their round-trip
distributions, the padding question across cores, and the heterogeneous
scheduler, with the launch as the unit of observation throughout.

Part (a) is phrased as two bounds rather than one because the streaming data
has already shown that the ordering question has no single answer: with the
consumer caching the producer's index the orderings are indistinguishable,
and without it SeqCst costs several times AcqRel. The cross-core harness is a
different measurement and may not reproduce that split, which is exactly why
it is registered as a split and can fail on either side.

Part (c) pairs the two QoS modes by launch index rather than inside one
process, because they are separate invocations. That is weaker pairing than
H61 used, since the two runs share the machine's state at that time but not a
code layout, and it is recorded as weaker rather than presented as equivalent.

Directions. Parts (b), (d) and (e) are registered in the direction the
preliminary runs observed, because that is the claim needing confirmation or
refutation rather than a guess of mine. For (b) that direction is the reverse
of what H4 originally predicted, since H4 is already refuted and is not
re-tested as written.

Decision rules: the launch is the unit; each launch is reduced to the median
of its three epochs per case before anything is compared; differences and
ratios are formed inside a launch and then taken across launches. Counting
parts are verdicted on the count of launches against the registered 21 of 30
with the exact sign-test p-value reported. Ratio parts are medians of
per-launch ratios, which is the statistic registered and not the ratio of
medians; both will be reported, as they were for H63.

Disclosure: the bounds in (a) are informed by the H61 streaming result of
1.07x and 0.98x cached and 6.79x and 3.79x uncached, measured on this machine
under this protocol but in the streaming harness and not the cross-core one.
Nothing has been measured in the cross-core harness under this protocol, on
any machine. The preliminary numbers that (b), (d) and (e) take their
directions from are the unquotable rows named above, and they are used for
direction only and not as thresholds.

A verification step runs before the measurement, and the paper's RQ1 depends
on it. The repo documents AcqRel as lowering to `ldapr`, SeqCst to `ldar` and
Fenced to `ldr` plus `dmb ish`. Checking LLVM's lowering for aarch64 off the
measurement machine confirms this holds at armv8.3-a and above and that at
baseline armv8-a both AcqRel and SeqCst lower to `ldar` instead, which would
collapse the comparison. The build uses `target-cpu=native`, which on an M4
should select a CPU with FEAT_LRCPC, but should is not verified, so
`scripts/census.sh` runs first on the measurement machine and the mnemonics it
reports are recorded with the results. If `ldapr` is absent, RQ1 is not
measuring what it claims and the run says so before any number is quoted.

## Note on the 2026-11-14 run

H42 to H49 were measured on 2026-11-14 at commit f99db57 (tree 83517b1; was ba0bba8 before the author-name rewrite of 2026-11-17) (summaries in
results/2026-11-14_m4.txt). The first attempt of the run script lost power
after it had pushed the code and before it measured anything (GitHub was
confirmed at ba0bba8); the measuring run that followed used the same
commit. The run ended on battery; the operator reported a power failure
about 7 minutes after the start, during the H48 measurement (the script
records the power source only at the start and the end). The core clock
(PMU cycles over wall time) was 4.03 to 4.46 GHz in every H48 epoch but
one (3.68 GHz), and verdicts use cycles. The gap-detection check passed: 100 of 100
held-back datagrams detected on each feed, with no receive timeout, which
closes the H37 correctness item left open on 2026-11-13. The barrier census
matches the reasoning recorded before the run: each seqlock writer loop
holds one `dmb ish`, the copy loops none. Results marked exploratory above
(copy2 below copy1; cas2 below faa2; the PGO stage shifts) were not
predicted and are not claims.

## Pre-registered 2026-11-16, twenty-third run

**Why this run exists.** The trading path measured in this work ends in a
decision costing 22 cycles of toxicity update and 21 of pre-trade risk
checks, against 135 cycles for the whole path on real BX data. A reviewer is
entitled to ask whether a path whose decision is that cheap is
representative, because an options market maker's decision is not a signal
lookup but a pricing kernel. That question has a measurable answer rather
than an argumentative one, and this run takes it.

**Disclosure, because it weakens the registration and hiding it would be
worse.** The harness was smoke-tested on x86-64 Linux before these thresholds
were fixed, and that test produced per-contract wall-time ratios of 2.28
(separate to fused), 0.999 and 0.954 (chain to loop, straddle and
out-of-the-money), 0.867 (single to double precision) and 0.252 (cumulative
normal to fused). Those figures are from a different instruction set and a
different compiler backend and are not the measurement machine, but they were
seen, and a registration written after seeing related data is weaker than one
written before. Every threshold below is therefore set from the mechanism it
tests, and where the x86 observation already sits on the other side of a
threshold that is stated in the part itself, so the outcome cannot be quietly
reinterpreted afterwards.

**What the kernel is.** Black and Scholes 1973 in the Merton form that
carries a continuous dividend yield, with the five first-order sensitivities
as closed forms. The cumulative normal is Abramowitz and Stegun 26.2.17. None
of that is novel and none of it is claimed to be: the contribution is the
cost, not the formula. The kernel is validated before it is measured, by
eight tests whose oracles share none of its machinery. The cumulative normal
is checked against compensated Simpson integration of the density with the
tail integrated outward rather than subtracted from a half, worst absolute
error 7.448e-8 against the 7.5e-8 the approximation claims. Every sensitivity
is checked against a central difference of a price built on that integrated
normal, worst deviation 0.35 of a tolerance whose absolute part is the
difference's own cancellation floor. Put-call parity closes to 4.4e-16, the
fused kernel agrees with the six separate functions to 2.2e-16, and the
strike ladder agrees with the scalar kernel to 5.7e-16.

**Protocol.** 30 independent launches of `titan-greeks 3 ui pmu` and 30 of
`titan-greeks 3 bg pmu`, on AC power with Low Power Mode off, Docker quit and
no profiler attached. The launch is the unit of observation and pairs are
formed inside each launch, as in H61 onward. 4096 contracts per arm per
epoch, 64 strikes per chain call, seven arms, two strike ladders.

**The hypotheses, each naming its arm, ladder, QoS class, metric and
statistic, because H64's registration failed to and three of its five parts
could not be verdicted as written.**

* **H65 (a)** The median across 30 launches of the per-launch ratio of
  `separate` to `fused` cycles per contract, straddle ladder,
  USER_INTERACTIVE, is **at least 2.0**. Mechanism: each of the six separate
  functions recomputes `d1`, `d2`, both discount exponentials and up to two
  cumulative normals, so the fused kernel removes roughly five sixths of that
  shared work. x86 smoke: 2.28, on the same side.
* **H65 (b)** The median across 30 launches of the per-launch ratio of `cdf`
  to `fused` cycles per contract, straddle, USER_INTERACTIVE, is **at least
  0.20**. Mechanism: a fused call evaluates two cumulative normals and one
  density, each dominated by one `exp`. x86 smoke: 0.252, on the same side.
* **H65 (c)** The median across 30 launches of the per-launch ratio of `f32`
  to `fused` cycles per contract, straddle, USER_INTERACTIVE, is **above
  0.90**. Mechanism: the code is scalar, so no vector width is gained, and
  the operation count is identical on a narrower type. x86 smoke: 0.867,
  **already on the refuting side of this threshold**, which is stated here so
  that a refutation is the registered outcome and not a rescue.
* **H65 (d)** The median across 30 launches of the per-launch ratio of
  `chain` to `loop` cycles per contract, straddle, USER_INTERACTIVE, is
  **below 0.90**. Mechanism: two exponentials, one square root and the drift
  term move from once per contract to once per 64. x86 smoke: 0.999,
  **already on the refuting side**, which suggests LLVM already hoists those
  terms out of the per-contract loop at `opt-level=3` with fat LTO. If this
  part is refuted that is the finding, and it is a statement about the
  compiler rather than about the kernel.
* **H65 (e)** Cycles per contract in the `fused` arm, straddle,
  USER_INTERACTIVE, exceeds **135** in at least 21 of 30 launches. 135 is the
  cycles per message of the full single-core trading path on real BX data,
  from `results/2026-11-11_m4.txt`, which is flagged PRELIMINARY PROTOCOL in
  this table. The comparison is therefore against a fixed number from that
  run and is not paired; the kernel's absolute cycles are reported so it can
  be redone when the path is re-measured under the launch protocol. No
  prior observation: the x86 smoke test recorded wall time only.
* **H65 (f)** The median across 30 launches of the per-launch ratio of branch
  mispredicts per contract, straddle to out-of-the-money, in the `fused` arm,
  USER_INTERACTIVE, is **at least 2.0**. Mechanism: the cumulative normal
  branches on the sign of its argument, and the straddling ladder crosses the
  spot so `d1` changes sign along it while the out-of-the-money ladder does
  not. No prior observation: the x86 smoke test recorded wall time only.

**What this run cannot settle.** Whether a vectorized kernel over a strike
ladder reaches the throughput a quoting engine needs, because the kernel here
is scalar and the comparison would need an explicit NEON implementation. And
whether the efficiency cores change any of these answers in wall time rather
than cycles, which the `bg` half of the run will show but which no part above
registers a threshold for, deliberately: H64 showed the two units disagree on
those cores and a threshold that did not name its unit would repeat exactly
the defect this registration is written to avoid.

## Pre-registered 2026-11-16, twenty-fourth run

**Why this run exists.** H65 measured a closed-form pricing kernel at 58.7
cycles, 43 percent of the whole feed-to-decision path, which settled that
vanilla pricing is affordable per message on this machine. It says nothing
about a payoff that depends on the path, because such a payoff is not a
formula evaluated once but a statistic accumulated at every observation: a
running sum for an arithmetic average, a running product for a geometric
one, a running extreme for a lookback, a survival flag for a barrier. The
cost of path dependence is therefore a per-step cost, and this run measures
it.

**Every threshold below was fixed before any arm was timed on any machine,
and that is a deliberate correction to H65's registration.** H65's had to
disclose that an x86 smoke test had already placed two of its thresholds on
their refuting side. Here the thresholds were written first, from the
mechanisms stated with each part, with the harness compiled and the
validation suite passing but nothing timed anywhere, not even in the Linux
container this code was written in.

After they were fixed, an x86-64 smoke test was run to confirm the harness
emits every arm, and it produced per-step wall-time ratios of 0.760 for part
(a) and 0.755 for part (b). Those are from a different instruction set and a
different compiler backend, they are wall time rather than the cycles both
parts are registered on, and they are recorded here because the sentence
above would otherwise become untrue the moment anything ran. They sit just
above both thresholds, so if the M4 behaves similarly those two parts are
refuted. No threshold was moved after seeing them, and the paragraph is
written this way so that nobody has to take that on trust: the thresholds and
these numbers are in the same commit, and the commit that follows carries the
measurement.

**What the engine is, and how it was validated before any of it was
measured.** Geometric Brownian motion under the risk-neutral measure,
discretized exactly rather than by an Euler step: the log increment over a
step is normal with mean `(r - q - sigma^2/2) dt` and variance
`sigma^2 dt`, so there is no discretization bias at any step size and the
only error in a simulated price is Monte Carlo error. The generator is
xoshiro256++ seeded through splitmix64, with normal variates by Box-Muller.
Ten tests check it against oracles that share none of its machinery:

* uniforms against a chi-square over 64 equal bins on 640,000 draws: 57.98
  against 63 degrees of freedom, and the mean within 1.3 standard errors of
  one half.
* normal variates against their first four moments and against a
  Kolmogorov-Smirnov statistic computed with the cumulative normal from
  `pricing`, which is itself validated against compensated Simpson
  integration of the density: KS 0.001624 against a five percent critical
  value of 0.002150, skewness 0.19 standard errors from zero, excess
  kurtosis 0.51.
* the path generator against the exact moments of the process, which are
  closed forms: `E[S_T]` and `Var[ln S_T]` within 1.66 standard errors at
  one, 100 and 250 steps.
* a simulated European call against the Black-Scholes price that the
  `pricing` tests have already validated by central difference and by
  put-call parity: worst departure 1.12 standard errors over five contracts.
* a simulated geometric Asian against the exact closed form for *discrete*
  averaging, derived here rather than taken from the continuous Kemna and
  Vorst result, because at 250 observations the two differ by 0.6 percent in
  variance and enough paths would report a correct engine as broken. Worst
  departure 1.92 standard errors, and the discrete form's gap to the
  continuous limit falls from 4.9e-1 to 4.9e-5 as the observation count rises
  by four orders of magnitude.
* the barrier monitor against in-out parity, which is an identity holding
  path by path and needing no formula: residual between 0 and 5.2e-15, with
  both legs carrying between 34 and 83 percent of paths so the check is not
  vacuous. And against an unreachable barrier, where the monitor must return
  the plain generator's terminal price bit for bit, which catches a monitor
  that consumes, reorders or perturbs a variate.
* the claim in the source brief that coarser monitoring raises survival, as a
  monotonicity property of the monitor: the touch probability rises 0.521,
  0.590, 0.641, 0.667, 0.679 across 4, 12, 52, 250 and 1000 observations.
* log space against price space on identical variates: terminal prices equal
  exactly and the touch flag differs on 0 of 20,000 paths. Rolling scalars
  against a stored path: identical to the last bit. Branchless extremes
  against branchy: identical to the last bit.
* the arithmetic average never below the geometric one, on every path, with
  the two separating by up to 12.9 percent so the inequality is not trivial.

Two defects were found by that suite and are recorded in the errata below.

**Protocol.** 30 independent launches of `titan-paths 3 ui pmu` and 30 of
`titan-paths 3 bg pmu`, on AC power with Low Power Mode off, Docker quit and
no profiler attached. 2000 paths of 250 observations per arm per epoch. Every
arm inside one epoch is seeded identically, so the arms walk the same paths
and differ only in what they accumulate; across launches the launch is the
unit of observation and pairs form inside it, as in every run from H61.
The barrier arms run at two distances: `near` at 80 percent of spot, where a
touch is close to even odds so the monitor's comparison is unpredictable, and
`far` at one percent of spot, which no path reaches so the same comparison is
perfectly predicted.

**The hypotheses. Each names its arms, barrier distance, QoS class, metric
and statistic.**

* **H66 (a)** The median across 30 launches of the per-launch ratio of
  `asian_geom` to `asian_arith` cycles per step, USER_INTERACTIVE, is **below
  0.70**. Mechanism: a geometric average is additive in the logarithm, so it
  needs no exponential until the path ends, while an arithmetic average needs
  the price at every observation and so needs one every step. H65 measured a
  cumulative normal, which is one exponential plus a degree-five polynomial,
  at 15.87 cycles, so an exponential is of that order against a step whose
  other work is a share of one Box-Muller pair and a few flops.
* **H66 (b)** The same statistic for `barrier_log` against `barrier_price`,
  `near`, USER_INTERACTIVE, is **below 0.70**. Mechanism: a barrier
  comparison is monotone under the logarithm, so it can be made against the
  accumulator with the barrier's logarithm taken once, removing the same
  exponential per step.
* **H66 (c)** The same statistic for `lookback_free` against
  `lookback_branchy`, USER_INTERACTIVE, is **above 0.95**, that is, the
  branchless form is **not** faster. Mechanism: the theory of records. In a
  random walk the probability that step `k` sets a new extreme is about
  `1/k`, so the expected number of records in 250 steps is about `ln 250`,
  which is 5.5, or 2.2 percent of steps. A branch taken two percent of the
  time is well predicted, so there is almost nothing for a branchless form to
  win, while `fmax` and `fmin` add a dependency on the accumulator that the
  comparisons do not. This part predicts that an optimization fails.
* **H66 (d)** The median across 30 launches of the per-launch ratio of
  `barrier_log_exit` to `barrier_log` cycles **per path**, `near`,
  USER_INTERACTIVE, lies **between 0.55 and 0.95**. Mechanism: the validation
  suite measured that stopping at the first touch removes 46.2 percent of
  observations at this barrier, so 0.538 is the floor a free saving would
  reach; the added test per step costs something, so the ratio should sit
  above that floor, and below one because the saving is real. Per path rather
  than per step, because the arm that stops early takes fewer steps and a
  per-step figure would divide the saving away.
* **H66 (e)** The median across 30 launches of the per-launch ratio of
  `stored` to `rolling` cycles per step, `near`, USER_INTERACTIVE, is **above
  1.10**, and L1D misses per step are **below 0.05** in both arms. Mechanism:
  250 doubles is 2 KiB, which fits this core's L1 data cache, so storing the
  path costs the store and reload traffic and a second pass over it rather
  than cache misses. The second half of the part is what distinguishes those
  two explanations, and it is registered so the first half cannot be
  attributed to misses after the fact.
* **H66 (f)** The median across 30 launches of the per-launch ratio of
  `rolling` to `terminal` cycles per step, `near`, USER_INTERACTIVE, is
  **below 1.50**. Mechanism: over bare path generation, rolling adds one
  exponential for the arithmetic sum, one addition for the log sum, a minimum,
  a maximum and one barrier comparison per step, and the exponential dominates
  that list. If this part holds, every statistic an exotic payoff in this
  module needs costs less than half again the cost of generating the path.

**What this run cannot settle.** Whether a vectorized generator changes any
of these answers, since the engine is scalar and Box-Muller's sine and cosine
would need an explicit NEON implementation to compare. Whether a Brownian
bridge correction for breaches between observations, which the source brief
describes and this engine does not implement, costs enough to matter: that is
a different measurement and the engine would have to grow the feature first.
And whether any of this transfers to efficiency cores in wall time rather
than cycles, which the `bg` half will show but which no part above registers
a threshold for, deliberately, since H64 found the two units disagree there
and a threshold that did not name its unit would repeat the defect this
registration exists to avoid.

## Pre-registered 2026-11-16, twenty-fifth run

**Why this run exists: it opens nothing and closes three things.** Two of them
are claims this work has already published and left hanging, and the third is
the number H65's headline comparison is measured against. No new module, no
new binary, and no change to any existing harness: every arm below runs code
already in the repository that has already produced a published figure.

**The three holes, named plainly.**

1. The README states that index caching in the ring is worth 26.2x fewer
   cycles per message at ten million messages and 1.69x at two hundred
   thousand, and adds that run length is therefore a variable and is not yet
   controlled. I wrote that caveat myself. A reviewer will ask, and at present
   the honest answer is that nobody has checked.
2. The 135 cycles per message for the full trading path is flagged PRELIMINARY
   PROTOCOL in this table: it was measured with five epochs inside one
   process. H65's main finding, that a Black-Scholes kernel with all five
   sensitivities costs 43 percent of the whole feed-to-decision path, is a
   comparison against that number. A headline should not rest on a figure
   measured a way the paper elsewhere calls insufficient.
3. H19's ring-cost figure of 1.067 carries the same flag, and it is the basis
   of the claim that the ring adds under ten percent to the path.

**Every threshold below was fixed before any arm was timed.** The original
measurement's configuration was read out of `results/2026-11-11_m4.txt`
first, because a repeat at a different message window or batch size would not
be a repeat: feed `20190730_BX_ITCH_50`, 20,000,000 messages, batch 4096,
spread 300, recorded as `stage=full ... 32.34 ns [32.03, 34.87]
msg_cycles=135 msg_instructions=186 msg_l1d_miss=7.24 msg_br_mispred=2.74
ipc=1.38 core_ghz=4.19` over five epochs. The repeat uses the same feed,
window, batch and spread.

**One thing that reading changed, and it matters beyond this run.** The
trading path runs at IPC 1.38 with 7.24 L1D misses and 2.74 branch
mispredicts per message. H65's pricing kernel runs at IPC 5.09 with 0.009
L1D misses, and H66's path engine at IPC 3.77 to 4.74 with 0.0000. So the two
things H65 compared are not the same kind of workload: one is memory-bound and
branch-heavy, the other arithmetic-bound. That does not invalidate the
comparison, since both are cycles for a unit of work, but it does mean H66's
conclusions about the protocol and about power conditions, both measured on
arithmetic-bound kernels, cannot be assumed to transfer to this path. Part (b)
is the test of whether they do.

**Protocol.** 20 independent launches of `titan-stream N 2 pmu` at each of
N = 200,000, 1,000,000, 5,000,000 and 10,000,000, each sweep written to its
own raw file; and 30 independent launches of `titan-t2t
file=data/20190730.BX_ITCH_50 limit=20000000 epochs=2 batch=4096 spread=300
stages=full,direct,book pmu`. AC power, Low Power Mode off, Docker quit. The
launch is the unit of observation.

The sweep goes to separate files per message count rather than being paired
inside one, for a reason now recorded in `scripts/launches.py`: adding
`messages` to the streaming key raises inside `report_padding`, which unpacks
that key positionally, and the whole streaming section then disappears with no
error anyone reads. Doing it the other way turned nineteen lines of statistics
into zero and was caught only by running the analyzer on identical input
before and after the change.

**The hypotheses.**

* **H67 (a)** The ratio of uncached to cached cycles per message, Acquire
  ordering, 128-byte padding, performance cores, median across 20 launches at
  each message count, **varies by less than 1.5x** between the lowest and the
  highest count. Mechanism: the ring's loop body does not depend on how many
  messages pass through it, so neither should its cost per message. If this
  holds, the 26.2x in the README cannot be attributed to run length and must
  be withdrawn or re-attributed; the likeliest other cause is that the two
  figures come from code eight commits apart. If it fails, run length is a
  real variable, the caveat as written stands, and the mechanism needs
  finding.
* **H67 (b)** The median across 30 launches of cycles per message for
  `stage=full` on the real feed at the configuration above **lies within 10
  percent of 135**, between 121.5 and 148.5. Mechanism: the original figure's
  five epochs shared one process, and H61 showed that can reverse an answer.
  Against that, H66 found the protocol and the power source made no
  difference at all to an arithmetic-bound kernel. This path is not
  arithmetic-bound, so the registered prediction is that it reproduces, and
  the reason to doubt it is named: at 7.24 L1D misses per message the
  allocator and code placement that vary launch to launch have something to
  act on here that they did not have there.
* **H67 (c)** The median across 30 launches of the per-launch ratio of
  `stage=full` to `stage=direct` cycles per message is **at most 1.10**.
  Mechanism: H19 measured 1.067, the ring is two operations against the
  path's whole work, and part (b) tests whether this harness reproduces under
  the protocol at all.

**The question this run deliberately does not ask, recorded so it is not
mistaken for an oversight.** H65's comparison treats the pricing kernel's 58.7
cycles and the path's 135 as additive, and the IPC contrast above suggests
they are not: on an out-of-order core, arithmetic at IPC 5.09 can partly hide
behind the stalls of a path running at IPC 1.38, so 193.7 is an upper bound on
the combined cost rather than an estimate of it. Testing that needs a stage
inside `titan-t2t` that runs both, and this work does not alter an existing
harness that has already produced published figures. It is the next run's
question, and until it is measured the paper states 193.7 as a bound and says
so.

## Pre-registered 2026-11-16, twenty-sixth run

**Why this run exists.** H67 established that the streaming harness has always
been run at 200,000 messages, that such a run finishes before the frequency
governor responds so the measuring core sits at 1.085 GHz, and that RQ1's
uncached ordering comparison is entirely different at a run length the feed
implies. One NASDAQ BX session is 28,734,686 messages. RQ1 is one of the
paper's five research questions and its published uncached answer cannot be
quoted as written. This run replaces it.

**What H67 found, as the thing these thresholds are set against.** Warm epoch,
128-byte padding, performance cores, uncached, cycles per message:

        messages      AcqRel      SeqCst      Fenced    core GHz
         200,000      21.930     178.795      86.280       1.085
       1,000,000     133.860     204.815      99.750       2.790
       5,000,000     216.195     213.335      99.415       4.332
      10,000,000     221.990     221.250      99.185       4.315

**This registration is not blind and says so.** H67's sweep is the reason this
run exists, so its numbers were seen before these thresholds were written.
That is unavoidable: a run whose purpose is to replace a figure cannot be
registered without knowing the figure. What it does mean is that these
thresholds are not predictions in the sense H66's were. They are the
commitments that make the replacement checkable: each one states, in advance
and in a form that can fail, what the re-measurement must show for H67's
conclusion to stand on its own harness and protocol rather than on a sweep run
for a different purpose.

**The difference from H67's sweep, which is why this is a separate run rather
than a citation of that one.** H67 used two epochs, whose median is their
mean and so averages the cold first epoch in; its streaming numbers had to be
read from epoch 1 alone after the fact. This run uses three epochs, whose
median selects the warm middle one, which is what H61 used and what the
streaming figures in the paper were computed from. So this is the first
measurement of RQ1 at a realistic run length under the protocol the paper
actually states.

**Protocol.** 30 independent launches of `titan-stream 10000000 3 pmu`, AC
power, Low Power Mode off, Docker quit, no profiler attached. The launch is
the unit of observation and each launch is reduced to the median of its three
epochs per case before anything is compared. Thirteen cases as the harness
emits them. Roughly 25 to 30 minutes.

**The hypotheses.**

* **H68 (a)** The median across 30 launches of the per-launch ratio of SeqCst
  to AcqRel cycles per message, 128-byte padding, index not cached, lies
  **within 10 percent of 1.00**, and the same holds in nanoseconds. H67 put
  this at 0.987 and 0.997 at five and ten million from a two-epoch run; if it
  reproduces under three epochs, the published 6.79x is confined to short runs
  and RQ1's answer becomes that the ordering costs nothing even uncached, once
  the core has ramped.
* **H68 (b)** The same statistic for Fenced against AcqRel is **below 0.60**,
  that is, the explicit double barrier is at least 1.67 times cheaper than the
  RCpc acquire load when the index is not cached. H67 put this at 0.447. This
  part is registered separately from (a) because it is the inversion, and
  because it is the one a reviewer will disbelieve: it says that adding two
  `dmb ish` instructions per operation makes the loop faster. If it holds, the
  paper reports it and states plainly that the mechanism is not established.
* **H68 (c)** With the index cached, the three orderings all lie **within 10
  percent of each other** in cycles per message. This is the half of RQ1 that
  H67 found survives at every run length, and it is registered so that the
  re-measurement is not only of the part that changed.
* **H68 (d)** The warm-epoch core clock, reported per case by the harness, is
  **above 4.0 GHz in every arm**. This is a check on the run itself rather than
  a hypothesis about the ring: if it fails, the run was not long enough to have
  ramped and nothing else in this registration can be read.

**What this run cannot settle, and what would.** It will not explain why
AcqRel's uncached cost grows tenfold with run length while Fenced's is flat.
L1D misses move by 14 percent across that range and cannot account for it. The
measurement that would is a counter sweep on the uncached AcqRel arm across run
lengths, reading store-buffer and coherence events rather than cache misses,
and whether the M4 PMU exposes the needed events is not known here. Until that
exists the paper states the effect, states its size in both units, and states
that the cause is open.

## Pre-registration: H69, which side of the acquire-release pair carries the Fenced inversion

**Why this run exists.** H68 established, and H67 independently reproduced to
one part in a thousand, that replacing `ldapr`/`stlr` with relaxed accesses
bracketed by `dmb ish` makes the uncached streaming loop 2.23 times faster
(ratio 0.4475) and the cached loop 2.45 times slower. Both figures are
measured, neither is explained, and the paper currently states the inversion
and says the mechanism is open. That is honest but it is not finished: a
reviewer who believes the number will immediately ask which half of the
substitution does it, and the existing measurement cannot answer, because
`Fenced` changes the load side and the store side at the same time.

This run crosses the two halves. `FencedLoad` reads with `ldr` + `dmb ish` and
publishes with `stlr`; `FencedStore` reads with `ldapr` and publishes with
`dmb ish` + `str`. Both are correct for an SPSC ring, because each load of the
other side's index still carries acquire semantics and each publishing store
still carries release semantics, whether those semantics come from the
instruction or from the barrier beside it. They are substitutions a programmer
could make, not synthetic arms. A fifth arm, `Unordered`, uses relaxed
accesses on both sides; it is **not a correct ring** and exists only as a cost
floor, so the four ordered arms can be expressed as a fraction of the total
ordering cost in the loop rather than only against each other.

**What is already known about the answer, which is nothing.** Both sides have a
plausible stall story and this registration commits to neither. On the load
side, an RCpc acquire load that misses and takes a coherence miss may have its
ordering obligation enforced conservatively per instruction, where a plain load
followed by a barrier may overlap better. On the store side, a release store
must not become visible before prior accesses are observable, which with a
200-cycle miss in flight may stall at the store, where a barrier followed by a
plain store pays once and then leaves the store buffer unconstrained. The
cached and uncached configurations have opposite signs, so whatever the
mechanism is, it has to explain a sign change too.

**The x86-64 control, already measured.** The same harness on a virtualized
Intel Xeon, where `fence(SeqCst)` lowers to a locked operation or `mfence` and
acquire-release are plain moves, makes both barrier sides a pure cost and makes
them roughly symmetric: at 300,000 messages uncached, AcqRel 25.7 ns,
FencedLoad 78.9, FencedStore 80.1, Fenced 104.9, Unordered 14.7. The two sides
cost within 2 percent of each other and the two together cost less than their
sum, so the barrier cost saturates rather than adding. This is what a machine
with no inversion looks like under this harness, and it is the reference the M4
numbers are read against. A 30-launch version of it is recorded as E2.

**Protocol.** 30 independent launches of `titan-barrier 10000000 3 pmu`, AC
power at the start and at the end, Low Power Mode off, Docker quit, Chrome
quit, no profiler attached. Ten cases: five orderings at pad=128 on performance
cores, each cached and uncached, which is the configuration in which the
inversion was measured. The launch is the unit of observation; each launch is
reduced to the median of its three epochs per case before anything is compared,
and comparisons are paired inside each launch. Significance is the exact
two-sided sign test, for which 21 of 30 is the p < 0.05 threshold
(p = 0.04277). Roughly 30 to 40 minutes.

Write `A`, `FL`, `FS`, `F` for the median cycles per message of AcqRel,
FencedLoad, FencedStore and Fenced. Define the store-side share as
`S = (A - FS) / (A - F)` and the load-side share as `L = (A - FL) / (A - F)`.
Both are computed per launch and reported as the median across launches with a
bootstrap CI. They are shares of the **total difference between AcqRel and
Fenced**, whatever its sign: where Fenced is cheaper, as uncached on the M4,
that total is a saving and the shares divide a saving; where Fenced is dearer,
as everywhere on the x86 control and cached on the M4, the same statistic
divides a penalty. A launch whose `|A - F|` falls below 2 percent of `A` is
dropped from `S` and `L`, because the share then has no denominator, and the
count of dropped launches is reported rather than hidden.

**This registration was revised on 2026-11-17, before any M4 run, after the
x86-64 control in `results/2026-11-17_x86_barrier.txt` was analysed.** Parts
(f) and (g) were rewritten and the share definition above was generalised from
"share of the saving" to "share of the total difference". The revisions are
marked in place and the superseded wording is kept. Revising a registration
after seeing data from the machine it tests would be worthless; revising it
after a control run on a different machine exposed a malformed hypothesis is
the only reason to run a control first.

**The hypotheses.**

* **H69 (a) The saving is store-dominant.** Uncached, `S` is **at or above
  0.70** and the paired sign test on `A - FS > 0` reaches **21 of 30**. This
  would say that replacing the release store with a barrier and a plain store
  is most of the effect, and that the acquire load is close to free.
* **H69 (b) The saving is load-dominant.** Uncached, `L` is **at or above
  0.70** and the paired sign test on `A - FL > 0` reaches **21 of 30**. At
  these thresholds (a) and (b) cannot both hold, so the design is forced to
  pick a side or report neither. That is the point of registering the number
  rather than the direction.
* **H69 (c) The two sides are additive.** Uncached, `|S + L - 1|` is **at or
  below 0.15**. If this fails while (a) or (b) holds, one barrier is doing work
  the other would otherwise have to do, and the inversion is not a sum of two
  independent substitutions. The x86 control already fails an equivalent test,
  so a failure here is a real possibility and not a defect.
* **H69 (d) Each side alone is a measurable effect.** Uncached, both
  `A - FL` and `A - FS` reach **21 of 30** in the same direction. If only one
  does, the inversion is localised to that side outright, which is a stronger
  and simpler result than any share statistic.
* **H69 (e) The sign change is reproduced and localises to the same side.**
  Cached, `F / A` is **above 2.0**, reproducing H68's 2.449, and the side that
  carried the uncached saving is the side that carries the cached penalty, that
  is, `S` cached and `S` uncached are **both at or above 0.70** or **both at or
  below 0.30**. If the sign flips between configurations but the responsible
  side does not, two different mechanisms are at work and the paper says so.
* **H69 (f) The Unordered arm is a usable floor at all.** *Rewritten
  2026-11-17 after the x86 control and before the M4 run; the original is
  below.* Uncached, the median of `(A - U) / A` is **positive**. That is the
  whole hypothesis: not a size, a sign. If it is positive, the arm bounds the
  ordered arms from below and the ordering fraction may be reported. If it is
  negative, removing the ordering made the loop slower, the arm is not a floor,
  and **no fraction-of-loop statement may be made from it in the paper.**

  *What this replaced, and why.* The original read: "the median of `(A - U) / A`
  is at or above 0.30", registered as a scale check so that a 2.23x ratio
  between two ordering policies could be read against the size of the ordering
  cost itself. The x86-64 control (E2) measured that quantity at **-0.3596**
  [-0.7430, -0.2321], with AcqRel cheaper than Unordered in 24 of 30 launches,
  p = 0.0014. Removing all ordering made that loop 36 percent slower. An arm
  slower than the arms it is meant to bound is not a floor: it is a different
  loop, in which the producer is no longer held in step and the two sides
  interleave differently. The original hypothesis was therefore not refuted but
  malformed, and a malformed hypothesis is corrected before the run or it
  contaminates everything computed from it. The original wording stays in this
  file; it is not edited away.
* **H69 (g) The run is long enough to be read.** *Rewritten 2026-11-17, same
  reason, and this is the fourth version of this check.* Every **uncached**
  arm's `epoch_ms` is **above 200**. The cached arms' durations and clocks are
  **recorded without a pass or fail.**

  *Why the restriction.* The cached arms are roughly 20 times faster per
  message than the uncached arms by construction, so at a fixed message count
  they always run short: on the M4 they finish 10,000,000 messages in about 23
  ms, and the x86 control's shortest epoch was 86.559 ms in the cached
  unordered arm. A single global threshold across all arms therefore fails on a
  perfectly correct run, which is exactly how H68 (d) failed. The recurring
  error, now named so it stops recurring, is **writing one global threshold for
  a quantity that differs per arm by design.** The result this run exists to
  produce lives in the uncached arms, so that is where the duration check binds.

**What this run cannot settle, and what would.** It cannot exclude a queue
occupancy explanation. A barrier changes how far the producer runs ahead of the
consumer, so some of the difference between these arms may be a difference in
how full the ring sits rather than in the cost of an instruction. Measuring
that needs occupancy instrumentation inside the hot loop, which would perturb
the thing being measured, so the honest statement is that this alternative is
open. It also cannot say why the sign changes with caching; it can only say
whether the same side is responsible on both sides of the change. And it is a
single machine: the x86 control shows what a machine without the inversion
looks like under the same harness, but one Apple part and one virtualized Intel
part do not make a cross-architecture claim.

## Standing note on validity gates, written after H69 (g) failed

**This note contained a fabrication in its first draft and the corrected
version is below it. The fabrication is left on the record because the note is
about this exact failure.** The first draft opened: "Five registrations have now
carried a version of the same check. H18's, H61's, H68 (d)'s, H69 (g)'s
original and H69 (g)'s revision." H18 and H61 were invented. H18 is a
single-core tick-to-trade cost hypothesis, supported with the predicted range
missed low; H61 is the padding question. Neither carries a validity gate of any
kind. Two citations were manufactured to make a pattern look larger than it
was, inside a paragraph whose subject is not manufacturing patterns. A query
over the table settles it in one line: only H68 and H69 contain a duration or
clock gate, and only those two have a verdict blaming the check.

**The verified count is three versions across two registrations.**

1. **H68 (d)** required every arm's core clock above 4.0 GHz. It failed because
   the cached arms are roughly 20x faster per message and finish before the
   clock has any reason to be high. A per-message threshold cannot control a
   per-duration phenomenon.
2. **H69 (g), as first registered**, required every arm's epoch above 200 ms.
   The x86-64 control showed it would fail on a correct run, because the cached
   arms run short by construction, so it was revised before the M4 run rather
   than after it.
3. **H69 (g), as revised**, required every **uncached** arm's epoch above 200
   ms. It failed at 99 of 150, on `fencedstore` at a 194.6 ms median, an arm
   whose clock sat at 4.368 GHz squarely among the three that passed.

The revision is the worst of the three because its own text named the error it
then committed. It read: the recurring error, now named so it stops recurring,
is writing one global threshold for a quantity that differs per arm by design.
It then set one threshold across five uncached arms whose cost differs by 21x
by design. Naming a mistake in writing did not prevent committing it in the
next paragraph.

**The rule, for every registration after this one.** A validity gate states the
hazard, not a proxy for it. The hazard in all three cases was arms being
compared at different clocks. So the gate is a bound on the **clock spread
across the arms inside one comparison**, computed from the `core_ghz` every arm
already reports, and never a bound on the duration, the message count or the
clock of any single arm. H69's measured spreads, as calibration for future
bounds: 1.094x across the four ordered uncached arms over 540 lines, 1.122x
within the cached set, 1.388x across both sets together, which is why no figure
in this project compares a cached arm against an uncached one.

**And the gate is written before the arm set is frozen, not after.** All three
were written once the arms already existed, at which point the per-arm cost
spread was knowable and was not looked at. Looking at it costs one query
against a previous run.

## Erratum: the author-name rewrite of 2026-11-17 orphaned 31 provenance references

**The defect.** Normalising the git author name on three early commits rewrote
every commit SHA in the repository, because those three were the first three
commits. Twenty-five distinct commit SHAs were recorded as provenance across
twenty-five results files, two sections of this document, four SVG figures and
`scripts/figures.py`, thirty-one sites in all. **Every one of them became
unreachable in a fresh clone the moment the rewrite was pushed.** A reviewer
cloning the repository and trying to check out the commit a measurement was
made at would have got a fatal error, on every results file in the project.

**This was foreseeable and was not foreseen.** The rewrite was validated against
five checks, all of which passed and all of which were about the rewrite's own
correctness: identical tree, identical commit count, one author, one email,
empty diff. None of them asked what else in the repository referred to a commit
SHA. The question "what points at the thing I am about to renumber" is the first
question an author rewrite should raise, and the validation did not contain it.
The five checks were sound and the set of checks was incomplete, which is a
different and harder failure than a wrong check.

**The repair.** Trees are not changed by an author rewrite: only commit objects
are. So each stale SHA was resolved to its tree in a clone that still held the
orphaned objects, and that tree was matched against `git log --all --format='%T
%H'` on the rewritten history to recover the new SHA. All twenty-five mapped
with no ambiguity and none was unresolvable. Every site now reads
`commit <new> (tree <tree>; was <old> before the author-name rewrite of
2026-11-17)`, with the compact `commit <new> (tree <tree>)` form in figure
captions where the canvas width is fixed. Verified against a fresh clone of the
published remote: 26 of 26 commit references and 26 of 26 tree references
resolve, 0 missing.

**The durable fix, which is the part worth keeping.** Every results file now
records the **tree** alongside the commit. A tree hash is invariant under any
rewrite of authorship, committer, dates or messages, so it identifies the exact
source state in any clone of any version of the history. The one file that had
recorded a tree all along, `results/2026-11-11_m4.txt`, was the only one that
was never ambiguous. Future results files record both, and the reproduce script
should be checked for this before the Zenodo snapshot is minted.

**What this changes about the remaining work.** The rewrite was done on the
argument that it was free before a DOI exists. That argument was right about the
DOI and wrong about the repository's own internal references, which cost thirty
one edits to repair. No further history rewrite is to be performed on this
repository for any reason.

## Pre-registration: H70, what burstiness alone costs the tail

**Why this run exists.** Every tail figure in this project so far was measured
either at a constant offered rate or on one real NASDAQ session. A constant
rate has no burstiness at all. The real session has burstiness tangled with
every other property of that particular day, so it cannot separate the cost of
clustering from the cost of load, of message mix, or of that day's volatility.
Neither answers the question a practitioner actually has, which is what
arrival clustering costs when nothing else changes.

The Hawkes arrival schedule added in `crates/titan-itch/src/arrivals.rs` makes
that separable. Burstiness is a single parameter, the branching ratio, and the
mean rate is held constant while it varies: measured Fano factors of 0.900,
1.642, 3.241, 13.920 and 34.501 at branching 0.0, 0.3, 0.5, 0.8 and 0.9, with
the realised rate inside 0.8 percent of the requested one at every setting.
The message content, the book, the code and the offered rate are identical
across arms. Only the arrival times differ.

**The reporting instrument.** The seven printed percentiles are a ladder, not
a curve. `titan-pipeline ccdf` emits 65 log-spaced tail quantiles per run,
stopping at the deepest quantile with at least ten observations beyond it, and
each line carries that count so a reader can see where resolution ran out. At
10,000,000 messages the deepest honest quantile is 0.999999. Saturated runs
emit nothing, because their percentiles describe a growing queue rather than
the system.

**Protocol.** 30 independent launches of `titan-pipeline messages=10000000
instruments=64 rates=5000000 hawkes=0.0,0.5,0.8 epochs=3 ccdf`, AC power at
both ends, Low Power Mode off, Chrome and Docker quit. The launch is the unit
of observation, each launch is reduced to the median of its epochs per arm
before anything is compared, comparisons are paired inside each launch, and
significance is the exact two-sided sign test at 21 of 30, p = 0.04277.

The `hawkes=0.0` arm is the control and is **not** the same thing as the
`rates=` arm: both have the same mean rate, but one draws its arrival times
from a Hawkes process with the self-excitation switched off and the other is
perfectly periodic. Including both separates the cost of randomness from the
cost of clustering, which a single comparison would merge.

**The hypotheses.**

* **H70 (a) Burstiness inflates the tail at a fixed mean rate.** The p99.9 of
  branching 0.8 is **at or above 2.0 times** the p99.9 of branching 0.0, and
  the paired sign test reaches **21 of 30**. Both arms offer the same mean
  rate, so anything here is the cost of clustering alone.
* **H70 (b) The inflation grows with the branching ratio.** Median p99.9
  ordered as branching 0.0 below 0.5 below 0.8, in **at or above 24 of 30**
  launches. Registered separately from (a) because a single large step with no
  ordering between the middle arms would mean the parameter is not acting as a
  dial.
* **H70 (c) The median is almost untouched.** The p50 of branching 0.8 is
  within **1.25 times** the p50 of branching 0.0. If clustering moves the
  median as much as the tail, the arms differ in throughput rather than in
  tail behaviour and (a) is not measuring what it claims.
* **H70 (d) Randomness alone is not the effect.** The p99.9 of branching 0.0
  is within **1.5 times** the p99.9 of the periodic `rates=5000000` arm. This
  separates the cost of a random arrival time from the cost of clustered
  arrival times. If it fails, the comparison in (a) is contaminated by
  randomness and the branching ratio is not isolated.
* **H70 (e) The curves are resolved where they are read.** Every quantile used
  in (a) through (d) carries **at least 100 observations beyond it**, not the
  10 the emitter allows at its deepest point. The p99.9 of a 10,000,000
  message run has 10,000 observations beyond it, so this should pass with four
  orders of magnitude to spare; it is registered because a figure drawn to
  p99.9999 from a short run is exactly the error this emitter exists to make
  visible, and the check should be explicit rather than assumed.
* **H70 (f) No arm saturated.** Every launch reports `saturated=0` for every
  arm. A saturated run's percentiles describe a queue that grew for the whole
  run, and comparing those across arms measures nothing about the system. If
  any arm saturates, that arm is dropped and the fact is reported rather than
  the offered rate being quietly lowered until it passes.

**What this run cannot settle.** It cannot say whether the inflation is caused
by the ring, the book, the wait strategy or the scheduler, only that arrival
clustering causes it. Decomposing it needs the per-stage split that
`titan-pipeline split` already emits, which is a separate registration. It is
also one mean rate: 5,000,000 messages per second was chosen because earlier
runs show it is comfortably below saturation on this machine, and the shape of
the effect at a rate near saturation is not addressed.


## Pre-registration: H71, what clustering costs at the edge of capacity

Written 2026-11-19, before the run. H70 answered its question at one third to
one half of capacity and said so in its own scope note: "the same sweep at
10,000,000 is a new registration, not a re-reading of this one". This is that
registration.

WHY THIS RATE AND NOT ANOTHER. Every earlier sweep on this machine records
saturated=0 at 10,000,000 messages per second and saturated=1 at 15,000,000,
for all three wait strategies, so 10,000,000 is the highest rate the full path
is known to sustain. Service time is roughly 100 to 130 ns per message, which
leaves almost no slack between bursts. If clustering ever costs anything on
this path, it costs it here.

WHAT IS HELD FIXED. Everything but the mean rate. Same binary, same commit,
same 10,000,000 messages per epoch, same 3 epochs, same 30 independent
launches, same spin, isb and wfe, same four arms, same 65 log-spaced CCDF
quantiles gated at ten observations beyond. Holding the message count rather
than the duration keeps the tail resolved to the same depth as H70, which
reached 10,656 observations beyond the p99.9 level. The epoch takes half as
long in wall time because the same messages are offered twice as fast.

THE MECHANISM BEING TESTED, STATED PLAINLY SO IT CAN FAIL. A Hawkes schedule
with branching 0.8 has a measured Fano factor of 13.920 against 0.900 at
branching 0.0, so its instantaneous rate during a burst is many times its mean.
At a mean of 5,000,000 a burst drains before the next one arrives and H70
measured the clustering as free at every depth of the curve, with the ratio
pinned between 0.98 and 1.07. At a mean of 10,000,000 there is no slack to
drain into. The prediction is therefore not that the tail stretches but that
the arm SATURATES: the queue formed by a burst outlives the burst, achieved
rate falls below 97 percent of offered, and the run is excluded by the
instrument's own gate. That is (a), and it is the primary.

WHY (d) IS THE MOST IMPORTANT PART OF THIS REGISTRATION. H70's control became
its finding. A Poisson schedule against a perfectly periodic one at the same
mean rate cost 1.29x at the median, 1.76x at p85, 2.122x at p95.73 and 2.604x
at p99.64, then collapsed to 1.052x at p99.9 and 1.02x in the far tail, at 30
of 30 or 29 of 30 with p below 1e-5 under all three wait strategies. That was
found after the fact, which is the weakest standing a result can have in this
project. (d) states it as a number in advance, at a different operating point,
before the data exist. If it reproduces, the effect is the paper's strongest
claim and it was predicted. If it does not, H70's post hoc finding is confined
to one rate and the manuscript will say so.

WHAT THIS RUN STILL CANNOT SETTLE, WRITTEN BEFORE IT RUNS. It cannot explain
why a random schedule costs what it does. A queueing account is available and
untested: a periodic schedule never presents two messages at once while a
Poisson schedule does so constantly, and the per-stage split that
`titan-pipeline split` already emits is what would decompose it. That is a
separate registration. It remains one machine, one instrument count, one feed,
performance cores only, and two mean rates across H70 and H71 rather than a
curve in rate.

PHYSICAL PROTOCOL. Charger connected, AC power confirmed by pmset at the start
AND at the end of the run, Low Power Mode off, Docker not running, no profiler
attached, all browser tabs closed, no other work on the machine while it runs.
No PMU counters are read, so no sudo is needed and the kperf and powermetrics
exclusion does not apply; thermal state is read through pmset, which does not
touch the PMU.

## Pre-registration: H72, at the rate E3 measured

Written 2026-11-19, after E3 and before the run. Two rates have now been
chosen badly. H70 chose 5,000,000 and answered its question at a third to a
half of capacity. H71 chose 10,000,000 from H70's scope note, which read the
ceiling off runs at a shorter epoch, and every one of its 1,080 cells
saturated. E3 then measured the envelope instead of inferring it.

WHAT E3 MEASURED. The highest offered rate at which all four arms stay
unsaturated is 10,000,000 messages per second at 2,000,000 messages per epoch
and 7,000,000 at 10,000,000 messages per epoch. The ceiling falls by 30
percent when the epoch grows five times longer. H71 was therefore 43 percent
over capacity for its own run length. E3 also shows the degradation is not
clustered-arm specific: at 8,000,000 and 10,000,000 messages per epoch the
PERIODIC arm is the one that saturates first, in 3 of 9 cells, while all three
Hawkes arms are clean, and at 9,000,000 the least saturated arm of the four is
hawkes_0.5 at 2 of 9. Those are 9-cell counts in an exploratory calibration
and are not a finding; they are a reason not to expect clustering to cost
capacity, which is consistent with H70 measuring it as costing nothing in the
tail.

WHY 7,000,000 AND NOT 8,000,000. 7,000,000 is the highest rate with zero
saturated cells for every arm. 8,000,000 has the periodic arm at 3 of 9. A
registration must be able to fail, and running at a rate where the control arm
is already partly over capacity would let a saturated control, rather than an
absent effect, decide the verdict. That was precisely H71's failure.

(c) IS THE POINT OF THE RUN. H70's control became H70's finding and was never
predicted. H71 was supposed to predict it and could not measure it. This is
the third attempt and the first at a rate chosen by measurement. If the
randomness effect reproduces at 7,000,000 with the numbers stated above, the
manuscript's strongest claim rests on a prediction made in advance at a second
operating point. If it does not, the claim is confined to 5,000,000 and the
paper will say so in those words.

WHY THE FAILURE BRANCH IS TIGHTER THIS TIME. H71's (f) allowed the run to be
recorded as untestable and moved on. Allowing that twice would turn the rate
into a free parameter tuned until a result appears. So if (e) fails, no third
rate is chosen by hand: the envelope is remeasured on a finer ladder and the
registration is rewritten against that.

PHYSICAL PROTOCOL. Charger connected, AC power confirmed at the start and the
end, Low Power Mode off, Docker not running, no profiler attached, tabs closed,
no other work on the machine. No PMU counters are read, so no sudo. Expected
wall time is longer than H71's 28 minutes because each epoch feeds the same
10,000,000 messages at 7,000,000 per second instead of 10,000,000; the run
script measures launch 1 and prints its own projection.
