# Gate 4: title, abstract, section map, venue

Written 2026-11-16 on 73 pre-registered hypotheses and 25 results files. Every
number below is followed by the file it comes from. Nothing here is quoted
from memory, and nothing is quoted from a row that carries a PRELIMINARY
PROTOCOL flag unless a later run reproduced it under the independent-launch
protocol.

## The thesis

The contribution is not a faster ring or a cheaper book. It is that on this
class of machine the measurement protocol decides the conclusion about as
often as the code does, and that five specific protocol dimensions can be
quantified rather than warned about. Three published-style claims in this work
reversed under protocol changes that touched no code at all. That is the
paper.

This framing is chosen because it is what the measurements actually support.
A paper claiming "here is a fast trading path on a laptop" would rest on
figures that this work has itself shown to be protocol-dependent. A paper
claiming "the protocol is a confound, here is its size in five dimensions"
rests on the reversals, which are the best-evidenced results in the whole
body of work: each is measured over 30 independent launches with a sign test
and a bootstrap interval, and two of them are confirmed by independent runs
agreeing to one part in a thousand.

## Title

Primary, for arXiv and Zenodo:

> **Same Code, Opposite Answers: Measurement Protocol as a Confound in
> Microarchitectural Evaluation on a Fanless Apple M4**

Alternate if a reviewer finds the primary too informal:

> **Protocol Sensitivity in Single-Machine Microarchitectural Measurement:
> Five Dimensions That Reverse Conclusions on Apple Silicon**

For the JSys submission, which must carry a different title from the arXiv
version (see Venue):

> **When the Harness Decides: Run Length, Epoch Structure and Core Residency
> in Single-Machine Performance Measurement**

## Abstract

Microarchitectural claims about lock-free data structures are usually reported
as properties of code. We measure a software tick-to-trade path and its
components on one fanless Apple M4 laptop under 73 pre-registered hypotheses,
and find that five dimensions of the measurement protocol, none of which
touch the code, change or reverse the conclusions.

Run length is the largest. A single-producer single-consumer ring in which the
consumer does not cache the producer's index shows sequentially consistent
loads costing 6.79 times acquire loads at 200,000 messages, and costing the
same to within three parts in a thousand at ten million (0.997 in cycles,
1.007 in nanoseconds, 30 launches). The short run finishes before the
frequency governor responds and is measured on a performance core at 1.085
GHz against 4.33 GHz for the long one. Over the same range the ranking
inverts: a plain load with explicit barriers either side costs 3.93 times an
acquire load at 200,000 messages and 0.447 times at ten million, confirmed by
two runs with different epoch counts agreeing to one part in a thousand. That
ordering also flips sign with index caching, costing 2.45 times the acquire
form when the index is cached and 0.45 times when it is not. Cache behaviour
does not explain it: the barriered variant carries more L1D misses in every
configuration and is faster regardless. Two of three per-ordering answers
about cache-line padding reverse across the same range.

Epoch structure is the second dimension. Reducing a launch to the median of
two epochs averages a cold epoch in rather than selecting past it; at 200,000
messages the two epochs of one arm cost 217.190 and 21.930 cycles per message,
so their median is a figure for neither.

Core residency is the third. On efficiency cores a cross-core round trip takes
fewer cycles than on performance cores and 3.057 times the wall time, because
the efficiency cluster is pinned at 1.080 GHz against 3.51 to 3.63; the better
wait strategy also changes with the cluster, one winning 30 of 30 launches on
one and losing 30 of 30 on the other.

The reporting unit is the fourth, and follows from the third: a cycles-only
paper and a nanoseconds-only paper reach opposite conclusions about the same
runs. The unit of observation is the fifth: epochs inside one process share a
code layout and an allocator state, and at a p99 near two microseconds five
independent launches of identical work differ by up to 1.364 times, which is
the floor below which no claim here is admissible.

Against that, figures measured at a realistic run length are stable. The full
trading path costs 134.178 cycles per message [134.045, 134.332] over 30
launches, reproducing a five-epoch measurement to within 0.6 percent, and its
ring operations are 6.3 percent of it. A Black-Scholes price with all five
first-order sensitivities costs 58.733 cycles, 43 percent of that path, so
repricing per message is affordable on this machine. One exponential costs
7.647 and 7.693 cycles measured from two independent pairs of arms. Power
source does not matter for an arithmetic-bound kernel: battery and wall power
agree to 0.16 percent on every absolute figure.

We release the harness, the 73 registrations with their verdicts, 25 raw
results files, and a single command that reproduces every figure from a clean
clone.

## What each number in the abstract traces to

