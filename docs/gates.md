# The gate suite

Eleven checks that a measurement paper must pass before it is committed. Every
one of them exists because something went wrong in this project, every one of
them was confirmed to fail on a deliberately broken input before it was
trusted, and every one of them names the defect register entry that motivated
it.

This file exists so the suite can be adopted without reading the project it
came from. The checks are plain Python with no dependencies beyond the standard
library and `pdftotext` for the figure checks. They assume a repository laid out
as `docs/paper/main.tex`, `docs/paper/refs.bib`, `docs/figures/`,
`docs/experiments.md`, `docs/defects.md` and `results/*.txt`, and the paths are
at the top of each file.

Run them all and stop on the first failure:

```
for g in scripts/check_*.py; do python3 -I "$g" || break; done
```

## What each one refuses

| check | refuses | motivated by |
|---|---|---|
| `check_provenance.py` | a number in the manuscript that appears in no result file, is not declared as arithmetic over such numbers, and is not cited to a reference | D3, D23 |
| `check_claims_semantics.py` | a number used for the wrong quantity, a register status line disagreeing with its own markers, an abstract over arXiv's 1920 character limit, a duplicate citation key, a dangling citation, an entry nothing cites | D1 to D5, D8, D10, D33, D38 |
| `check_record_lines.py` | prose in a results file wearing a record prefix, and a derived statistic filed under another run's key | D32 |
| `check_counter_sanity.py` | a performance counter field holding one value across a whole file, which is the signature of a counter that was never programmed | D19, D21, D27 |
| `check_power_lines.py` | a results file that does not state its power condition, and a register row claiming a power state no run it cites recorded | D12, D26 |
| `check_figure_claims.py` | a figure asserting a number that an independent recomputation from the records does not produce | |
| `check_paper_figures.py` | an embedded figure that is missing, carries a Type 3 font, quotes a number absent from the results file its caption names, or no longer matches the records it draws | D7, D17, D34, D35, D36 |
| `check_figures_deterministic.py` | a figure that is a function of when or where it was built rather than of its data | D7 |
| `check_instruction_claims.py` | an instruction the manuscript names that the compiler does not emit, checked against a committed disassembly census | D11, D15, D18 |
| `check_dashes.py` | an em dash or en dash anywhere the authors wrote, including after BibTeX rewrites it into the bibliography | |
| `check_authorship.py` | a commit whose author and committer differ from the declared one, a tooling trailer, or a generated-prose marker in a tracked file | |

## The four rules the suite is built on

These were learned the hard way and each cost at least one wasted run.

**A number in a paper must exist in a file.** Not be recoverable from one by
rounding, not be reproducible by arithmetic the reader is trusted to repeat. The
provenance check reads the manuscript, extracts every numeral, and requires each
one to be either present in `results/`, declared as an expression over numbers
that are, or attributed to a citation. Percentages are declared separately
because a percentage is a claim about a denominator.

**A threshold must name the cells it is evaluated on.** Six pre-registered
thresholds in this project were precise, checkable, fixed before the data
existed, and not tests of the claims they were written for: one scoped over five
arms when the claim concerned a pair, one over an absolute when the claim
concerned a composition, one against the thread that does not execute the
instruction under test, one guard wider than the comparison it guarded, one
normalising a quantity by the thing under comparison, and one cross-checking two
counters whose definitions had not been read side by side. Pre-registration
stops a threshold moving. It does not stop it pointing elsewhere.

**A check that spans two machines compares the quantity under claim and nothing
that travels with it.** Four register entries went on one question before this
was stated plainly. Comparing rendered PDF bytes fails on correct work.
Comparing extracted text fails on correct work, because the extraction encodes
glyph positions as whitespace. Comparing word tokens fails on correct work,
because the extractor splits a label wherever spacing suggests a gap. The
figure currency check compares the numbers a figure draws, because the words
come from the plotting script and both sides of the comparison run the same
script.

**A report is written after the computation it reports.** Three times a check
in this suite collected failures into a list and printed the list before a later
step appended to it, so the exit status counted failures the output never
named. A gate that reports a count it cannot explain is worse than a missing
gate, because the next person has to re-derive what the script already knew.

## Adopting it

Each check is independent. Take the ones that fit, change the paths at the top,
and run them in a pre-commit hook or a CI step. The one piece of discipline that
makes them worth anything is the last column of the table above: when a check
fires on correct work, the fix is not to loosen it but to work out what it can
and cannot see. When it fails to fire on broken work, break something
deliberately and confirm it fires before trusting it again.
