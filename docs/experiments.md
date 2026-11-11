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
| H14 | With head and tail forced into one 128-byte line, 64-byte padding is faster than 128-byte padding in uncached streaming | pending | |
| H15 | On a real NASDAQ BX day the L3 book matches the reference book after every message, with no unknown order references | pending | |
| H16 | On real data the L3 book takes fewer L1D misses per message than on the synthetic default mix (13.8) | pending | |