| Claim | File |
|---|---|
| 73 hypotheses, 25 results files | `docs/experiments.md`, `results/` |
| 6.79x at 200,000 messages | `results/2026-11-15_m4_launch_protocol.txt` |
| 0.997 cycles, 1.007 ns at 10,000,000 | `results/2026-11-16_m4_rq1.txt` |
| 1.085 GHz against 4.33 GHz | `results/2026-11-16_m4_runlength.txt` |
| Fenced 3.93x then 0.447x | `results/2026-11-16_m4_runlength.txt`, `results/2026-11-16_m4_rq1.txt` |
| Fenced 2.45x cached, 0.45x uncached | `results/2026-11-16_m4_rq1.txt` |
| More L1D misses and still faster | `results/2026-11-16_m4_rq1.txt` |
| Two of three padding answers reverse | `results/2026-11-16_m4_rq1.txt` |
| 217.190 and 21.930 cycles in two epochs | `results/2026-11-16_m4_runlength.txt` |
| 3.057x wall time, 1.080 GHz, wait strategy 30/30 both ways | `results/2026-11-16_m4_crosscore.txt` |
| 1.364x noise floor | `results/2026-11-15_m4_launch_protocol.txt` |
| 134.178 cycles [134.045, 134.332], ring 6.3 percent | `results/2026-11-16_m4_runlength.txt` |
| 58.733 cycles, 43 percent | `results/2026-11-16_m4_greeks.txt` |
| 7.647 and 7.693 cycles | `results/2026-11-16_m4_paths.txt` |
| 0.16 percent battery against wall power | `results/2026-11-16_m4_paths.txt` |

## Section map

1. **Introduction.** The protocol-as-confound claim, and the three reversals
   that motivate it.
2. **Machine, harness and counters.** Mac16,13, kperf on the measuring
   thread, the 24 MHz counter grain, and the instruction census that proves
   `ldapr`, `ldar` and `dmb ish` are each in the binary rather than assumed
   from a source annotation.
3. **The protocol.** The independent-launch design, within-launch pairing,
   the exact sign test, and the measured noise floor of 1.364x. Includes the
   epoch-structure result, since it is a property of the protocol rather than
   of any workload.
4. **Run length decides RQ1.** The ordering sweep, the clock evidence, the
   inversion, the padding reversal, and what is ruled out as a mechanism.
5. **The trading path.** Component ablation, the figures that reproduce, and
   the 6.3 percent ring share.
6. **Arithmetic-bound kernels.** Pricing and path-dependent payoffs: the
   exponential's cost, fused against separate evaluation, and the result that
   a stored path beats a fused loop because the fused loop serializes on its
   accumulator chain.
7. **Core residency and the reporting unit.** Efficiency cores, the
   cycles-against-nanoseconds divergence, and the scheduler evidence: median
   zero involuntary context switches for every lock-free variant on
   performance cores against 53,658 for a mutex baseline on efficiency cores.
8. **Five dimensions, collected.** A table of each dimension, its measured
   size, and which published claim it would have invalidated.
9. **Threats to validity.** One machine, one chassis, one operating system.
   Short runs that never reach sustained-load throttling. Three mechanisms
   recorded as open, including the barrier inversion, which is a headline
   whose cause is unknown.
10. **Related work and conclusion.**

## Venue

**Decision: arXiv and Zenodo first, then JSys.** Reasons and corrections
below.

Three facts corrected against what I told you earlier today.

* **I said JSys has no deadline. That was wrong.** It opens a new submission
  forum per deadline, with recent ones listed as May, August and October. I
  could not retrieve the current one: the instructions page is script-driven
  and did not render. That has to be read off `jsys.org/instructions`
  directly before anything is submitted.
* **JSys review is double blind.** A public repository named for the system
  cannot be linked from the submission.
* **A paper already on arXiv may be submitted, but the JSys title must differ
  from the arXiv title and the system name must differ too.** That is why
  three titles are listed above and why the JSys version names no system at
  all. A measurement study does not need one, so this constraint costs
  nothing here.

What makes JSys the right target anyway: it is diamond open access with no
author or reader fees, it calls explicitly for Problem papers, which is what
this is, it returns reviews within six weeks, and it runs an artifact
evaluation that this repository is already built to pass, since one command
reproduces every figure from a clean clone. Open review with published
reviewer names is a further reason: it is a venue where a paper that reports
its own refutations is read favourably rather than penalised.

Sequencing, and why arXiv goes first: the immediate purpose is a citable
artifact for funded-position applications, and an arXiv identifier with a
Zenodo DOI provides that in days rather than months. JSys explicitly permits
it. The cost is maintaining two titles, which is mechanical.

## What blocks submission

1. The barrier inversion is the paper's most striking result and its
   mechanism is unknown. The paper can state it as open, and should, but a
   reviewer will ask and the answer must be "here is what we ruled out and
   here is the measurement that would settle it".
2. 24 rows still carry PRELIMINARY PROTOCOL. Three such figures have since
   reproduced within 0.6 percent, which is evidence the flag is conservative,
   but no figure from those rows belongs in the paper without that statement
   beside it.
3. The environment metadata is internally inconsistent: 14 results files say
   macOS 27.2 and 6 say macOS 26B123. One is wrong and the reproducibility
   section cannot be written until the machine is asked.
