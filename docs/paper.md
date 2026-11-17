# Gate 4: title, abstract, section map, venue

Written 2026-11-16 on 73 pre-registered hypotheses and 25 results files. Every
number below is followed by the file it comes from. Nothing here is quoted
from memory, and nothing is quoted from a row that carries a PRELIMINARY
PROTOCOL flag unless a later run reproduced it under the independent-launch
protocol.

## The thesis

**Revised twice, and the second revision retracts a claim this document made
as a headline.** History first, because the errata are part of the record.

*First draft, 2026-11-16 morning.* Claimed the contribution was that
measurement protocol decides microarchitectural conclusions. That claim, in
that generality, is Mytkowicz, Diwan, Hauswirth and Sweeney, "Producing wrong
data without doing anything obviously wrong!", ASPLOS 2009
(doi:10.1145/1508244.1508275), which established that measurement bias is
significant and commonplace and demonstrated it from UNIX environment size and
link order. A paper restating it would be rejected in one line, and correctly.

*Second draft, 2026-11-16 evening.* Replaced it with a cross-architecture
headline: sequential consistency costs 18.2 times acquire-release on x86-64 and
1.0 times on the M4, and "the architecture that makes weak ordering free is the
one that makes strong ordering expensive."

**That sentence is the definition of total store order, not a finding, and it
is withdrawn as a contribution.** On x86-64 an acquire load and a release store
are plain moves, so the denominator of that ratio is very nearly free, while a
sequentially consistent store needs a locked operation or a fence, so the
numerator is real. A large ratio on x86-64 and a ratio near one on AArch64,
where `ldapr` and `stlr` both carry ordering in the instruction, is what any
reader of the two memory models would predict before measuring. The number is
correct and it was measured honestly; the mistake was calling the textbook
prediction a discovery. It stays in the paper as a **control**: it is how the
harness demonstrates that it reproduces known behaviour on a known machine,
which is what earns the reader's trust for the machine where the behaviour is
not known.

*Third draft, 2026-11-17, and this is the one.* The nearest prior work is
Khayet and Morrison, "Arm Weak Memory Consistency on Apple Silicon: What Is It
Good For?", ASPLOS 2026. They measure Apple M1 and M4 under Apple's runtime
TSO toggle against native Arm mode across 49 application workloads and conclude
that the Arm memory model has no inherent performance advantage on Apple
silicon, typical execution times being within about 3 percent. And they state
explicitly that they leave the performance of atomic read-modify-write
operations and memory barriers to future work, on the stated assumption that
modern concurrent software is not bottlenecked on synchronization.

**That deferral is this paper.** A single-producer single-consumer ring in a
market-data feed handler is the case in which that assumption fails: the loop
body is a few instructions and an index handoff, so the ordering operation is
not a rounding error on the work, it is a large fraction of it. This work
measures the thing they deferred, on one of the two parts they used, and
reports three results.

**First, and it is the one no memory model predicts: a barrier followed by a
plain store beats a store-release by 2.57 times when the cursor line is
contended, and loses to it by 2.75 times when the line is in L1.** Replacing
`ldapr`/`stlr` with relaxed accesses bracketed by `dmb ish` makes the uncached
streaming loop 2.23 times faster (ratio 0.4475 in cycles, 0.4464 in
nanoseconds) and the cached loop 2.45 times slower, and two runs with different
epoch counts agree on the uncached figure to one part in a thousand. Adding two
full barriers per ring operation is supposed to cost more than a one-way
acquire under every memory model that describes this hardware, and uncached it
costs less than half. The obvious explanation is excluded by the counters: the
barrier arm carries **more** L1D load misses than the acquire arm in every
configuration and is faster uncached anyway.

**H69 then located it, and the location is the whole point.** Crossing the two
halves of the pair over 30 launches shows the effect is entirely on the store
side. Substituting the release store alone accounts for 133.611 of the 124.544
cycles saved, at 30 of 30 launches; substituting the acquire load alone saves
35.080, which is real at 30 of 30 but a quarter of the size and of the opposite
practical sign once both are applied. The two substitutions are strongly
sub-additive: together they save 124.544 cycles where separately they save
168.691, and adding the load-side barrier on top of the store-side barrier
**costs** 9.067 cycles, 10.6 percent, at 29 of 30. So the cheapest correct ring
on this part when the line is contended is neither of the two arms the
literature would compare: it is an `ldapr` acquire load with a `dmb ish` and a
plain store, at 85.269 cycles against 218.880 for the textbook
acquire-release pair.

