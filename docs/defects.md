# Defect register

Every defect found in this project, what it was, how it was found, and what
was done. Nothing is removed from this file once added. A defect that is
closed keeps its entry and gains a resolution; a defect that cannot be closed
keeps its entry and says so.

The register exists because this project had two kinds of verification and
only one of them was working. Mechanical checks ask whether a number
reproduces: whether a script runs, whether a file has the expected line count,
whether a figure regenerates byte for byte, whether a second implementation
agrees. Semantic checks ask whether the number is the right number for the
claim it supports. Every gate built here before 2026-11-20 was mechanical.
The Fano defect below passed all of them.

**Status at 2026-11-24: forty four closed, none partly closed, none open. Forty four entries in all.**
Partly closed are D4, where the lead results are now fully recomputable but a
cycle-level replication still needs an interactive run with root, and D8,
where the hypothesis that latency tracks the coefficient of variation rather
than the Fano factor is registered but not yet run. D5, D6 and D7 were each
partly misdiagnosed and every entry carries its correction.

---

## D1. The abstract states the wrong number of hypotheses

**Found** 2026-11-20 by counting rows and comparing with the manuscript.
**Severity** factual error in the published abstract.

The abstract states "a pre-registered protocol of 74 hypotheses". The table in
`docs/experiments.md` holds 80 rows. The figure was correct when written and
was not updated as rows were added.

**CLOSED 2026-11-20** the abstract states 77 hypothesis rows and three
exploratory calibrations, and `scripts/check_claims_semantics.py` fails if the
manuscript and the table disagree.

---

## D2. The paper never discloses that its cost-floor arm is not a correct ring

**Found** 2026-11-20 while auditing the abstract against the data.
**Severity** the most serious honesty defect found in this project.

The abstract's lead figure, "we measure that fraction and find it is 95.3
percent", is computed against an arm with all memory ordering removed. That
arm fails its own correctness check. `results/2026-11-17_m4_barrier.txt`
records that all 180 `sum_ok=0` lines in the run belong to it, 90 contended
and 90 L1-resident, while each of the four correct rings reported `sum_ok=1`
in 180 of 180. The source calls it a cost floor and says it is not a correct
ring. `docs/paper/main.tex` contained zero occurrences of that fact.

Using an incorrect arm as a floor is legitimate and standard. Failing to say
so next to the number is not. A reviewer opening the artifact finds it in one
grep and reads the omission as concealment.

**CLOSED 2026-11-20** all three occurrences of the figure now carry the
disclosure in the same passage, and `scripts/check_claims_semantics.py` fails
if any occurrence appears without it. The checker normalises whitespace first,
because the first version of it was defeated by a LaTeX line break inside the
phrase it was looking for.

---

## D3. Two numbers that invite division and do not divide

**Found** 2026-11-20.
**Severity** presentation defect that reads as an arithmetic error.

The abstract says the store-side substitution "accounts for 133.611 of the
124.544 cycles saved" and the contribution states the store-side share as
1.0702. A reader divides and gets 1.0728. The two are different estimators:
the share is the median of the per-launch ratios, the quotient is the ratio of
the medians. Both are defensible and they are not equal, which is exactly what
a reader is not told.

**CLOSED 2026-11-20** each number is labelled with the estimator that produced
it, and the ratio of the medians, 1.0728, is stated beside the median of the
ratios, 1.0702, so the reader is not left to discover the difference.

---

## D4. The raw measurements behind the lead results were never committed

**Found** 2026-11-20 by counting records per result file.
**Severity** artifact reproducibility failure at the core of the paper.

Fifteen of the thirty-two files under `results/` contain no per-record lines
at all. They hold prose, medians and intervals. The file that backs the first
three contributions, `results/2026-11-17_m4_barrier.txt`, is 269 lines and
holds one record. The CCDF work is the exception: H70, H72 and E3 commit their
full raw output, 143,101 records in total.

The binary was never the problem. `titan-barrier` emits a complete `bd` record
per arm per epoch, carrying `ns_per_msg`, `epoch_ms`, `sum_ok` and the
switch counts. The delivery script that produced the committed file aggregated
those records and discarded them.

The consequence is precise. Nobody, including the author, can recompute the
medians, the bootstrap intervals or the thirty-of-thirty sign tests behind the
lead results. They can only read the summary and trust it. An artifact
evaluation that asks for results to be reproduced rather than merely for the
code to run would fail on this.

Partly closed 2026-11-20, superseded by the full closure below. The lead
results became recomputable then; the cycle-level replication came later.

H76 re-ran the barrier cross and retained all 900 records, which are committed
in `results/2026-11-20_m4_barrier_raw.txt`. Every median, interval and sign
test behind the paper's first three contributions is now recomputable from
committed data by anyone. The run took 3 minutes 58 seconds, which is what the
original file had said it would.

It also replicated, on a newer operating system build: the contended inversion
came back at 2.5401 against the published 2.57, the resident inversion at
2.5146 against 2.75 with its sign intact, both rankings identical arm for arm,
and the correctness flags identical at 180 of 180 for each correct ring and 0
of 180 for the cost floor.

**Still open, and narrower than the original defect.** The original ran with
the PMU and reported cycles; this ran without it and reports nanoseconds,
because the PMU requires root and an unattended script must not contain a
password prompt. Ratios were compared, which holds where the arms run at
similar frequency; the original verified that across its 540 lines at 4.035 to
4.414 GHz and this run could not. A cycle-level replication needs an
interactive run. The same applies to the run-length sweep, which uses
`titan-stream ... pmu` and therefore also needs root.

**CLOSED 2026-11-21.** H77 ran the cross interactively under `sudo` with the
counters active: 30 launches, 900 records, every one carrying `msg_cycles`,
`msg_instructions`, `msg_l1d_miss`, `msg_br_mispred` and `core_ghz`, in 3
minutes 27 seconds on wall power.

The inversion holds in cycles. Contended, 2.5766 at 30 of 30; resident, 2.7008
at 30 of 30, so the sign change is a property of the work done and not of the
clock it was done at. The five orders rank identically in cycles and in
nanoseconds at both caching states, arm for arm. Correctness is 180 of 180 per
correct ring and 0 of 180 for the cost floor.

And the assumption that stood unverified was worth checking, because it was
wrong where it mattered most. H76's nanosecond figure for the resident
inversion was 2.5169. In cycles it is 2.7008, a difference of 7.3 percent, and
the cycle figure agrees with the 2.75 the original PMU run published while the
nanosecond replication did not. The frequency confound H76 could not measure
accounts for the gap between its own number and the original. The paper now
reports cycle ratios, which is what the registration said would happen if the
frequency check failed.

---

## D5. The artifact script re-measures and can overwrite committed figures

**Found** 2026-11-20 by running it.
**Severity** the abstract's reproducibility claim was misleading. The second
half of the original report, that the script corrupts published figures, was
WRONG and is corrected below.

The abstract said "the artifact reproduces every figure and table with one
command". The script re-measures on the host it runs on, and its own closing
lines say the numbers will not match the paper unless it is run with `--full
--pmu` on an Apple silicon machine on wall power. That part of the defect is
real and the wording is corrected.

**Correction, same day.** The original entry also claimed that running the
script rewrites the five publication figures "with numbers from the wrong
machine", on the evidence that one invocation during the audit modified all
five. That inference was wrong. The figure scripts read only from `results/`;
fresh measurements are written to `artifacts/<timestamp>/` and never into
`results/`. The script says so itself at line 166. Comparing the extracted
text of a committed figure against a regenerated one shows the content is
identical. The five files changed because of the embedded creation date, which
is D7, and for no other reason. The error here was reading a byte difference
as a content difference without checking, which is the same failure mode as
D8: a mechanical observation taken for a semantic one.

**CLOSED 2026-11-20.** The claim is reworded: the abstract says every figure
regenerates from the committed measurement files with one command, and that
reproducing the measurements requires an Apple M4 on wall power. The script
additionally declines to regenerate the publication figures outside `--full`.
That guard is defence in depth rather than a fix, since the figures read only
committed data today, and it is kept so that a future change wiring fresh
measurements into them cannot quietly reach `docs/figures`.

---

## D6. The hypothesis chart is stale

**Found** 2026-11-20: regenerating `docs/figures/hypotheses.svg` changes it.
**Severity** cosmetic, on the front page.

It was generated when the table held fewer rows and has not tracked it since.

**CLOSED 2026-11-20.** Regenerated from the current table. The chart is built
from `docs/experiments.md` by `scripts/figures.py` and writes SVG, so it never
depended on D7; the original entry said otherwise and was wrong about the
cause.

---

## D7. The manuscript figures are not byte-reproducible

**Found** 2026-11-20 while investigating D5.
**Severity** limits what any gate can verify.

Matplotlib writes a `/CreationDate` into each PDF, so regenerating a figure
from identical data produces a different file. The byte-identity check that
guards the hand-written SVG figures cannot be extended to the PDFs as they
stand.

**CLOSED 2026-11-20, for the property that is achievable, and the other one is
documented rather than pretended.**

`scripts/figures_paper.py` passes a metadata mapping with the creation date
suppressed to every `savefig`, so a figure is now a function of its input data
and not of the moment it was produced. Two regenerations one second apart on
the same machine produce byte-identical files.

**Cross-platform byte-identity is NOT achievable and the first version of this
fix wrongly assumed it was.** Regenerating the same figures from the same
committed data on macOS produced files 19 to 121 bytes smaller than the ones
generated on Linux, consistently, across all five. The cause is font
subsetting: matplotlib embeds a subset of whatever font the host resolves, and
the host's FreeType version decides how that subset is written. Nothing about
the measurements differs. A gate that compares regenerated bytes against
committed bytes therefore fails on any machine that is not the one that
produced the commit, which is every reviewer's machine.

The check is accordingly same-machine determinism: generate twice, compare.
That is the property the project can guarantee and the one that matters, since
it is what stops a figure drifting from its data. The committed figures are
the ones produced on the measurement machine, an Apple M4, because that is the
platform the paper is about.

---

## D8. Fano factor was used where a coefficient of variation was required

**Found** 2026-11-20, one hour after the opposite conclusion had been stated.
**Severity** would have been a fatal error in review.

The project characterised arrival burstiness by the Fano factor of event
counts and concluded, on that basis, that its clustering results contradicted
Kingman's heavy-traffic approximation. Kingman's variability term consumes the
squared coefficient of variation of the interarrival times, which is a
different statistic. Measured on the identical schedules by
`titan-arrivals`, the Fano factor rises from 0.976 to 25.526 across the arms
while the squared coefficient of variation moves from 1.000 to 1.024.

The approximation therefore predicts almost no clustering penalty on these
schedules, and almost none was measured. The result confirms the theory
instead of contradicting it, and the contradiction claim was an artefact of
using the wrong statistic.

Every mechanical gate passed throughout. The figures regenerated byte for
byte, an independent reimplementation reproduced every number, and the number
was still the wrong quantity for the argument.

Partly closed 2026-11-20, superseded by the full closure below. CLOSED 2026-11-20.** The Related Work section now states what the
approximation predicts and what was measured, and names the dissociation
between the two statistics as the contribution rather than a contradiction of
theory. `crates/titan-itch/src/bin/titan-arrivals.rs` reports both statistics
side by side on the same schedules so the two can never again be confused.
`scripts/check_claims_semantics.py` fails if the manuscript mentions the Fano
factor in a paragraph that does not also name the coefficient of variation.
**Still open:** the hypothesis that the latency penalty tracks the coefficient
of variation rather than the Fano factor is not yet registered or run.

