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

**Status at 2026-11-20: nine closed, two partly closed, none open.**
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

**PARTLY CLOSED 2026-11-20. The lead results are recomputable; a cycle-level
replication is not yet done.**

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

**PARTLY CLOSED 2026-11-20.** The Related Work section now states what the
approximation predicts and what was measured, and names the dissociation
between the two statistics as the contribution rather than a contradiction of
theory. `crates/titan-itch/src/bin/titan-arrivals.rs` reports both statistics
side by side on the same schedules so the two can never again be confused.
`scripts/check_claims_semantics.py` fails if the manuscript mentions the Fano
factor in a paragraph that does not also name the coefficient of variation.
**Still open:** the hypothesis that the latency penalty tracks the coefficient
of variation rather than the Fano factor is not yet registered or run.

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