**The order of the four correct rings reverses completely between the two
caching configurations**, which is the finding a practitioner has to care
about: uncached, fencedstore 85.269 < fenced 94.336 < fencedload 183.800 <
acqrel 218.880; cached, fencedload 8.021 < acqrel 8.053 < fenced 20.567 <
fencedstore 22.120. The best uncached arm is the worst cached arm.

**The mechanism is a hypothesis and the paper will say so.** A release store
must not become visible before prior accesses are observable; with a coherence
miss in flight that obligation is consistent with the store stalling until the
miss completes, where a `dmb ish` and a plain store pay a fixed barrier once and
then retire into the store buffer with no further per-store obligation. That
would explain the saving under contention and the penalty without it, since with
the line in L1 there is nothing to wait on and the barrier is pure overhead. It
is not established. A queue-occupancy explanation is also not excluded, because
a barrier changes how far the producer runs ahead, and instrumenting occupancy
inside the hot loop would perturb what is being measured.

**One supporting measurement makes the premise of all of this concrete.** An
unordered arm, relaxed on both sides and deliberately not a correct ring, runs
the uncached loop at 10.185 cycles against 218.880, so **ordering is 95.3
percent of that loop.** The same arm lost or corrupted data in 180 of 180
epochs on this M4 and in 0 of 30 launches on the x86-64 control, so weak
ordering breaks this ring observably on AArch64 while total store order masks
the identical defect. The claim that synchronization is load-bearing here is
measured on both counts, cost and correctness, rather than asserted.

**Second, a source of measurement bias specific to this machine class.** On the
M4, the two uncached ordering comparisons change by 4.3 and 8.8 times between
200,000 and 10,000,000 messages, the second with a sign flip; on the x86-64
host the same two comparisons change by 1 and 3 percent. The M4 evidence for
the mechanism is direct: a 200,000-message run finishes before the frequency
governor responds and is measured on a performance core at 1.085 GHz against
4.33 for the long run. The clock tracks how long the core has been busy, not
how much work it did, which is why arms processing identical message counts sit
at different frequencies. Duration-driven frequency scaling and heterogeneous
core residency did not exist on a general-purpose CPU in 2009 and are not
discussed in the 2026 work either. This is the specific, post-2009, Apple
silicon instance of the general phenomenon Mytkowicz named, which is a
contribution; claiming the general phenomenon was not.

**Third, a unit result that bears directly on the nearest prior work.** On
efficiency cores the same comparison answers differently in cycles and in
nanoseconds: fewer cycles (0.808 to 0.984) and 3.057 times the wall time,
because the efficiency cluster is pinned near 1.080 GHz. Khayet and Morrison
use execution time as their only performance metric and say so. On a part whose
clusters run at fixed and very different clocks, execution time and cycle count
are not interchangeable and can order two implementations oppositely. This work
reports both units for every figure and shows a case where they disagree.

The protocol material stays, as the mechanism section of the second result. The
three reversals this work produced are then evidence rather than
embarrassment, and they are the best-evidenced results here: each over 30
independent launches with an exact sign test and a bootstrap interval, two of
them confirmed by independent runs agreeing to one part in a thousand.

**Two caveats govern the paper and belong in the abstract.** The x86-64 host is
virtualized, and worse than that phrase suggests: the container exposes no
cpufreq interface at all, so the frequency hypothesis is not merely untested
there, it is untestable from inside. The absence of the run-length effect there
is consistent with the M4's frequency scaling being the cause and equally
consistent with the hypervisor pinning the clock. A bare-metal x86-64 machine
or an Apple machine with a fan would separate them. Until one is run, the claim
is that the effect is absent on a virtualized x86-64 Linux host, and nothing
stronger. Second, the barrier inversion is one Apple part in one data
structure; it is a measured anomaly with an open mechanism, not a
recommendation to anybody about how to write a ring.

## Title

Primary, for arXiv and Zenodo. **Revised 2026-11-17 after H69 located the
effect.** The previous title was "A Full Barrier Cheaper Than a One-Way
Acquire", which the decomposition showed to be imprecise: the acquire load is
the secondary effect at a quarter of the size, and the whole of the inversion
sits on the release store.