**CLOSED 2026-11-20, the remaining half.** H74 registered the competing
prediction and ran it: branching held at 0.8 so the Fano factor is fixed, the
decay rate swept so the squared coefficient of variation moves. Across the
ladder cv squared goes 1.1005 to 8.1921 while Fano goes 24.79 to 25.78, so
the two theories predict 7.444 times and 1.040 times for the same comparison.
The measured median ratio is 6.232 and the peak is 13.5030 at 30 of 30
launches, p=1.86e-09, with 841 ns becoming 11356 ns. The periodic and Poisson
references show no effect against the low-beta arm, 8 and 9 of 30.

So the statistic this project had been using was not merely a different
summary of the same thing. It predicted no effect where a factor of thirteen
exists. The error that started this entry was not a slip of terminology.

---

## D9. The headline result is the only measurement on a different OS build

**Found** 2026-11-20, by an environment probe run on the measurement machine
rather than inferred from this workspace.
**Severity** confounds the strongest claim in the paper.

macOS on the measurement machine moved from 27.2 build 26B123 to 27.2.1 build
26B131 between 12:49 and 17:12 IST on 19 November. Every result in this
project was taken on 26B123 except one: H72, run at 17:12 that day, which is
the registered replication at 7,000,000 messages per second and the source of
the paper's headline figure of 3.4436.

H72's registration states that a single variable changed from H70, the mean
rate. That is false. Two changed. The cross-rate claim, that the peak grows
from 2.7765 at 5,000,000 to 3.4436 at 7,000,000 and moves from about p99.55 to
about p98.12, therefore varies rate and operating system together.

**How it was missed.** On the evening of 19 November the operator reported
that a software update appeared to have run while the machine was unattended.
The platform lines of H71 and of E3 were checked, both read 26B123, and the
conclusion "platform unchanged across your software update" was written down.
Both of those runs predate the update. H72 had not yet run. Nothing re-checked
the platform after it did. The check was performed on the wrong side of the
event it was meant to detect.

**CLOSED 2026-11-20, favourably.** H75 re-ran H70's exact configuration on
build 26B131 with 30 fresh launches. All five registered parts hold, and by
margins far inside their tolerances: the peak ratio came back at 2.7873,
2.6835 and 2.5706 against H70's 2.7765, 2.7505 and 2.5430, differences of 0.4,
2.4 and 1.1 percent against a registered 15; the peak location was identical
to the quantile for two wait strategies and 0.089 decades away for the third,
against a registered half decade; the median ratio differed by 0.7, 0.1 and
0.4 percent.

The update did not move the measurement, so the cross-rate comparison varies
one thing again. The paper gains a claim it could not previously make: the
effect reproduces at 30 independent launches, two days later, across an
operating system revision. That is reproduction across time and platform, not
a re-analysis of the same bytes, and it is a stronger statement than the one
the confound threatened.

---

## D10. The abstract claims a uniform launch count the project does not have

**Found** 2026-11-20, by tracing every manuscript number to its results file
and then to the launch count that file states.
**Severity** an overstatement of the protocol, in the abstract, attached to a
headline contribution.

The abstract says every result comes from a protocol with "30 independent
launches per comparison". The run-length sweep did not use 30. Its file states
"20 independent launches of `titan-stream N 2 pmu` at each of N = 200,000,
1,000,000, 5,000,000 and 10,000,000", with 30 used only for the separate t2t
part of the same file. The 16.33 spread the abstract and the contributions
both quote comes from the 20-launch half.

Twenty paired launches is a perfectly sound protocol and the finding stands.
Claiming thirty when the number is twenty is the defect, and it is the kind a
reviewer finds by opening one file.

**CLOSED 2026-11-20** the abstract and the contribution state the counts that
were used, and `scripts/check_claims_semantics.py` now reads the launch counts
out of every results file the manuscript draws on and fails if the manuscript
claims a uniform count higher than the lowest one actually used.

**A note on how this was nearly missed, and on a bad check.** The first
version of the trace asked whether any manuscript number appears only in
files cited by rows flagged PRELIMINARY PROTOCOL. It reported that 16.33 did,
which looked like a far worse defect: a preliminary number in the paper. That
was wrong. The run-length file is cited by seven rows, three of them
preliminary and four not, and 16.33 belongs to H67, which is not. The check
conflated file-level provenance with row-level provenance.

That conflation is itself worth recording: a single results file serving both
preliminary and launch-protocol rows means the file alone does not establish
which protocol produced a number. The row does. Any future traceability check
has to read the row.

The check written to prevent D10 then failed twice in testing before it
worked. Its first form took the lowest launch count mentioned anywhere under
`results/` and so fired on a pilot run described in prose. Its second form
used a negative lookbehind, which let the regular expression start matching
inside a number and report a uniform claim of "0 independent launches". Both
were caught by a regression test that asserts the check fires on the wrong
wording and passes on the right one. A gate that has never been shown to fail
is not a gate.

---

## D11. The only test of the kperf module ran on the wrong platform

**Found** 2026-11-20, from a one-test discrepancy between the suite on the
development machine and the suite on the M4: 101 tests against 100.
**Severity** a coverage hole on the platform that produces every measurement
in the paper, and a silent one, because both counts looked healthy.

`crates/titan-platform/src/kperf.rs` carried exactly one test, and it was
gated `#[cfg(not(target_os = "macos"))]`. It asserts that opening the
framework fails, which is only true where kperf does not exist. On the M4 the
test is compiled out, so the suite ran 100 tests there and never touched the
module at all. Every counter measurement in the paper comes through code the
test suite had never executed on the machine that produced it.

The discrepancy was visible in two terminal outputs and would have stayed
invisible to either one alone.

**CLOSED 2026-11-20** a macOS counterpart asserts the guarantees that hold
without privilege: the framework loads, the event database opens and reports a
name, the event list comes back populated with no empty names, and every
counter the experiments ask for resolves against the database under at least
one of its candidate spellings. Programming counters needs root and is not
attempted.

**Two errors inside this fix, both recorded because both are the shape of the
defect itself.**

The first version called `k.events(&[])` and `k.events(events::CYCLES)`.
`events()` takes no arguments. It compiled on the development machine because
the macOS branch is gated out, so the broken code would have failed only on
the M4. The signature was read, the test rewritten against it, and the macOS
branch then type-checked by temporarily flipping its `cfg` so the compiler
reads the body. That flip is now the standard check here: a gated branch that
no compiler in the build environment ever reads is not verified by a green
build.

The second version type-checked and still failed on the M4, and failed for a
better reason. It asserted that `kpc_cpu_string` returns a non-empty string
and `kpc_pmu_version` a non-zero version. In an unprivileged process on
26B131 neither is true: the database opens and names itself "Apple silicon",
while `kpc_cpu_string` returns empty and `kpc_pmu_version` returns 0. That is
a fact about privilege, not a fault, and it was asserted on an assumption
about a machine not available to the author of the assertion. Those two values
are now printed as context and never asserted.

That failure also hid what the test was written to collect: the panic fired
before the event count and the counter resolutions were reached, so one run
produced no usable information. The test now prints every value before it
asserts anything, so a failing run still reports the whole picture.

**What the module is now known to do unprivileged on 26B131.** kpep side
works: `kpep_db_create` opens and the database names itself. kperf side does
not: `kpc_cpu_string` empty, `kpc_pmu_version` 0, `kpc_force_all_ctrs_get`
reporting no permission. This belongs in the Method section, since it bounds
what the counter harness can report without root.

---

## D12. I reported the power provenance of the corpus and got it wrong

**Found** 2026-11-20, by making the power field uniform and then checking
every file instead of only the files I had looked at before.
**Severity** a false assurance given in writing about the provenance of the
measurements, and a real gap underneath it.

Earlier the same day I traced the manuscript's numbers to their results files
and reported that three files lacked a POWER line, all three legitimately,
and that every M4 file the manuscript draws on was confirmed wall power.

Both halves were wrong. Twenty-five of thirty-five results files had no power
field at all, because the run script only began capturing the power state on
2026-11-17 and everything before that date is silent. And three numbers the
manuscript quotes rest solely on files with no recorded power state: 16.33
and the pair 0.808 and 3.057. On a fanless machine the power state bounds the
sustained clock, so this is provenance that matters rather than bookkeeping.

The earlier check reached the opposite conclusion because it examined only
the files it had already decided were interesting, which is not a check.

**CLOSED 2026-11-20** every results file now states a power condition and
none of them is guessed: a measured state where the harness captured one,
"not applicable" with a reason for the two virtualized x86-64 hosts and the
pure-computation calibration, and "not recorded" with a reason for the
twenty-five that predate the field. `scripts/check_power_lines.py` fails if
any file is silent or states an absence without a reason, and reports the
counts so the proportion is visible rather than implied. The manuscript now
says beside the run-length result that the power state of that run was not
recorded.

**What is still not known.** Whether those runs were on wall power is a fact
about the measuring machine on days already past. It is not recoverable from
the repository and it will not be guessed. If the author can attest to it,
that attestation goes in the threats section labelled as an attestation, not
as a measurement.

---

## D13. Novelty was asserted from memory and never searched for

**Found** 2026-11-20, by running the literature search that should have
preceded the first draft of the abstract.
**Severity** the most dangerous class in this register. Every other entry is a
number that was wrong. This one is a claim of priority, and a reviewer who
knows the field would have rejected the paper over it rather than asking for a
revision.

Until today no search had been run. The abstract's novelty framing rested on
recollection, and three parts of it do not survive contact with the record.

**1. The run-length hazard is published.** The abstract said "we also report a
measurement hazard specific to this machine class". Emurian, Raghavan, Shao,
Rosen, Papaefthymiou, Pipe, Wenisch and Martin reported it in 2014: a chip that
boosts and then throttles makes a short run faster than a long one, so
extrapolating from a short run overestimates sustained performance. They bound
it at about 8 percent on a server-class part and at up to 5.8 percent error on
a nominal two times speedup.

What is ours is sharper than what was claimed. The same hazard on a fanless M4
is 16.33 times, two orders of magnitude beyond their bound, and Emurian et al.
explicitly do not report any case where it reverses a ranking: their findings
concern inflated margins. Ours reverses the ranking, so the cheapest ordering
at 200,000 messages is the dearest at 10,000,000. Grounded against their result
the claim is stronger and defensible; asserted as a discovery it was neither.
The abstract now cites them and states both numbers.

**2. The Fano and cv squared distinction is textbook, not a finding.** For a
renewal process the Fano factor tends to the squared coefficient of variation of
the gaps as the counting window grows, and at finite windows it is bounded by
one plus twice that quantity. A Hawkes process is not renewal, which is exactly
why the two diverge here, and the divergence under self-excitation is itself
described in the literature: the Fano factor keeps growing with the window while
cv squared is a fixed property of the gap distribution.

So H74 does not discover that the two statistics differ. It measures what
choosing the wrong one costs on a real pipeline: 13.5030 times where the Fano
account predicts 1.040. That is the contribution, and it is an empirical one.
The theoretical sharpening belongs beside it: a statistic that grows without
bound with the observation window cannot serve in a queueing bound, which is why
Kingman consumes the other one.

