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

**Status at 2026-11-20: six closed, one partly closed, one open.**
Open is D4, the raw measurements behind the lead results. Partly closed is D8.
D5 and D6 were each partly misdiagnosed and both entries carry the correction.

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

**OPEN.** Plan: re-run every experiment whose raw output was discarded with
the raw retained, beginning with the barrier cross and the run-length sweep
because the manuscript's figures parse those two files. The original summaries
stay in place and the replication is reported beside them, including any
disagreement. The run itself is cheap: the committed file records that thirty
launches of three epochs over ten arms is single-digit minutes. Until this is
closed, the lead results of this paper cannot be independently recomputed.

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

**CLOSED 2026-11-20.** `scripts/figures_paper.py` passes a metadata mapping
with the creation date suppressed to every `savefig`. Two regenerations one
second apart now produce byte-identical files, verified. The committed PDFs
are rewritten once by this change, and their extracted text is unchanged,
which is how D5's mistaken half was found.

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