> **A Barrier and a Plain Store Beat a Store-Release by 2.6x: Locating the
> Cost of Memory Ordering in an SPSC Ring on Apple M4**

Alternate if a reviewer finds the primary too pointed:

> **The Cost of Memory Ordering in a Single-Producer Ring on Apple Silicon,
> and Why the Run Length Decides the Answer**

For the JSys submission, which must carry a different title and a different
system name from the arXiv version (see Venue):

> **Duration, Not Work: Measurement Bias from Frequency Scaling and Core
> Residency on Heterogeneous Consumer Silicon**

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
| 18.2x and 11.7x on x86-64, 1.01x run-length change | `results/2026-11-16_x86_crossarch.txt` (EXPLORATORY) |
| 4 performance and 6 efficiency cores, 16 GiB, macOS 27.2 build 26B123 | captured by `titan_t43`, to be written into every results file |

## Section map

1. **Introduction.** The measurement the nearest prior work deferred, why an
   SPSC ring is the case where its stated assumption fails, and the three
   results.
2. **Related work.** Positioned early rather than at the end, because the
   contribution is defined by what three specific papers did and did not
   measure. Drafted in full below.
3. **Machine, harness and counters.** Mac16,13, kperf on the measuring
   thread, the 24 MHz counter grain, and the instruction census that proves
   `ldapr`, `ldar` and `dmb ish` are each in the binary rather than assumed
   from a source annotation.
4. **The protocol.** The independent-launch design, within-launch pairing,
   the exact sign test, and the measured noise floor of 1.364x. Includes the
   epoch-structure result, since it is a property of the protocol rather than
   of any workload.
5. **The barrier inversion.** The lead result. The 0.4475 uncached ratio, the
   2.449 cached ratio, the sign change, the counter evidence that excludes
   cache behaviour, and the H69 cross that decomposes the substitution into
   its load side and its store side. The x86-64 run appears here as the
   control that reproduces the textbook prediction on a machine where the
   prediction is known.
6. **Run length decides RQ1.** The ordering sweep, the clock evidence, the
   padding reversal, and what is ruled out as a mechanism.
7. **The trading path.** Component ablation, the figures that reproduce, and
   the 6.3 percent ring share.
8. **Arithmetic-bound kernels.** Pricing and path-dependent payoffs: the
   exponential's cost, fused against separate evaluation, and the result that
   a stored path beats a fused loop because the fused loop serializes on its
   accumulator chain.
9. **Core residency and the reporting unit.** Efficiency cores, the
   cycles-against-nanoseconds divergence, and the scheduler evidence: median
   zero involuntary context switches for every lock-free variant on
   performance cores against 53,658 for a mutex baseline on efficiency cores.
   This section is where the paper argues that execution time alone is not a
   sufficient metric on a part with fixed and unequal cluster clocks.
10. **Five dimensions, collected.** A table of each dimension, its measured
    size, and which published claim it would have invalidated.
11. **Threats to validity.** One machine, one chassis, one operating system.
    Short runs that never reach sustained-load throttling. A virtualized
    control host with no cpufreq interface. Three mechanisms recorded as
    open, including the barrier inversion, which is the lead result and whose
    cause is unknown.
12. **Conclusion.**

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

1. **The barrier inversion's mechanism is unknown** and it is now one of two
   headline results. The paper can state it as open, and must, but the answer
   to a reviewer has to be "here is what we ruled out and here is the
   measurement that would settle it". Ruled out so far: cache behaviour, since
   the barriered variant carries more L1D misses in every configuration and is
   faster regardless.
2. **The cross-architecture result rests on one exploratory run on a
   virtualized two-core host.** It is the strongest claim in the paper and the
   weakest-evidenced. It needs: a bare-metal x86-64 machine to rule out
   hypervisor masking, more launches, and if at all possible a PMU so the
   comparison is in cycles as well as nanoseconds. This is now the project's
   highest-priority measurement.
3. **24 rows still carry PRELIMINARY PROTOCOL.** Three such figures have since
   reproduced within 0.6 percent, which is evidence the flag is conservative,
   but no figure from those rows belongs in the paper without that statement
   beside it.