**3. This machine class has been characterised before.** There is a published
evaluation of the M-series for high-performance computing using STREAM, GEMM and
powermetrics, and there are open tools measuring Apple Silicon memory bandwidth,
cache latency and even two-thread cache-line handoff. The sentence "nobody has
published measurements of this machine" is false and would have been checked in
one search by any reviewer.

What no search found is the specific thing this paper does: a comparison of
memory-ordering constructs with full tail distributions, under a pre-registered
protocol with independent launches, on Apple Silicon. The nearest work measures
Apple's total store order mode against native Arm mode across application
workloads and states that it leaves atomics and barriers to future work.

**4. One recent paper is complementary and is now cited.** A September 2026
measurement study characterises a real CME feed across 281 sessions, finds
self-exciting arrival clusters, and then simulates a pipeline using a per-hop
cost taken from the author's earlier work. It assumes the quantity this paper
measures, and this paper holds fixed the arrival process that one measures. The
two fit together, and saying so is better positioning than claiming to be alone.

**CLOSED 2026-11-20** the abstract states the prior bound and cites it, two
verified references are added, and the novelty claim is now the ranking reversal
and the magnitude rather than the phenomenon.

**A gate that could not do what I wanted it to do.** While fixing this I found
that check_provenance.py never verified the two figures quoted from Emurian et
al., because its pattern requires two decimals. The instinct was to widen the
pattern. That is wrong: "8" and "3" each appear in thirty-five results files by
coincidence, so a substring search on a short number always succeeds and would
have reported false confidence. Every percentage in the manuscript is now
declared with a source that must exist, and the check says plainly that it does
not verify the value. A check that cannot establish something should say so
instead of passing.

**The standing correction.** No novelty claim enters this paper again without a
search behind it, and the searches belong in the record rather than in my
recollection.

---

## D14. A registered threshold measured the wrong thing

**Found** 2026-11-21, by the threshold failing and the failure not meaning what
it was written to mean.
**Severity** a fault in the design of a pre-registration, which is the part of
this project that is supposed to be above reproach. It did not corrupt a result,
and it is recorded because the registration is the method.

H77 part (d) required the spread of `core_ghz` across the five orders, highest
over lowest, to be at or below 1.15 within each caching state. Its purpose was
to establish that comparing two arms in nanoseconds is fair, because a
nanosecond ratio is only a work ratio if the arms ran at similar clocks.

It came back 1.1922 contended and 1.1448 resident, so it failed. But look at
where the spread is. Contended, the extremes are the unordered cost floor at
3.691 GHz and `acqrel` at 4.401 GHz, and the cost floor spends 10.198 cycles per
message against `fencedstore`'s 83.579. An arm doing an order of magnitude less
work per message does not hold the core at the same frequency, and it is already
reported only as a cost floor because it fails its checksum in every epoch.
Between the two arms that actually form the comparison the frequency ratio is
1.0086 contended and 1.0611 resident.

So the threshold was written over all five arms when the claim it protects
concerns a pair. It failed for a reason that is real but is not the reason it was
asking about. A threshold that can fail without its failure meaning what it
says is a badly specified threshold, and specifying it was my work.

**CLOSED 2026-11-21** the failure is reported as a failure, because a
registration that gets reinterpreted after the data arrives is worth nothing.
The registered consequence fires as written and the paper now reports cycle
ratios. Where the spread comes from is recorded beside the verdict so a reader
can see what failed and what did not. Any future frequency check is specified
over the arms being compared, not over every arm in the run.

**It caught something anyway, which is the part worth keeping.** The resident
figure moves from 2.5169 in nanoseconds to 2.7008 in cycles, 7.3 percent, and
the cycle figure agrees with the 2.75 the original PMU run published where H76's
nanosecond replication did not. A frequency confound of 6.1 percent between the
two compared arms, which the five-arm threshold did not isolate, is what sat
between H76 and the original. The paper is on firmer ground for having moved to
cycles, and the badly specified threshold is the reason it did.

---

## D15. The build script had never run on the platform the paper is written on

**Found** 2026-11-21, by installing a TeX toolchain on the M4 for the first time
and watching the build print an error it had never printed anywhere else.
**Severity** no wrong number reached the paper, and the guard that protects the
output would have caught it if it had mattered. It is recorded because of what
it says about where this project verifies things.

`docs/paper/build.sh` rewrites two dash ligatures that IEEEtran's bibliography
style generates into `main.bbl`. The first substitution is written
`sed -i.orig '...'`. The second is written `sed -i '...'` with no suffix.

GNU sed accepts both. BSD sed, which is what macOS ships, requires an argument
to `-i`, so it reads the substitution expression as the backup suffix and then
tries to execute `main.bbl` as its script, which it reports as `invalid command
code m`. The second substitution has therefore never run on the machine this
paper is measured on, and every build there printed an error.

Nothing was corrupted. The `\IEEEtranBSTCTL` entry in `refs.bib` sets
`CTLdash_repeated_names` to `"no"`, so the string that substitution exists to
remove is not generated: the count in the committed `main.bbl` is zero. And the
guard immediately after it fails the build if any double-hyphen survives, which
a six-hyphen ligature contains, so a real occurrence would have stopped the
build rather than reaching the PDF.

**CLOSED 2026-11-21** both substitutions are written without `-i`, through a
temporary file, which behaves identically on BSD and GNU sed. The dead
substitution is kept and kept working rather than deleted, so that removing the
control entry cannot silently reintroduce a dash. Verified by rebuilding and
confirming that bibtex plus the rewrite reproduce the committed `main.bbl`
unchanged, with no error printed.

Also fixed in the same pass: the script now checks for `pcrb7t`, `pcrr7t` and
`phvb7t` before running a pass, because IEEEtran sets its monospace in Courier
and its headings in Helvetica and BasicTeX carries Times alone. Without them
pass 1 dies on a missing TFM, which reads as a corrupt document rather than a
missing font. The message now names the exact `tlmgr` command.

**THE PATTERN THIS IS THE THIRD INSTANCE OF, AND IT IS THE ONE TO FIX.** D7 was
a figure-determinism check that passed on the development machine and failed on
the M4. D11 was a test that existed only off macOS, so the module every counter
measurement passes through was never exercised on the machine that produced
them. D15 is a build script whose macOS path had never executed. Each time the
tooling was verified on the machine that is not the ground truth, and each time
the fault was invisible to a green build.

The standing correction is narrow and checkable: a tool that will run on the
measuring machine is not verified until it has run there. Where that cannot be
done from the development side, the delivery script carries the check and
reports it, as t72 did for kperf and as t79 now does for the build.

---

## D16. A validation ran, returned a negative answer, and nobody read it

**Found** 2026-11-21, by committing the platform probe's output and then reading
it instead of skimming it.
**Severity** a stated capability of the measurement apparatus that the apparatus
itself reports it does not have. No published number is wrong, and the Method
section would have been wrong without this.

`syscall_test` in `titan-probe` exists to establish whether the user-only count
mode excludes kernel work. Its own comment gives the logic: with kernel counting
a `getppid` call costs hundreds of instructions, and a filter that truly excludes
kernel mode leaves only the user-side trap stub, a handful.

Measured on this part, over 100,000 calls, three trials per mode, with the modes
alternated so run-order drift cannot masquerade as a mode effect:

    mode=All       1110.2, 1110.1, 1110.2 instructions per call
    mode=UserOnly  1110.2, 1110.1, 1110.2 instructions per call

By the test's own logic the filter is not excluding kernel mode. The numbers are
identical to one part in ten thousand across modes. The test had been running
and passing visual inspection for days because its output was read as "the
counters work" rather than as the comparison it was written to make.

A plausible mechanism is that `FIXED_CYCLES` and `FIXED_INSTRUCTIONS` are
fixed-class counters, and the per-event flag that carries the mode reaches
`kpep_config_add_event` but the code only calls `kpc_set_config` when the
configurable class is present. That is a hypothesis and the probe does not test
it, so it is not asserted.

A second line of evidence sits in the same file and agrees. The known-length
instruction chain expects exactly 1,020,000,000 instructions and the counter
reports between 1,020,167,770 and 1,020,323,076 across twenty trials, an excess
of about two hundred thousand instructions, which is the kind of residue that
interrupts and involuntary switches leave and which a user-only count would not
carry.

**CLOSED 2026-11-21** the Method section states that every counter figure in
this paper includes kernel work done on the thread's behalf, because that is
what the apparatus was measured to do rather than what its mode flag claims.
The limitation is bounded and already instrumented: the hot loop performs no
system call, and every `bd` record carries `invol_switches` so a reader can see
how much involuntary kernel activity each measurement absorbed. The count mode
is reported as untested rather than as working, and the probe output is
committed so the next reader sees the comparison rather than taking the comment
on trust.

**The pattern.** This is the second time a check produced the right output and
the wrong conclusion was drawn from it, after D8, where every mechanical gate
passed while the paper compared the wrong statistic. A test that runs is not a
test that was read.

---

## D17. I reintroduced D7, four days after closing it

**Found** 2026-11-21, by the new gate failing on the M4 while passing here.
**Severity** no wrong number reached the paper. What it damaged is the claim
this register exists to support, that a defect recorded here stays fixed.

D7 established that the manuscript figures are not byte-reproducible across
machines. The same script on the same committed data produced files 19 to 121
bytes smaller on the M4 than on the development machine, consistently, across
all five figures. Its closure says so in as many words: cross-platform
byte-identity is not achievable, and the first version of that check was wrong
for demanding it. The property that is achievable is same-machine determinism,
and `scripts/check_figures_deterministic.py` was written to test exactly that,
by regenerating twice on one machine and comparing those two runs to each
other.

Four days later I wrote `check_paper_figures.py` and had it compare a fresh
regeneration against the committed bytes. That is cross-machine byte-identity.
It passed on the machine that committed the figures and failed on the machine
that measures them, with all seven figures reported modified. The delivery
script carried a second copy of the same mistake, failing on a dirty working
tree after regeneration.

The lesson had been written down, in this file, by me, and I did not read it
before writing a gate in the same area.

**CLOSED 2026-11-21** the figure gate now checks currency rather than bytes: it
regenerates into a backed-up tree and compares extracted text, so a figure that
has fallen behind the records it draws fails, while a stream-compression
difference between two machines cannot. Determinism remains owned by the gate
that already tested it correctly, and the two no longer overlap. The working
tree is restored whatever happens, so running the gate never leaves a modified
figure behind. Proven both ways: it fails when a committed figure no longer
matches a regeneration, and it passes on a tree where only the bytes could
differ.

**What changes in how I work.** Before writing a check in an area this register
already covers, I read the entry that covers it. A register nobody rereads is a
diary, not a control.

---

## D18. The paper named an instruction the compiler does not emit on both paths

**Found** 2026-11-21, by reading the instruction census output instead of
assuming what the census would say.
**Severity** the paper's central claim is about the store side, and it named
the wrong store instruction for half the measured path. A reviewer with
`otool` finds it in a minute.

The manuscript said the acquire-release arm uses `stlr`, twice, and named no
acquire-side instruction at all. The census over the compiled library says
otherwise:

    titan_census_push_acqrel        ldapur=1  stlr=1
    titan_census_pop_acqrel         ldapr=1   stlur=1
    titan_census_push_fencedstore   ldapur=1  dmb=1  str=3
    titan_census_pop_fencedstore    ldapr=1   dmb=1  str=2

