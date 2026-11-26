# The manuscript

`main.tex` is the IEEEtran source. `refs.bib` holds only citations verified
against at least two independent bibliographic sources or read directly.

## Build

    cd docs/paper
    pdflatex main && bibtex main && pdflatex main && pdflatex main

IEEEtran V1.8b (2015/08/26) and IEEEtran.bst are required. On Debian or Ubuntu
they come from `texlive-publishers`.

## Dashes: zero, in the source and in the rendered PDF

This project forbids em dashes and en dashes. That is enforced on both the
source and the compiled output, and the compiled output is the one that
matters, because two of IEEE's own files generate dashes that never appear in
the manuscript text.

| Source of the dash | How it is removed |
|---|---|
| `IEEEtran.cls` emits a triple-hyphen ligature after `Abstract` and after `Index Terms` | `main.tex` patches both macros in its own preamble with `etoolbox`, substituting a colon |
| `IEEEtran.bst` turns page ranges into a double-hyphen ligature | `build.sh` rewrites them in `main.bbl` after bibtex |
| `IEEEtran.bst` writes a six-hyphen ligature in place of an author list identical to the one above it | `build.sh` restores the names in `main.bbl` |

**`IEEEtran.cls` and `IEEEtran.bst` are never edited.** A modified class file
would be a format non-conformance that a reviewer or an automated format check
would be right to flag. Everything happens in this project's own preamble and
build script.

The preamble patch raises a `\PackageError` if a future IEEEtran stops
matching, so the failure mode is a stopped build rather than a dash reaching a
submission unnoticed.

`build.sh` ends by extracting the text of the compiled PDF and failing if a
single em dash or en dash survives. It currently reports zero.

`scripts/check_dashes.py` covers the whole repository and reads source only. In
LaTeX and BibTeX source it also rejects the ligature sequences, which render as
dashes without being dash characters. A line may opt out with an explicit
verbose marker, and only the two `\patchcmd` calls use it, because a line whose
job is to delete a ligature has to name it. It reports 167 files and zero
violations.

**An earlier version of this file argued the six IEEE dashes should stay
because they are house typography. That was wrong against a rule stated as
absolute, and it is recorded here rather than deleted.**

## arXiv packaging

arXiv runs neither `bibtex` nor `biber` on an uploaded `.bib` file, so the
tarball must carry a prebuilt `main.bbl`. It must also carry `IEEEtran.cls`,
because arXiv's TeX installation cannot be assumed to hold it. `\pdfoutput=1`
is already on line 1 of `main.tex`, inside the first five lines, which is where
arXiv looks for it.

## Status

Complete. Seven sections (Introduction, Related Work, Method, Results,
Discussion, Threats to Validity, Conclusion), 15 pages, 31 references, eight
embedded figures, and an abstract of 1877 characters against arXiv's 1920
limit. No placeholders anywhere.

The document compiles with zero undefined citations, zero undefined
references and zero overfull boxes. Eleven checks in `scripts/` run against it
and must all pass before a commit: they refuse a number in the prose that
cannot be traced to a committed results file, a caption quoting a figure
absent from the file it names, a figure stale against its generator, a dash in
the rendered PDF, and a README that disagrees with the manuscript.

Sections were added one complete section at a time, with the document
compiling after each. That is how it was written, not a description of where
it has reached.
