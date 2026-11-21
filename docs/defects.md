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

**Status at 2026-11-21: twenty closed, none partly closed, none open. Twenty entries in all.**
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