The producer path uses `stlr` and the consumer path uses `stlur`, the unscaled
release store. The acquire side is `ldapur` on one path and `ldapr` on the
other. Both are RCpc forms and neither path emits `ldar`, which the paper also
never said. The scaled and unscaled encodings are different instructions, and
the claim under examination is specifically about the store side, so calling
both of them `stlr` is not shorthand, it is wrong.

**CLOSED 2026-11-21** the abstract names both store forms and says which path
each belongs to, the contribution says "the release store" rather than naming
one, and the Method states the full census for all four arms along with the
fact that substituting the store side adds exactly one barrier and one store to
each path. The census output is what established this, and it is the mechanism
the repository already had for exactly this purpose.

**Why it survived.** The census script existed and was never read against the
manuscript. That is the same shape as D16, where a validation ran and its
negative answer went unread, and as D13, where novelty was asserted without the
search that would have checked it. The repository keeps producing the evidence
that would have caught these, and the gap is between producing it and reading
it.

---

## D19. A registered counter returned zero, and the verdict script divided by it

**Found** 2026-11-21, by the H78 run.
**Severity** one registered threshold cannot be evaluated at all, and the
script meant to report that crashed instead of saying so.

H78 part (a) required the fenced-store arm's retired store misses to be at
least 0.95 times the acquire-release arm's, contended. Its purpose was to
exclude the throttling account, which needs the faster arm to take strictly
fewer coherence transactions.

`L1D_CACHE_MISS_ST_NONSPEC` returned exactly 0.0000 in all 900 records, one
distinct value across every arm and both caching states. The ratio is 0/0.
Part (a) is therefore unevaluable: not supported, not refuted, unmeasured. The
throttling account is not excluded by this run and the paper must say so.

The event resolved against the database, which E5 recorded as carrying it, and
the session programmed without error, so the failure is silent. Whether the
event is inert on this part, whether a retired-store-miss event needs a counter
class this configuration does not request, or whether the ring genuinely takes
no retired store misses even while bouncing a line between cores, is not
knowable from the repository.

**CLOSED 2026-11-21** the verdict for (a) is recorded as unevaluable with the
reason, the paper will not claim the throttling account is excluded, and
`titan-probe` gains a store-miss diagnostic that drives a loop guaranteed to
miss L1D on stores and reports what the counter says. If the counter reads zero
there too, the event is inert on this part and that is a fact about the machine
worth stating; if it reads non-zero, the fault is in how the session programmed
it and H78 (a) can be rerun.

**The second half, which is mine.** `scripts/h78` summary divided by the
store-miss median without guarding it, so the run ended on a
`ZeroDivisionError` after the contended table and before the resident one. The
data was committed and pushed correctly, so nothing was lost, but the operator
saw a traceback instead of a verdict. Every ratio in the verdict scripts now
reports "unevaluable, denominator is zero" rather than raising. A measurement
apparatus that crashes when a counter reads zero cannot report the one thing a
zero counter means.

---

## D20. A registered threshold measured the wrong quantity, for the second time

**Found** 2026-11-21, by H78 part (c) failing.
**Severity** the refutation is correct and stands. What is wrong is that the
threshold tested something other than the claim it was written for.

Part (c) required the resident waiting ratio between the two arms to lie
between 0.80 and 1.25, on the reasoning that the resident penalty is barrier
latency rather than waiting, so waiting should be about equal. Measured, the
fenced-store arm waits 2.0522 times the acquire-release arm resident, and (c)
is refuted.

The claim was about the composition of the penalty and the threshold was
written over an absolute quantity. The fenced-store arm's total resident cost
is 22.495 cycles against 8.032, so even an unchanged waiting fraction would
raise absolute waiting well outside the band. The quantity that matches the
claim is the non-waiting remainder, which is 3.822 against 13.855, a difference
of 10.033 cycles per message. That is visible in the data and is recorded in
the row as a post-hoc observation, explicitly not as a result, because it was
not registered and fitting a threshold to data already seen is the practice
this protocol exists to prevent.

**CLOSED 2026-11-21** (c) is reported as refuted without reinterpretation. Any
claim about the composition of the resident penalty requires its own
registration with the non-waiting remainder as the stated quantity.

**This is D14 again.** There the frequency-spread threshold was written over
all five arms when the claim concerned a pair. Here a composition claim was
written over an absolute. Both times the threshold was checkable and both times
it did not check the claim. The correction is specific: before a threshold is
committed, state in one sentence what would make the claim false, and confirm
the threshold is a test of that sentence rather than of a quantity that merely
moves with it.

---

## D21. The harness could not tell a dead counter from a real zero

**Found** 2026-11-21, by the counter validity check written for D19.
**Severity** structural. One registered threshold was lost, and nothing in the
measurement path would have reported it.

The diagnostic settles the question D19 left open. `L1D_CACHE_MISS_ST_NONSPEC`
counts on this part, and counts correctly: 0.9446 retired store misses per
store over a 64 MiB buffer where close to every store must miss, 0.0003 for
the same count of stores into 4 KiB where close to none should, and 0.9930 in
a read modify write stream over 64 MiB. The expected shape of all three was
fixed in the source before the run. So the event is not inert, and the H78
zero is a property of the apparatus, not of the machine.

Which part of the apparatus is not yet established, and two causes fit the
evidence equally well.

A. The event cannot be allocated alongside `FIXED_INSTRUCTIONS` and
`LDST_UNIT_OLD_L1D_CACHE_MISS`, which is the set the barrier harness requests
and which differs from the set the diagnostic used. A silent allocation
failure reads zero.

B. The barrier harness reads its counters on the consumer thread, and counting
is per thread. The arms under test differ in the producer's store instruction:
the census shows `push_acqrel` emitting `stlr` and `pop_acqrel` emitting
`stlur`. If the consumer's stores land in a line it already owns, it retires no
store misses whatever the producer does, and part (a) was measuring the wrong
thread from the moment it was written.

B would be the more serious finding, because it would mean the threshold was
not merely unmeasured but unmeasurable as specified, and that a correct
version of (a) has to read the producer's counters. The isolation pass added to
`titan-probe` repeats the three known passes under the barrier's exact event
set. Zero there means A. Counting there refutes A and leaves B.

**The reporting gap.** `kpep_config_add_event` reports a reason through an out
parameter as well as through its return value, and this code created the
variable, passed its address, and discarded it. The reason is now recorded per
event and exposed through `Session::add_errs`, printed by the probe. It is
recorded and not enforced on, because the meaning of a non zero reason
alongside a successful return is not established here, and failing a run on an
unverified condition would be a worse fault than the one being repaired.

**Cause A is refuted, by the isolation run.** Under the barrier harness's
exact event set, `[FIXED_CYCLES, FIXED_INSTRUCTIONS, L1D_CACHE_MISS_ST_NONSPEC,
LDST_UNIT_OLD_L1D_CACHE_MISS]`, the store miss counter read 0.8501 per store
over 64 MiB, 0.0002 into 4 KiB and 0.9944 in the read modify write stream, and
every configuration reason code came back zero for every event in both sets.
The event allocates and counts alongside the fixed instruction counter. So the
event set is not at fault and cause B stands, which is recorded as D22.

**The isolation run reached the right conclusion by the wrong route, and that
is worth recording.** The diagnostic took the store miss column as index one
for both event sets. Index one is the store miss event in the probe's set and
the fixed instruction counter in the barrier's, so the barrier columns printed
4.0242, 6.0242 and 5.0242 under a store miss heading: instruction counts.
The script's own verdict line tested that mislabelled column for zero, found
it non zero, and announced the correct answer for a reason that had nothing to
do with the counter. What caught it was the prediction, not the check: the
4 KiB pass, which must barely miss at all, printed higher than the 64 MiB
pass, which must miss on nearly every store. Columns are now labelled with the
event names the session reports rather than by position, so the reading cannot
be wrong without the names being wrong too.

**CLOSED 2026-11-21** `scripts/check_counter_sanity.py` fails when a counter
derived field holds one distinct value across a whole results file, which is
the signature of a counter that was never programmed, with known exceptions
listed against the defect that explains them and removed when the measurement
is redone. The reason codes are recorded. The isolation pass is in place.

**Two faults in the gate itself, both found by reading its output rather than
its exit status.** It first read only lines beginning with `bd `, a convention
of one harness and not of the data, and so examined 10 counter fields when
the results directory holds 47. It then carried a floor of thirty records that
silently excluded ten of the thirteen files carrying counters. With both
repaired it checks 47 fields across 11 files, names the 3 files below the floor
instead of passing over them, and every one of them varies. The blast radius
of D19 is therefore one field in one file, verified rather than assumed: no
earlier run lost a counter.

---

## D22. Part (a) read the thread that does not execute the instruction

**Found** 2026-11-21, by the isolation run refuting the alternative.
**Severity** a registered threshold that could not have been evaluated as
written, on any machine, for a reason that has nothing to do with the counter.

Counting is per thread, and `Session::read` reports the counters of whichever
thread calls it. In `titan-barrier` that thread runs the consumer loop, so
every counter field this harness has ever committed describes the consumer.

The arms under test differ in the producer's store instruction. The committed
census has `push_acqrel` emitting `stlr` and `pop_acqrel` emitting `stlur`.
H78 part (a) asked whether the faster arm retires fewer store misses, which is
a question about the side executing the release store, and read the answer
from the other side. The consumer's stores land in a line it already holds, so
it retires no store misses whatever the producer does, and 0.0000 across 900
records is a correct measurement of the wrong quantity.

This is a worse fault than D19 appeared to be. A counter that fails to program
is an accident of configuration. A threshold written over the wrong thread was
never going to answer its question, and no amount of rerunning the same
harness would have revealed it: the number would have come back zero every
time, consistently, and consistency reads as reliability.

The three faults have one shape. D14 wrote a threshold over five arms when the
claim concerned a pair. D20 wrote one over an absolute when the claim
concerned a composition. This one wrote one over the wrong thread. In each
case the threshold was precise, checkable, fixed in advance, and not a test of
the claim. Precision is not validity, and pre-registration protects against
moving the goalposts, not against aiming at the wrong goal.

**CLOSED 2026-11-21** `titan-barrier` gains a `prodside` mode that spawns the
consumer and keeps the producer on the measuring thread, so the counter reads
and the timing window sit around the send loop. The ring, the arms, the epoch
structure and the record format are unchanged and the default path is
untouched, so the earlier runs stay comparable to each other. Whether they
stay comparable to this one is registered rather than assumed: a smoke test on
both paths shows the timing window does move, so H79 fixes the comparability
check before the run and states what follows if it fails. The file level
config line now records `measured_side`, so no producer side record can be
mistaken for a consumer one.

---

## D23. A number can be accounted for by coincidence

**Found** 2026-11-21, while adding the thread scope disclosure to the method
section.
**Severity** the provenance gate, which exists so that every figure in the
manuscript traces to a committed record, can pass a figure whose real source
is not committed at all.

The gate accounts for a number when that number appears somewhere under
`results/`. For most figures that is the right rule. For a figure tied to one
particular run it is the wrong rule, because a four decimal value matches an
unrelated file often enough to matter. It did immediately: 0.8501, written into
the method section from the counter allocation diagnostic, resolved against a
record in `2026-11-14_m4.txt` and the gate reported zero numbers unaccounted
for while the diagnostic itself was uncommitted.