4. **Related work is not written** and the audit above shows why that is not a
   formality. One search changed the paper's thesis. The sections on
   measurement bias, on SPSC ring design, and on Apple silicon
   characterisation all need the same treatment before any of this is
   submitted.

## Record of what the environment actually is

Settled by asking the machine rather than by typing a string:

    ProductVersion  27.2            BuildVersion  26B123
    hw.model        Mac16,13        cpu           Apple M4
    performance cores 4             efficiency cores 6
    memory          16 GiB
    rustc 1.92.0 (ded5c06cf 2025-12-08), cargo 1.92.0, clippy 0.1.92
    uname           Darwin 27.2.0 arm64

The earlier concern that fourteen results files said macOS 27.2 and six said
macOS 26B123 was not a contradiction. Both are correct and they are the
product version and the build of one operating system. That was my error, not
the files', and it is recorded here because the same error pattern, asserting
a defect from a surface difference without checking what the strings mean,
produced a second false alarm about clippy earlier the same day. Results files
should carry both strings so the question cannot arise again.

## Section 2 drafted in full: Related work

Written 2026-11-17. Every citation in the first group was checked this session
against at least two independent bibliographic sources, or read directly. The
second group is flagged because it has not been, and nothing from the second
group may appear in a submitted manuscript until it has.

### Verified

**Mytkowicz, T., Diwan, A., Hauswirth, M., and Sweeney, P. F. "Producing wrong
data without doing anything obviously wrong!" ASPLOS 2009, pages 265 to 276,
ACM. doi:10.1145/1508244.1508275, ISBN 978-1-60558-406-5.** The paper that
named measurement bias in systems evaluation and showed it is both significant,
meaning it can reverse a conclusion, and commonplace. Its demonstrated bias
sources are the size of the UNIX environment and link order. It also surveyed
133 papers from ASPLOS, PACT, PLDI and CGO and found that none with
experimental results adequately considered measurement bias.

*Relation to this work.* This work's second result is an instance of the
phenomenon Mytkowicz et al. named, in a bias source that did not exist on a
general-purpose CPU in 2009: the frequency a core runs at is a function of how
long it has been busy, not of how much work it was given, so two
implementations given identical message counts are measured at different
clocks. The general claim is theirs and this paper says so in its first
paragraph. What is new is a specific post-2009 source, its measured size (4.3
and 8.8 times, one with a sign flip), and the direct clock evidence for the
mechanism. An earlier draft of this document claimed the general result and was
wrong to.

*A note on the DOI.* Two different DOIs for this paper circulate:
`10.1145/1508244.1508275` and `10.1145/1508284.1508275`. The first is correct
and is what dblp, the authors' own institutional listing, and the ACM record
all give; the second appears in a user-contributed BibSonomy entry and is a
transposition. Noted because the wrong one would be copied silently.

**Wrenger, L., Toellner, D., and Lohmann, D. "TOSTING: Investigating Total
Store Ordering on ARM." ARCS 2023, Springer. doi:10.1007/978-3-031-42785-5_10.
Extended as "Analyzing the memory ordering models of the Apple M1," Journal of
Systems Architecture, 2024, doi:10.1016/j.sysarc.2024.103102.** Apple silicon
implements a runtime-switchable total store order mode, used by the Rosetta 2
x86 translator, alongside the standard AArch64 weak ordering. The switch is an
implementation-defined bit in `ACTLR_EL1`, with support advertised in
`AIDR_EL1`; this is independently documented in the Linux kernel patch series
that exposes it through `prctl`. Having both models on one chip lets the two be
compared with the rest of the machine held constant. Their headline figure is
that TSO is on average 8.94 percent slower than weak ordering, on
multi-threaded SPEC CPU 2017 FP workloads, measured on an M1.

*Relation to this work.* Theirs is a comparison of two hardware memory models
with the program held fixed. This is a comparison of ordering annotations and
barrier placements within one memory model, with the hardware held fixed. The
axes are orthogonal: nothing in an 8.94 percent whole-application figure for
switching the machine's model predicts a 2.23 times difference between two
correct ways of writing the same ring under the weak model.

