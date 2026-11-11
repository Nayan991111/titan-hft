# Security

## Scope and threat model

Titan is a measurement harness that runs on the experimenter's own machine.
It opens no listening network socket except the prototype `titan-core`,
which binds UDP on the loopback address 127.0.0.1:5555 only. Input files
(ITCH 5.0 captures) are read from local disk.

Properties the code aims to keep, and how they are checked:

| Property | Check |
|---|---|
| Malformed or adversarial ITCH input never causes undefined behaviour | All parsing is bounds-checked safe Rust. `crates/titan-itch/tests/hostile.rs` feeds empty and truncated bodies, zero and oversized length prefixes, order reference 0, unknown orders, oversized executions, duplicate references, out-of-range instrument locates, and random framed and unframed bytes. |
| Invalid messages are counted, not silently applied | The L3 book drops and counts adds with reference 0 or an unknown instrument, and counts references to unknown orders (`L3Book::unknown`). |
| Capacity limits fail loudly | The order table stops with a clear panic above 70% load rather than degrading; size it from the feed (as `titan-replay` and `titan-t2t` do). |
| No known-vulnerable dependencies | `cargo audit` reports no advisories for `Cargo.lock`. |

In `titan-platform`, `titan-spsc` and `titan-itch`, `unsafe` is confined to:
raw-pointer access to the pinned order table and ring storage (indices are
masked to the table size), the shared ring cells (`UnsafeCell` with
single-producer, single-consumer ownership), `mmap`/`mlock`/`mincore`,
`getrusage`, QoS calls, the counting global allocator, inline assembly for the
counter and wait instructions, and the kperf bindings. The prototype crates
`titan-core` and `titan-feed-sim` also use `unsafe`; they are kept only as a
baseline and are not part of the measured path. kperf and kperfdata are loaded from
their absolute system paths; their symbols are cast to typed function
pointers after a null check.

PMU runs need root (`sudo`). Run them only on a machine you control, from a
checkout you have reviewed.

## Reporting a vulnerability

Please use GitHub's private vulnerability reporting: the **Security** tab of
this repository, then **Report a vulnerability**. Include the commit, the
input that triggers the problem, and the observed behaviour. Reports are
acknowledged within 7 days.