The gate was not broken. A deliberately fabricated value, 0.7431, failed it.
The hole is narrower and more dangerous than a broken check: it passes the
cases that look like the cases it was built for.

**A second fault, found in the same minutes and worse.** Those two figures came
from a diagnostic that this session is about to rerun with corrected column
labels, and a counter diagnostic does not repeat to four decimal places. Had
they stayed, the manuscript would have frozen a number that the rerun
contradicts, with a gate that passed it. The method section now states the
shape of that result in words, that the event misses on close to every store
over a buffer far larger than L1D and on close to none over one that fits,
which is the claim the diagnostic actually supports.

**CLOSED 2026-11-21** `scripts/check_provenance.py` carries a `CITED` map from
a run specific number to the file that must contain it, checked in addition to
the corpus wide rule, so such a number is verified against its own source
rather than against any source. The map is empty, which is the correct state:
an entry is added when the run holding the number is committed, never before.

**The tooling fault underneath this.** The edit that first added `CITED`
reported success and changed nothing. It matched on `PERCENT = {` while the
declaration reads `PERCENT: dict[str, tuple[str, str]] = {`, replaced nothing,
and printed its confirmation anyway, after which two further edits were
written against text that was not in the file. Every edit in this session now
asserts its anchor before replacing and reads the file back afterwards. The
pattern is the one the register keeps recording in the measurements, arriving
in the instrument used to write the measurements down: a step that reports
success without checking the thing it claims to have done.

---

## D24. The cost floor's incorrectness is a race, not a property

**Found** 2026-11-22, by H79 part (e) failing.
**Severity** a correctness threshold that has held across every run of this
project turns out to have been measuring the thread arrangement.

The unordered arm exists as a cost floor. It has no ordering and is expected
to produce a wrong sum, and (e) has required `sum_ok` to be 0 of 180 for it in
every registered run, which it has been. Measured on the producer thread it is
1 in 137 of 180 records.

Nothing about the arm changed. The consumer is spawned in this mode instead of
the producer, the interleaving changes, and a race that had always resolved
one way now resolves the other way three times in four. The arm is exactly as
incorrect as it was.

The threshold was written as a property of the arm. It is a property of the
arm and the schedule together, and the earlier runs did not reveal that
because they all shared one schedule. A test that passes consistently under
the only arrangement ever tried is not evidence of a stable property, and the
consistency is what made it look like one.

**CLOSED 2026-11-22** (e) is recorded as refuted. The registered meaning of
the unordered arm narrows to what it can support: in a given thread
arrangement it either does or does not produce correct sums, and the figure is
reported per run rather than asserted as 0. The paper gains the observation
rather than hiding it, because it is worth stating plainly: a ring with no
ordering at all passed its own correctness check in 76 percent of records
once the threads swapped roles, which is why the correctness of a memory
ordering implementation cannot be established by running it.

---

## D25. The event cannot answer the question, on either thread

**Found** 2026-11-22, by H79 part (a) returning 0/0 after the thread was
corrected.
**Severity** terminal for the threshold as written. This is not a measurement
to retry.

D22 established that H78 part (a) read the consumer while the arms differ in
the producer's store, and H79 moved the counters to the producer. Every arm
read exactly 0.0000 retired store misses contended there as well. Resident,
two arms read 0.0002 and the rest read zero.

The explanation is in what the event counts. `L1D_CACHE_MISS_ST_NONSPEC`
counts retired stores that missed in the first level data cache, and a miss
means the line was absent. The ring's stores go to slots cycled through a
1024 entry buffer and to a cursor the thread writes constantly, so the lines
are present. A store to a present line held without write permission needs
ownership, not a fill, and an upgrade is not a miss by this counter's
definition. The quantity part (a) was built on does not occur in this
workload on either side of the ring.

So (a) is closed as unmeasurable with this event rather than left pending.
Rerunning it on a third arrangement would return the same zero for the same
reason. **The consequence for the paper is stated rather than worked around:
the throttling account is not excluded by store miss evidence, and the paper
will not claim that it is.** What the paper does have on that point is the
waiting decomposition, which is strong and now replicated on both threads at
0.2338 and 0.2161, and the consumer side load miss counts, which are reported
with their scope named.

**The guard that was supposed to catch this passed.** Part (b) was written
after D19 precisely so that a zero counter would be recorded as a failed
measurement rather than mistaken for a result, and it required the median
store miss to be strictly above zero for at least one arm in at least one
caching state. It passed, at 0.0002, from two resident arms, while every
contended arm, which is the only cell part (a) uses, read exactly zero. The
guard was scoped more widely than the thing it guards, so it reported
measurable for a comparison that was not measurable.

That is the fourth time in this project that a threshold was precise,
checkable, fixed in advance and scoped to the wrong thing: D14 over five arms
instead of a pair, D20 over an absolute instead of a composition, D22 over the
wrong thread, and now a measurability guard over all cells instead of the
cells under test. The correction is narrow and mechanical, and it is the only
one that has worked: a threshold must name the exact cells its claim is
evaluated on, and a guard must be evaluated on exactly the cells it guards.

**CLOSED 2026-11-22** (a) is retired as unmeasurable with this event, with the
reason recorded. `titan-probe events` dumps the full event database to a
committed file so that any replacement counter is chosen against what the
hardware exposes rather than from memory, which is how the first one was
chosen. No new hypothesis is registered on this question until that file
exists and a candidate event's definition has been read.

---

## D26. The register asserted power states that were never recorded

**Found** 2026-11-22, by a check written minutes earlier for my own error.
**Severity** four rows of the register stated a measurement condition as
though it had been measured. One of them was one commit from the paper.

The H79 row was written as "on wall power" without the file being read. The
operator then reported having run it without the charger. Nothing had checked,
because nothing could: the register's power claims were prose and the files'
POWER lines were data, and the two had never been compared.

Comparing them found three more, all older than the H79 row. The register said
H64 re-measured a case "on wall power" in four separate rows; H64's run,
`results/2026-11-16_m4_crosscore.txt`, records no power state. It said the H17
retest ran on wall power; both runs it cites record none, and the `_ac` in one
filename is a naming convention, not a field. It reported H66's clocks as
4.045 to 4.062 GHz on AC against 4.044 to 4.049 on battery; the clocks are
measured and the labels were not recorded anywhere.

The cause is dated. Power capture began on 2026-11-17, and 25 of the 41 result
files predate it and say so honestly in their own headers. The claims about
those runs came from the operator's memory of the day and were typed into the
register in the same voice as the measured figures, where nothing
distinguished them.

This is D12's successor and the pair is worth reading together. D12 found that
the files did not record their power state. D26 finds the register asserting
power states for exactly those files. Fixing the first created the conditions
for the second: once most runs carried a real POWER line, a row asserting one
looked like it had been checked.

**CLOSED 2026-11-22** All four claims are corrected to say what is recorded
and what is not, in every row they appear in, including the two the check did
not flag, because the check verifies that some run a row cites records the
claimed state and not that the run the sentence is about does. That limit is
written into the check itself rather than left for someone to discover. Three
register power claims now verify against a cited run, and the H79 row asserts
no power state until its file is read.

---

## D27. A refutation resting on a counter never seen to move

**Found** 2026-11-22, immediately after H80 and H81.
**Severity** one registered part is refuted on the strength of a zero, and
nothing has shown the event producing that zero can read anything else.

`ST_MEM_ORDER_VIOL_LD_NONSPEC` read exactly 0.0000 on every arm of 1800
records across both threads, so part (c) of each run is refuted and part (d)
is unevaluable. A plain reading says this ring triggers no memory order
violations, which is plausible: the producer stores a slot and then a cursor,
and the loads that follow read a remote cursor, so a load rarely aliases an
older store in the same thread.

Plausible is not measured. This is the D19 mistake with a different event, and
D19 cost two runs.

**The gate written for exactly this did not look.** `check_counter_sanity`
exists to fail when a counter derived field holds one value across a whole
file. Its list of fields was explicit and had been typed before
`msg_mem_order_viol` existed, so the field was invisible to it: the regular
expression saw `msg_cycles`, `msg_ldst_wait_cycles` and `core_ghz` in those
records and nothing else. A check whose coverage must be extended by hand each
time the thing it checks grows is a check that silently stops covering it. The
field pattern now matches any `msg_` field by shape, and the two files are
listed as pending against this entry, reported on every run, so no claim can
quietly rest on them.

**CLOSED 2026-11-22, against the reading that was convenient.** The validation
pass ran and the counter did not move. The aliasing loop, built to trigger
violations, reported 0.000000 per operation, and so did the control. The two
passes did behave differently, writebacks 0.0005 against 0.0001, so the loops
were not identical, and cycles per operation were 21.64 against 21.50, so
neither pass paid a replay penalty.

Two explanations survive and this project cannot separate them: the event is
inert on this part, or the aliasing loop produced no violations because store
to load forwarding resolved every collision without one. Either way the zero in
H80, H81 and H82 is not evidence that the ring triggers no memory order
violations.

**So part (c) of H80 and of H81 is downgraded from REFUTED to UNEVALUABLE.** It
had been recorded as a refutation resting on this counter, and a refutation
resting on a counter never seen to move is not a refutation. The paper makes no
claim about memory order violations in either direction, and the mechanism is
neither tested nor excluded.

The line stops here rather than continuing. Two attempts have failed to move
this counter and a third would spend the evening chasing an instrument while
twelve points of submission mechanics and twelve of prose sit untouched. The
violation counter is retired from the harness: `violprobe` remains only so the
three existing files can be read, and a new `trafficprobe` mode replaces it
with `ARM_L1D_CACHE_REFILL`, the inbound half of the same coherence traffic,
which does count.

The validation pass is kept, with its expected outcome still fixed in the
source before the counter is read: an aliasing loop whose store
address depends on a pointer chase, so it resolves late and the following load
issues first, with the load's address colliding with the store's on an
unpredictable half of iterations so the disambiguation predictor cannot learn
to hold it back; and a control loop with the collision removed. If the
aliasing pass counts and the control does not, the event discriminates and the
ring's zero is a measurement, and part (c) stands refuted. If both read zero,
the event is inert here and parts (c) and (d) become unevaluable, like H78 (a)
before them. This entry closes when that pass has run, in either direction.

---

## D28. A per message rate whose numerator is driven by duration

**Found** 2026-11-22, by checking H80 and H81 against each other rather than
against their thresholds.
**Severity** the registered consequence of the central threshold does not
follow, and it would have entered the paper as a mechanism.

Part (b) compared writebacks per message between the two arms, on the
reasoning that the throttling account requires the faster arm to do strictly
less coherence work. It was refuted at 0.5004 on the producer and 0.4734 on
the consumer, unanimously across thirty launches, and the registered
consequence said the account survives.

It does not follow. Writebacks per message correlate with cycles per message
at r=0.9966 across the ten producer cells and r=0.9879 across the ten consumer
cells, and at 0.9954 and 0.9951 inside the contended cells alone. The ring
waits by spinning on a remote cursor, so a slower arm polls more times and
moves the line more often. Per message coherence traffic is very largely a
function of how long the message took, and a ratio of it between a fast arm
and a slow one measures the duration difference a second time.