**Khayet, Y., and Morrison, A. "Arm Weak Memory Consistency on Apple Silicon:
What Is It Good For?" ASPLOS 2026, Pittsburgh, PA, March 22 to 26, 2026.** The
nearest prior work and the paper this one is positioned against. Measures
Apple M1 and M4 under the TSO toggle against native Arm mode: microbenchmarks
for load-load reordering, store-store reordering, store-to-load forwarding and
SIMD load throughput, and 49 application workloads from SPEC CPU 2017, PARSEC,
SPLASH-2x and OpenBenchmarking, single-core and multi-core. Their conclusion is
that the Arm model has no inherent performance advantage on Apple silicon:
typical execution times are within about 3 percent, the harmonic mean TSO
slowdown falls from 4.2 percent to 1.9 percent with software changes, and the
few severe cases, up to about 70 percent, are attributed to Apple
implementation artifacts rather than to TSO's ordering requirements. Their
statistical practice is careful: at least 8 runs per benchmark, continued until
the 95 percent confidence interval is within 1 percent of the median, with
randomized run order and non-parametric tests.

*Relation to this work, in three parts, and this is the paragraph that defines
the contribution.*

First, **they explicitly leave the performance of atomic read-modify-write
operations and memory barriers to future work**, on the stated assumption that
modern concurrent software is not bottlenecked on synchronization. Barriers
appear in their work only as experimental controls. Acquire, release and
sequentially consistent orderings are not measured. This paper measures exactly
that, on one of the same two parts.

Second, **their assumption is reasonable for their workloads and false for
this one.** In SPEC and PARSEC the synchronization operation is a small
fraction of a large loop body. In an SPSC ring carrying market data the loop
body is an index load, a slot access, an index store and a branch, so the
ordering operation is a large fraction of the work. The pre-registered H69 (f)
exists to measure that fraction rather than assert it, by comparing every
ordered arm against a deliberately unordered cost floor. This paper therefore
tests the premise under which the deferral was made, rather than disputing the
conclusion drawn under it.

Third, **two methodological points, offered as complements and not as
criticism.** Their only performance metric is execution time, which they state.
On a part whose performance and efficiency clusters run at fixed and very
different clocks, execution time and cycle count are not interchangeable: this
work measures a case in which the efficiency cores take fewer cycles (0.808 to
0.984 of the performance-core figure) and 3.057 times the wall time, so the two
units order the same pair of implementations oppositely. And frequency scaling
is not discussed in their paper; the second result here is that run duration
changes which ordering wins, by up to 8.8 times with a sign flip, through the
clock. Their protocol of repeating until the confidence interval is tight does
not by itself address this, because a bias that is a function of run duration is
reproducible: repeating a short run faithfully reproduces the short run's
answer. That is the sense in which this is a complement rather than a
correction, and the paper will say it in those words.

### Still to verify before submission, and not to be cited until then

* **x86-TSO.** The rigorous programmer's model for x86 multiprocessors, by
  Sewell, Sarkar, Owens, Zappa Nardelli and Myreen, which the surveys cite for
  the claim that x86 is TSO and that acquire-release therefore lowers to plain
  moves. The background claim is also derivable from the Intel and ARM
  architecture manuals directly, which may be the better citation for a
  measurement paper. Exact venue, year, pages and DOI not checked this session.
* **The practitioner expectation.** The widely held engineering belief is that
  ordering is nearly free on x86 and costs real barriers on ARM. That belief is
  correct as stated, and the inversion measured here does not contradict it,
  because it compares two ordered forms rather than ordered against unordered.
  This distinction needs stating carefully in the introduction so no reader
  takes the paper to be claiming something it is not. Blog and forum sources
  are evidence of what practitioners believe but are not citable for the
  technical claim; the manuals are.
* **SPSC ring design.** The lock-free queue literature, for the design of the
  ring itself rather than for ordering cost. Not surveyed this session.
* **Apple silicon characterisation.** Independent microarchitectural
  measurement of the M-series beyond the two papers above. Not surveyed.
* **The ASPLOS 2026 DOI.** Khayet and Morrison was read as an author-hosted
  PDF. The proceedings DOI and final page numbers must be filled in from the
  ACM record before submission.

### What this section means for the submission

Related work was the largest hole in Gate 4 and it is the one that nearly sank
the paper twice: once by restating Mytkowicz, once by presenting the definition
of total store order as a discovery. Both were caught by searching, and both
were caught only after the claim had already been written down. The order is
now fixed for the rest of this project: **the prior-work search precedes the
claim, not the manuscript.**