The other direction is worth stating because it points the opposite way. Per
cycle the faster arm does more coherence work, 1.2821 on the producer and
1.2162 on the consumer contended, and less when resident, 0.7184 and 0.6165.
That is the shape of traffic set by real work and diluted by stall cycles, not
of traffic causing the stalls.

So the throttling account is neither supported nor refuted by these runs, and
the paper will say that rather than either of the stronger things.

This is the fifth threshold in this project that was precise, checkable, fixed
in advance and not a test of its claim, after a pair read as five arms, a
composition read as an absolute, the wrong thread, and a guard wider than the
comparison it guarded. The new failure mode is distinct and worth naming: the
quantity was normalised by the very thing under comparison, so the comparison
appeared in the answer twice.

**CLOSED 2026-11-22** H82 is registered to discriminate them, holding the arm
fixed and varying only the wait strategy, which moves duration without
touching the instruction under test. If both arms lie on one duration traffic
line, the arm adds nothing beyond its duration and the account is dead. If the
arms sit on separate lines, there is arm specific traffic and the account has
something to stand on. `titan-barrier waitsweep` provides the eight cells, and
the correctness of all eight was checked before the thresholds were written.

**A tooling fault alongside it.** The verdict script printed "2 supported"
while listing three, because one verdict was built as `worst <= 1.10 and det`,
which returns the list rather than a boolean, and the tally tested identity
against `True`. Six parts were listed and five counted. The expression is a
boolean now and the tally asserts that it accounts for every part, so a
summary can no longer disagree with the lines above it.

---

## D29. A duration lever that acted on one arm only

**Found** 2026-11-22, by H82's own guard refusing the regression it gated.
**Severity** the discriminating experiment could not be evaluated, which is the
guard working, and the design error was mine.

H82 varied the wait strategy to move duration while holding the ordering
instruction fixed, so that coherence traffic could be regressed against time
and the arms compared on one line. Across spin, isb, yield and wfe the
acquire-release arm's cost spanned 1.3295, above the registered floor of 1.25,
and the fenced-store arm's spanned 1.1137, below it. Parts (c) and (d) are
unevaluable.

The reason is obvious in hindsight and should have been obvious in advance. The
wait strategy determines what a thread does while waiting, so it moves an arm
in proportion to how much of that arm's cost is waiting. The acquire-release
arm spends 142 of its 220 contended cycles waiting; the fenced-store arm spends
22 of 86. A lever made of waiting therefore barely touches the arm whose cost
is the barrier. Registering a regression across two arms while choosing a lever
that only moves one of them is an error of design, not of thresholds.

The correction is a lever that acts on both, and it has to be shown to act on
both before the regression is registered rather than discovered afterwards. Ring
capacity is such a lever: a smaller ring makes the producer wait on the
consumer's cursor more often whatever the ordering costs. Across capacities 2,
16, 128 and 1024 the two arms span 21.5 and 2.7 in cost, both far above the
floor, and H83 registers the lever check as its first guard so that the same
failure stops the run instead of the analysis.

**CLOSED 2026-11-22** `titan-barrier` takes ring capacity as a per case value
rather than a constant, records it in every record as `cap=`, and gains a
`capsweep` mode. The eight cells were run and checked correct before H83's
thresholds were written. H82 keeps its unevaluable verdicts; its data is not
reinterpreted.

**What H82 found anyway, recorded as post hoc and not as a result.** Across all
eight cells the two arms never cross on writebacks per cycle: the
acquire-release arm reaches at most 0.005028 and the fenced-store arm falls no
lower than 0.005335, a separation of 1.0612 to 1.2781, while the wait strategy
moves acqrel's duration by a third. That is arm specific coherence traffic which
duration does not explain, and it is exactly what H83 is registered to test
properly. It is written here rather than in the paper because fitting a
threshold to data already seen is the practice this protocol exists to prevent.

---

## D30. A threshold calibrated on the wrong machine

**Found** 2026-11-22, by H83's lever guard refusing the regression it gated for
the second time in two attempts.
**Severity** a guard was set to a value that no measurement on the target
machine supported, and it blocked a run that would otherwise have been fine.

H83 registered that the capacity lever must move each arm's cost by at least
1.50 before a regression across the arms would be read. Measured on the M4, the
fenced-store arm spans 2.0606 and the acquire-release arm 1.4205, so the guard
fires and parts (c) and (d) are unevaluable.

The threshold came from this project's own development container, an x86 Linux
machine, where the same sweep spans 21.5 and 2.7. I ran the lever there, saw it
move both arms by a wide margin, and wrote 1.50 as a comfortable floor. A
lever's magnitude is a property of the machine, and this project's standing rule
is to never assume a fact that is only knowable on the measurement machine. I
broke it in the one place where the rule is easiest to forget, which is a
threshold rather than a result.

What makes it worse is that the Linux numbers were not merely different in size
but opposite in shape. There, a smaller ring made the acquire-release arm
dramatically more expensive, 275 nanoseconds per message at capacity 2 against
12.8 at 1024. On the M4 capacity 2 is the cheapest cell for that arm, 154.386
cycles against 208.656 at 1024. I had the direction backwards and would not
have known it from the container.

**CLOSED 2026-11-22** The threshold is not lowered to fit what was measured,
because fitting a guard to the data it guards is the practice this protocol
exists to prevent. H83 keeps its unevaluable verdicts and its data is not
reinterpreted. H84 instead asks the question the M4 data actually poses, using
a natural matched duration cell rather than a regression that needs a wide
lever, and any future lever threshold is to be taken from a pilot on the
measurement machine and cited to its file.

**The guard pattern is working and that is worth recording too.** Two
registered discriminators in a row have been stopped by their own guards before
they could produce a confident answer from an unsuitable design, and both runs
were still worth their time: H82 gave the non overlap of writebacks per cycle
and H83 gave the sign reversal with capacity and a matched duration comparison
that neither was designed to produce. A guard that fires is not a wasted run.
It is the difference between a paper that reports what it measured and one that
reports what it hoped.

---

## D31. A verdict script that interprets, and overstated what it had

**Found** 2026-11-22, by reading H84's table rather than its verdicts.
**Severity** five thresholds held and the sentence printed under them claimed
more than they establish. Had that sentence been copied into the paper it would
have been the easiest finding in the project for a reviewer to break.

H84's registered parts all hold. The comparison does change sign with ring
capacity, acqrel faster at capacity 2 in 20 of 20 launches and slower at 1024 in
20 of 20, and coherence traffic carries no arm specific component beyond 1.2168
against a band of 1.50. Those are the measurements and they stand.

Beneath them the script printed, as a REGISTERED READING, that the paper's
central claim is bounded to rings of capacity 16 and above and that capacity is
a third governing variable. I wrote that sentence into the script before the run,
as a registered consequence, and both halves of it go further than the data.

The winner across capacities 2, 4, 8, 16 and 1024 is acqrel, fencedstore,
acqrel, fencedstore, fencedstore. Three sign changes over five points. A
boundary at 16 rests on the two points above it while the region below
alternates, and no threshold was registered over that region at all.

And capacity is not a separable variable in this design. The slot type is eight
bytes and the slot region is page aligned, so capacity alone fixes the slot
array's footprint, 16, 32, 64, 128 and 8192 bytes, which first fills a 128 byte
line at exactly the capacity where the acquire-release arm breaks by 2.716 with
refills and writebacks each rising near fourfold. Capacity alone also fixes how
far the producer may run ahead. Three candidate causes move as one, and naming
the one that happens to label the axis is not an inference.

**CLOSED 2026-11-22** The reading is withdrawn and replaced with what the data
supports: the sign of the comparison depends on ring capacity over the range
measured, non-monotonically, and which property of the ring governs it is open.
H85 is registered to separate footprint from slot count and run-ahead with a two
by two that changes the footprint by sixteen while holding both others fixed.

**The lesson is about where interpretation lives.** A registered consequence is
meant to bind the analysis in advance, and it does that well for a threshold,
which is arithmetic. It does it badly for a sentence, which can smuggle an
extrapolation into a place where it looks pre-approved. Thresholds stay in the
scripts. Readings of them belong in the register and the paper, where they can
be argued with, and from now the scripts print the numbers and name the row to
read rather than announcing what the numbers mean.

---

## D32. Derived statistics filed under another run's key

**Found** 2026-11-22, while gathering figures for the results section.
**Severity** ten lines of H84 statistics sat in the H84 file under the key
`h83_cell`, and two figures the manuscript needed had never been emitted at all.

The H84 verdict script was written from the H83 one. Everything that referred to
the run was renamed, the file path, the mode name, the capacities, the questions
and the thresholds, except the record key in the emit path, which nothing reads
during a verdict run and which therefore no test exercised. The file's records
were correct and its derived block attributed them to the previous experiment.
Nothing had been read wrongly because nothing had read them yet, which is the
only reason this is a defect and not a retraction: a script grouping derived
lines by prefix would have merged two different sweeps.

A second fault surfaced in the same minutes. Part (d) of H84 compared the
writeback rate per cycle between the arms and printed the comparison, and the
emit path never wrote it to the file, so the one figure the results section
needed for that claim existed only in a terminal. A number the manuscript
quotes has to exist in a record, and the provenance gate enforces that from the
manuscript side, so the omission would have shown up as a gate failure rather
than as a wrong number. It is still a gap in the record.

**CLOSED 2026-11-22** The key is corrected, the rate and sign comparisons are
emitted, and the derived block is regenerated twice with both regenerations
noted in the file itself. The 600 records above it are byte identical before and
after, verified by comparing them, because a derived block is script output over
records in the same file and is reproducible by running the script, while the
records are the measurement and are never rewritten.

`scripts/check_record_lines.py` now checks that a results file whose name
carries an hNN contains derived keys for that hNN and no other, with cross
references declared rather than tolerated: the H80 and H81 files legitimately
carry each other's statistics because part (g) of H81 is the agreement of their
directions, and that pair is listed with its reason. Eleven derived keys are
checked, and the check was confirmed to fail on an invented h99 key before
being trusted.

---

## D33. The abstract had grown past what any venue accepts

**Found** 2026-11-22, while finishing the manuscript.
**Severity** the paper could not have been submitted anywhere, and nothing in
the project would have said so until a submission form refused it.

The abstract had reached 909 words, roughly 5800 characters. arXiv's submission
form caps that field at 1920 characters. IEEE and the other targets expect
something closer to 250 words. The paper was structurally complete, every gate
passed, and it was unsubmittable.

Nothing went wrong in a single step. The abstract began at a reasonable length
and each result added a sentence that was individually justified: the cost
fraction, the cross of the two halves, the measurement hazard, the clustering
ladder, the protocol size. Every addition was true and sourced. The sum was a
page of prose where a paragraph belongs, and because no check looked at its
length, the growth was invisible from inside the project.

Two substantive overclaims were found in the same pass and corrected rather
than carried into the short version. The abstract said that a rule for when a
store-release costs more than a stronger barrier "is what we measure"; the
paper measures three conditions that decide it, one of them non-monotone and
confounded, which is not a rule. And the related work section said "the rule
measured here is caching state", written before ring capacity was known to
reverse the comparison again.

**CLOSED 2026-11-22** The abstract is 1654 characters and 265 words, 266 under
the arXiv limit, and states the capacity dependence, the waiting ratio and that
the mechanism is not identified, which the long version did not. The detail it
shed was already in the contributions subsection, where it belongs.

`scripts/check_claims_semantics.py` now fails when the abstract exceeds 1920
characters and prints the margin on every run, and the check was confirmed to
fail on a padded abstract before being trusted. A limit that lives in a web
form is invisible until the day it blocks you, so it lives in the gate instead.

---

## D34. A currency check that compared spacing

**Found** 2026-11-22, by the figure gate failing on the measurement machine and
passing here.
**Severity** a gate blocked a correct commit and would have been loosened or
switched off by anyone in a hurry, which is the worst thing a gate can do.

`check_paper_figures` verifies that an embedded figure still matches the records
it draws, by backing up the figures, regenerating them from committed data,
comparing, and restoring. The comparison ran on the output of
`pdftotext -layout`, which encodes glyph positions as runs of whitespace. A
different matplotlib or font version shifts a label by a point and the entire
string differs.

`fig-barrier.pdf` carries more text than any other figure in the paper, five
arm labels and ten values across two panels. It had never been currency checked
before because it was not embedded, and the first run after embedding it failed
on the measurement machine while passing on the machine that had last
regenerated it. Nothing about the data had moved.

This is the third false alarm from comparing PDFs across machines. D7 recorded
that cross-machine byte identity is not achievable and should not be demanded.
D17 recorded reintroducing that demand four days after closing it. D34 is the
same mistake at one remove: not bytes, but a text extraction that carries
layout, which is bytes wearing a different coat.

**CLOSED 2026-11-22** The check compares what a figure says rather than where
it says it. The numeric tokens and the alphabetic tokens of the extracted text
are compared as multisets, so spacing, column order and line breaks cannot
raise an alarm while a changed value still does. On failure it now prints which
numbers and labels the committed figure has that a regeneration does not, and
the reverse, so the next person sees what moved instead of a verdict. Swapping
one figure's content for another's was confirmed to fail, with the diagnostic
naming the numbers that differ, before the new form was trusted.

**The general rule this project keeps relearning.** A check across two machines
must compare the thing it cares about and nothing else. Every time a comparison
has been allowed to carry incidental detail, bytes in D7 and D17 and layout
here, it has fired on correct work. The question to ask of any new check is what
it would take for it to fail while the work is right.

---

## D35. The same false alarm again, one level down

**Found** 2026-11-22, by the measurement machine failing the gate D34 had just
repaired.
**Severity** a correct commit was blocked twice by the same check, for the same
reason, in the same hour.

D34 replaced a layout-sensitive text comparison with a comparison of numeric
and alphabetic tokens. The next run on the measurement machine failed again,
and the diagnostic that D34 added is what settled it: zero numeric differences,
and the labels reported as lost and gained were `ord`, `tor`, `un` against
`dl`, `nce`, `nord`, `oa`. Those are fragments. `pdftotext` breaks a label
wherever glyph spacing suggests a gap, so the same word came out as `un` plus
`ord` plus `ered` on one machine and `unord` plus `ered` on the other, and a
multiset of word tokens is no more stable than the layout was.

The numbers were identical, which is the thing the check exists to protect. I
removed layout and left tokenisation, which is layout at one remove.

**CLOSED 2026-11-22** The letters of the extracted text are concatenated into a
single string, so the same label yields the same content however it was split,
and the numbers are still compared as a multiset. Swapping one figure's content
for another's was confirmed to fail, reporting both the numeric differences and
that the letter content differs, before the new form was trusted.

**Four entries now, on one question.** D7 established that cross-machine PDF
byte identity is not achievable. D17 recorded demanding it again four days
later. D34 removed layout and kept tokenisation. D35 removed tokenisation. The
error each time was to compare an artifact of rendering alongside the content,
and the diagnostic added in D34 is what made D35 take minutes instead of an
evening. The rule, now stated once rather than rediscovered: a check that spans
two machines compares the quantity under claim and nothing that travels with
it, and it prints what differs rather than only that something did.

---

## D36. Three patches where one decision was needed

**Found** 2026-11-22, by the figure gate failing a third time on the
measurement machine.
**Severity** three of the operator's runs were spent on a cosmetic comparison,
and the fault each time was mine.

The check asks whether a committed figure still matches the records it draws.
Three attempts compared, in order: the raw output of `pdftotext -layout`, which
encodes glyph positions as whitespace; word tokens, which `pdftotext` splits
wherever spacing suggests a gap, so one machine produced `un` plus `ord` plus
`ered` and the other `unord` plus `ered`; and the letters concatenated, which
removed the splitting and kept the ordering, so a label shifted by a point is
emitted before rather than after its neighbour and the string changes while
every character is identical.

Each fix removed the specific artifact that had just fired and left the next
one in place. None of them asked the question that settles it.

The question is what a figure's words can tell you about its currency, and the
answer is nothing. The data appears in a figure as numbers. The words appear
because the plotting script writes them, and the committed figure and a fresh
regeneration come from the same script, so a word can differ only for a
rendering reason and never for a data reason. Comparing words adds no power to
detect staleness and adds a cross-machine failure mode.

**CLOSED 2026-11-22** The check compares the numbers and nothing else, with the
reasoning written in the function so the next person does not add the words
back. The textual side of a caption remains guarded by the separate rule that
every number a caption quotes must appear in the results file the caption
names. Swapping one figure's content for another's was confirmed to fail,
naming eight differing numbers in each direction, before the form was trusted.

**And the artifacts move to the machine that submits.** Figures are regenerated
on the measurement machine and committed from there, before the gate runs, so
the only rendering that has to agree with itself is the one that will be
submitted. Had that been the arrangement from the start, none of D34, D35 or
D36 would have cost a run.

**What the pattern says about me.** Four register entries on PDF comparison and
three of them in one hour. The first fix was reasonable, the second was a
reflex, and the third should have been a stop and a question. The rule I am
adding to my own practice, stated here because the register is where this
project keeps its corrections: after one fix to a check fails, do not write the
second fix. Work out what the check can and cannot see, and decide what it
should compare.

---

## D37. A cross-check between two counters that measure different things

**Found** 2026-11-22, by H86 part (d) failing by a factor of 61 in the wrong
direction.
**Severity** the threshold was written to protect two measurements against each
other and would have impeached a correct one.

Part (d) required the backend stall ratio between the arms to be at or below
0.50, on the reasoning that H78 had already measured a waiting ratio of 0.2338
between the same arms and the two should agree. It came back at 30.6473:
acquire-release spends 0.3507 cycles per message with nothing issuing because of
the backend, and fenced-store spends 10.7480.

The registered consequence said that a disagreement meant both measurements
were suspect and both must be rerun. **That consequence is void and was not
acted on**, which is recorded here rather than passed over in silence, because
quietly dropping a registered consequence is worse than the threshold that
needed it.

It is void because its premise is false. `ARM_STALL_BACKEND` counts cycles in
which nothing issued because of the backend. `LDST_UNIT_OLD_L1D_CACHE_MISS`
counts cycles in which an old load or store is waiting for data after an L1D
miss, and a cycle can be both waiting and issuing. The acquire-release arm waits
139.0505 cycles on misses while continuing to issue spin loop work, so it stalls
almost never. The fenced-store arm's barrier drains the pipeline, so when it
waits it stalls outright. The two counters are not proportional, were never
going to be, and neither measurement is impeached by their disagreement.

This is the sixth threshold in this project that was precise, checkable, fixed
before the run, and aimed at the wrong quantity, after a pair read as five arms,
a composition read as an absolute, the wrong thread, a guard wider than its
claim, and a rate normalised by the thing under comparison. The new variety is
worth naming because it is the most seductive: a consistency check between two
counters whose definitions were not read side by side. The correction extends
D25's rule. Before a threshold compares two counters, read both descriptions
from the machine's own dump in the same sitting and state in one sentence why
the quantities must move together. If that sentence cannot be written, the
threshold is not a cross-check.

**CLOSED 2026-11-22** Part (d) is recorded as refuted with its consequence
explicitly void and the reason given. Part (c) is also refuted, at 1.8319
against a band of 1.25, and for a related reason that is not a defect but a
property of the workload: each spin iteration retires uops, so uop count scales
with waiting and a work parity threshold over a spin-waiting ring cannot hold.
H87 is registered to find the 201.186 cycles per message, 92.07 percent of the
acquire-release arm's contended cost, that H86 left in neither the issuing nor
the backend-stalled state.

---

## D38. A duplicate citation key, a gate that printed less than it knew, and a
claim of mine that was simply wrong

**Found** 2026-11-22, while expanding the bibliography.
**Severity** three faults in one pass, two in the tooling and one in what I
told the operator.

**The claim.** I said Mytkowicz et al. was the canonical citation for
measurement bias and was absent from the bibliography. It was present, at line
15, and had been for some time. It was uncited, which is a different and smaller
problem. I added a second entry under the same key.

**The duplicate.** Two `@inproceedings{mytkowicz2009wrong,` entries. bibtex
resolves that by keeping one and discarding the other without saying which, so
a rendered citation can carry the details of an entry nobody is reading. The
only symptom was that a count of entries and a count of distinct keys
disagreed by one, which surfaced because two different scripts of mine counted
them differently and I chased the discrepancy instead of picking the larger
number. The original entry was fuller and is kept; the DOI, which only the
duplicate carried, is merged into it.

**The gate that knew and did not say.** `check_provenance` collects failures in
one list and printed that list before two further checks appended to it, so the
percentage and cited-number failures were counted in the exit status and never
shown. The output said three numbers were unaccounted for and named none of
them. That is worse than a missing check: it reports a problem and withholds
the evidence, so the next person has to re-derive what the script already knew.
The printing now happens after every check that appends.

**CLOSED 2026-11-22** The duplicate is removed with its DOI merged.
`check_claims_semantics` now fails on a duplicate key, on a citation absent
from refs.bib, and on an entry nothing cites, since an uncited entry does not
appear in the rendered bibliography under IEEEtran and is invisible dead
weight. All three were confirmed to fail before being trusted. The bibliography
is 15 entries, every one cited, every citation present.

**The pattern worth keeping.** Twice today a discrepancy between two of my own
counts was the only evidence of a real fault: here, entries against distinct
keys, and earlier, a verdict tally that listed six parts and counted five. Both
times the honest move was to chase the disagreement rather than report the
number that looked right. A tool that counts the same thing two ways and gets
two answers is telling you something.

---

## D39. My font report truncated the one column that matters

**Found** 2026-11-22, reading the output of my own typesetting step.
**Severity** the check that decides whether the PDF is acceptable to arXiv and
IEEE PDF eXpress printed its answer with the answer cut off.

The build step prints the fonts embedded in the rendered PDF, because a Type 3
font anywhere in a submission is rejected by both targets. My invocation piped
`pdffonts` through an `awk` expression that printed the first two
whitespace-separated fields, and font names contain no spaces while the type
column reads `Type 1` with one. So every line came out as a name followed by
the word `Type` and nothing else. The output listed ten fonts and told me the
type of none of them.

The names are informative on their own: NimbusRomNo9L, NimbusMonL, CMR10,
CMMI10, CMSY7 and CMSY10 are the standard Type 1 families that pdfTeX embeds,
and Type 1 is accepted by both targets. So the submission is very probably
fine. **Very probably is not a check**, and this project's own rule is that a
step reporting success without verifying the thing it claims is the defect this
register keeps recording.

**CLOSED 2026-11-22** The font report prints the full `pdffonts` table
unmodified and then greps it for `Type 3`, failing loudly if any appears, so the
answer is a verdict rather than a listing to be squinted at. The figures were
already covered by a separate Type 42 check inside the figure pipeline; this was
the gap for the document's own fonts.

**Why this one is worth an entry.** It is the fourth time today that a
reporting fault, not a measurement fault, cost real information: the figure
gate compared rendering three times, the provenance gate counted failures it
never printed, two of my scripts disagreed on a bibliography count and only the
disagreement revealed a duplicate key, and now the font column. Every one of
them was in the layer that tells me what happened rather than in the layer that
makes it happen. The measurements have been sound all day. The instruments
reporting on them have not.

---

## D40. Every counter run in this project used one wait strategy, and it
dominated what two of them measured

**Found** 2026-11-23, by H88.
**Severity** the highest in this register. One recorded conclusion is
downgraded, two figures become conditional, and one central claim is
strengthened. The cause was a constant nobody questioned.

Every run in this project that carries stall, waiting or issue counters also
carries `wait=isb`. The consumer's wait loop executes an `isb`, which is
context synchronizing and forces an instruction refetch. A loop built on it
generates frontend stalls by construction, and the two arms differ in how many
iterations they spin, so they differ in how many `isb` they execute.

H88 measured the size of that. The acquire-release arm's frontend share of
cycles is 0.9661 under `isb` and 0.0542 under `spin`, a factor of 17.8. The
96.58 percent frontend stall H87 reported is the wait loop.

**What this costs, claim by claim.**

H86's exclusion of the throttling account is **downgraded to pending**. It
rested on the fenced-store arm issuing on 2.7363 times more cycles than the
acquire-release arm. Under `isb` the acquire-release arm is 96.61 percent
frontend stalled, so its issue-active fraction is suppressed by the wait loop
rather than by the ordering construct, and a ratio of two such fractions says
little about either arm. H89 re-measures it across all four wait strategies and
registers in advance that a ratio below 0.90 means the account is live again and
the paper must retract the exclusion.

H78 and H79's waiting figures, 0.2338 on the consumer and 0.2161 on the
producer, keep their direction and lose their generality. H88's backend stall
ratio under `spin` is 0.3511, under `yield` 0.3425 and under `wfe` 0.3842, all
the same sign and the same order of magnitude, so the finding that the cheaper
arm waits materially less is not an artefact. The specific magnitudes are
conditional on the wait strategy and the paper states them that way.

H87's decomposition stands as recorded, which is to say its registered parts
were already unevaluable and its post hoc reading is now explained.

**And one claim comes out stronger.** H77's cost ratios, 2.5766 contended and
2.7008 resident, were measured under one wait strategy and nothing had tested
whether they survive another. H88 part (c) tested exactly that: the cost ratio
of the two arms is 0.4161 under spin, 0.3849 under isb, 0.3945 under yield and
0.4532 under wfe. The central finding of this paper is independent of how the
consumer waits, which it had been assuming for three weeks without evidence.

**CLOSED 2026-11-23** H89 is registered to settle H86. The manuscript gains the
wait-strategy robustness result and states which counter figures are
conditional.

**What H89 returned, written in the same day and left here rather than folded
into the text above.** The sentence above says H89 registers that a ratio below
0.90 revives the throttling account and forces a retraction. It did not reach
that bound and it did not reach the bound that would have kept the exclusion
either. Under a plain spin the fenced-store arm's issue-active fraction is
1.0800 times the acquire-release arm's, 0.0200 below the keep bound and inside
the interval the registration had named as inconclusive. So H86's exclusion is
downgraded from excluded to unsupported, not retracted, and the manuscript now
reports the throttling account as open. The isb cell replicates H86's 2.7363 at
2.6935, which matters: the number was right and the inference drawn from it was
not. The provision that made this reportable is that the registration carried
three outcomes rather than two, and that provision was not foresight. It was
written because this register already held five instances of a threshold aimed
at the wrong quantity, and a two-outcome threshold is one more way to be forced
into an answer the data does not carry. The methodological point is the one worth carrying: a parameter
held constant across every run is invisible, and its effect is attributed to
whatever was varied. The protocol in this project varies arms, caching state,
thread, capacity and wait strategy; the wait strategy was added last and
immediately showed that it had been silently carrying two results. Before a
counter figure is published, vary the parameters that were constant when it was
taken.

---

## D41. The same print-order fault, in the same file, two days apart

**Found** 2026-11-23, writing the wait-strategy result into the manuscript.
**Severity** low in consequence and high in what it says about the fix applied
to D38.

D38 recorded that `check_provenance` printed its list of broken declarations
before two later checks appended to it, so failures were counted and never
shown. The fix moved that one print. The file has a second list, of numbers
found in the manuscript and in no results file, and that one was still printed
before the step that filters it, so a number whose own run is simply absent
from the checkout appeared in the unsourced list and in the not-verified-here
list at once, with the count disagreeing with both.

The fix for D38 was applied to the list that had just failed rather than to the
pattern, which is the same thing D36 recorded about the figure gate. I did it
again, in the same file, two days later.

**CLOSED 2026-11-23** Both lists print after every step that can change them.
The rule, stated for the third time and now in the only place that matters,
which is next to the code: a report is written after the computation it
reports, and a check that collects into a list prints that list once, at the
end.

**And a note in the gate's favour.** It refused a derived figure because one of
its inputs was itself unsourced in this checkout, which is exactly right and
caught a convenience number the paper did not need. The figure was removed
rather than declared. A gate that is pedantic about inputs is worth more than
one that trusts an arithmetic I typed.

## D42. A registered test was placed on one cell, and the claim it decided was
scoped over every cell

**Found** 2026-11-23, reading H89's result against its own registration.
**Severity** moderate in consequence and high in what it adds. The verdict is
unchanged by it, because the registered reading is the one we took. It is the
seventh instance in this register of a pre-registered threshold that was
precise, checkable, honoured exactly as written, and not a test of the claim it
was written for, and it is the first one found inside a registration written
after the first five were known.

H89 asks whether H86's exclusion of the pipeline-throttling account survives the
removal of the `isb` from the consumer's wait loop. The claim is about the two
ordering constructs. It is not about any one way of idling, and the run sweeps
four of them. The registration nonetheless placed its deciding test on a single
cell, `wait=spin`, on the reasoning that a plain spin contains no
context-synchronizing instruction and is therefore the cleanest comparison.
That reasoning is sound and the cell was the right one to choose. The error is
that choosing one cell was never stated as a choice, and no rule was fixed for
what a disagreement among the four cells would mean.

The four cells disagree. The issue-active fraction of the fenced-store arm over
the acquire-release arm is 1.0800 under `spin`, 1.1042 under `yield` and 1.7344
under `wfe`, with `isb` at 2.6935. Three of the four clear the 1.10 bound that
would have kept H86 standing. The registered cell misses it by 0.0200.

**What we did.** We took the registered reading. H86's exclusion is downgraded
to unsupported, the manuscript does not claim that throttling is excluded, and
the three cells that clear the bound are recorded as a consequence and not
claimed. Switching to `yield` after seeing the data would have been the exact
practice this protocol exists to prevent, and the fact that the switch would
have been defensible on physical grounds is what makes it dangerous rather than
what makes it acceptable.

**What it costs honestly.** A reader may conclude from the full table that the
fenced-store arm does issue on more cycles under every wait strategy that
contains no synchronizing instruction, and that reader would not be unreasonable.
We cannot offer that conclusion, because the test that would have licensed it
was not the test we wrote. The cost of this defect is a result we may have and
cannot claim, and that is the correct cost for the mistake.

**One more thing this corrected.** The H88 row recorded that its post hoc
backend-stall reading was "registered as H89 rather than claimed". H89 places a
threshold on the throttling question only; its guard requires the backend
counter to be non-zero and tests nothing about the backend ratio. The row
overstated what existed, and the manuscript inherited the overstatement in a
sentence saying the backend mechanism was pre-registered and not yet run. Both
are corrected, and neither claimed anything, so nothing is retracted.

**CLOSED 2026-11-23** The fix is a rule added to the protocol and to the
manuscript's recommendation. Where a claim is scoped over several cells, state
which cells the test is evaluated on and fix in advance what a disagreement
among them means. And where a prior conclusion is under test, register three
outcomes rather than two: the bound above which it stands, the bound below which
it is retracted, and the interval between them, in which it is downgraded. H89
carried the three and could therefore report the answer the data supports. A
two-outcome version of the same registration would have forced us either to
retract a sound measurement or to keep a conclusion it does not support.

---
## D43. The page-range dash rule was written for the page ranges that happened
to be in the bibliography at the time

**Found** 2026-11-24, by the build refusing to produce a PDF.
**Severity** low in consequence and worth recording for the pattern. Nothing
shipped, because the guard that sits after the rewrite caught it and stopped
the build.

The build rewrites the double-hyphen ligature that IEEEtran.bst generates for a
page range, because that ligature renders as an en dash and this manuscript
contains none. The rewrite matched a double hyphen with a digit on each side,
which is every page range a bibliography of fifteen conference and journal
entries happens to contain.

Adding a PNAS correspondence item broke it. That journal numbers electronic
pages with a letter prefix, and IEEEtran.bst inserts a thin space as a
thousands separator inside the number, so the range arrives in main.bbl as
`E10\,516--E10\,517`. The character after the ligature is a letter, the rule
did not match, the ligature survived, and the guard refused the build with the
surviving line quoted.

**What this is an instance of.** The rule was correct for every input it had
ever been given and was not a statement about page ranges in general. That is
the same shape as the threshold defects in this register: a check that is
precise, honoured, and narrower than the claim it stands for. The difference is
that this one was caught by a guard rather than by reading a verdict a week
later, which is the argument for writing the guard.

**CLOSED 2026-11-24** The character class is widened to alphanumeric on both
sides, with the reasoning for stopping exactly there recorded beside it in
build.sh: a double hyphen between two alphanumerics cannot occur in a title,
which keeps its single hyphens, and cannot occur inside the six-hyphen
repeated-author ligature, which has no alphanumeric between its hyphens. The
guard is unchanged and still refuses the build on any survivor.
## D44. The figure gate read the first run a caption named and ignored the rest

**Found** 2026-11-24, by the gate refusing the new wait-strategy figure.
**Severity** low in consequence, recorded because it is the third instance in
two days of the same shape and the count is the point.

The gate requires that every number a figure caption quotes appears in the
results file the caption names. It found that file with a single regular
expression match, which is the first one. Every figure in the manuscript until
now was drawn from one run, so the difference between the first file and all of
them had never shown.

The wait-strategy figure is drawn from two: its cost and stall panels come from
the stall probe and its issue panel from the issue probe. The gate checked all
six caption numbers against the first file and refused the figure for the two
that live in the second.

The refusal was correct in form and wrong in substance, and the dangerous part
is what it invites. The quick ways out are to split one argument across two
figures, or to drop the two numbers from the caption. Both would have made the
paper worse to satisfy a check, which is the failure mode a gate suite has to
be watched for.

**CLOSED 2026-11-24** The gate now collects every results file a caption names,
requires all of them to exist so that naming a file one does not use cannot
widen the search, and checks each quoted number against their union. Confirmed
to fail on a fabricated caption number before being trusted: a figure quoting
9.9999 is refused and the message names both files it searched.
