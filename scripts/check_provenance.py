#!/usr/bin/env python3
"""Every number in the manuscript must trace to a measurement, and this proves it.

The rule this project works under is that no figure in the paper may be typed
in. That rule is only worth something if it is checkable, so this script takes
every number the manuscript asserts and places it in exactly one of three
categories. Anything that lands in none of them fails the build.

    MEASURED  the value appears verbatim in a file under results/, which is
              where terminal output from the measuring machine is recorded.
              The script reports which file.

    DERIVED   the value is computed from measured values. The formula is
              recorded here, the script evaluates it from the measured inputs
              and checks that it rounds to the value the manuscript prints. A
              derivation that stops agreeing is a failure, not a warning.

    EXTERNAL  the value is quoted from a cited work rather than measured here.
              The citation key is recorded and must exist in refs.bib, so a
              number attributed to a paper cannot drift away from its source.

A fourth outcome, a number in none of the three, is the one that matters: it
means a figure was typed into the manuscript that no measurement supports.

    python3 scripts/check_provenance.py

Exit 0 when every number is accounted for, 1 otherwise.

It also reports, without failing, any measured value whose only mentions in
docs/experiments.md are on rows flagged PRELIMINARY PROTOCOL. Those rows were
measured with epochs inside one process rather than over independent launches.
Nineteen rows carry that flag. No value quoted in the manuscript depends on
one at present; the report exists so that if one ever does, it is noticed at
the time rather than at review.
"""
from __future__ import annotations

import glob
import os
import re
import sys
from pathlib import Path

ROOT = Path(__file__).resolve().parent.parent
TEX = ROOT / "docs" / "paper" / "main.tex"
BIB = ROOT / "docs" / "paper" / "refs.bib"
EXPERIMENTS = ROOT / "docs" / "experiments.md"

# Values computed from measured ones. Each entry is the expression, the
# measured inputs it uses, and the number of decimals the manuscript prints.
# The script evaluates the expression and requires it to round to the printed
# value, so an edit to either side is caught.
DERIVED: dict[str, tuple[str, int]] = {
    # Section 4 and the run-length subsection, arithmetic over H86 figures that
    # are themselves in results/2026-11-22_m4_h86_issue.txt.
    "7.77": ("100 * 16.9888 / 218.525", 2),
    "0.16": ("100 * 0.3507 / 218.525", 2),
    "92.07": ("100 * (218.525 - 16.9888 - 0.3507) / 218.525", 2),
    # Section 4 arithmetic over figures that are themselves in the records.
    "2.716": ("173.725 / 63.972", 3),
    "106.5": ("139.0505 - 32.5100", 1),
    "136.9": ("217.755 - 80.894", 1),
    "2.57": ("218.880 / 85.269", 2),
    "2.75": ("22.120 / 8.053", 2),
    # Stated beside the median of the per-launch ratios so the reader is not
    # left to discover that the two estimators differ. See docs/defects.md D3.
    "1.0728": ("133.611 / 124.544", 4),
    # The Mach timebase on this part, 125/3, which is the counter grain the
    # probe measures the hardware obeying. results/2026-11-21_m4_probe.txt.
    "41.667": ("125 / 3", 3),
}

# Values quoted from a cited work. The key must resolve in refs.bib.
EXTERNAL: dict[str, str] = {
    "133": "mytkowicz2009wrong",
    "8.94": "wrenger2024m1",
}

# Percentages, which the matching above cannot verify. Its pattern requires two
# decimals, and that threshold is not arbitrary: "8" and "3" each occur in
# thirty-five results files by coincidence, so a substring search on a short
# number always succeeds and proves nothing. Widening the pattern would
# manufacture confidence rather than earn it.
#
# So this list does the one thing that is checkable: it requires every
# percentage in the manuscript to be declared, with a source that exists. It
# does not verify the value, and does not pretend to. A reviewer still has to
# read the cited paper or the named file. What it prevents is a new percentage
# appearing with no stated origin at all, which is how two figures quoted from
# Emurian et al. first entered the abstract unaccounted for.
# A number is accounted for when it appears somewhere under results/. That is
# the right rule for most figures and the wrong rule for a number tied to one
# specific run, because a four decimal value can match an unrelated file by
# coincidence. It did: 0.8501, quoted in the method section from the counter
# allocation diagnostic, resolved against a record in 2026-11-14_m4.txt, so
# this gate would have passed the claim with its real source uncommitted. Any
# number whose meaning depends on which run produced it is listed here with
# the file that must contain it. See D23.
# Entries are added only once the run that produced the number is committed.
# Two were added here from a diagnostic that was about to be rerun, which
# would have frozen a figure that the rerun changes; the manuscript states
# the shape of that result in words instead, and a number goes in only when
# the file holding it is in the repository.
CITED: dict[str, str] = {
    # The wait-strategy result. Each of these is emitted into that file by
    # scripts/h88_verdicts.py --emit at the precision quoted here.
    "0.9661": "2026-11-23_m4_h88_waitstalls.txt",
    "0.0542": "2026-11-23_m4_h88_waitstalls.txt",
    "0.4161": "2026-11-23_m4_h88_waitstalls.txt",
    "0.3849": "2026-11-23_m4_h88_waitstalls.txt",
    "0.3945": "2026-11-23_m4_h88_waitstalls.txt",
    "0.4532": "2026-11-23_m4_h88_waitstalls.txt",
    "0.3511": "2026-11-23_m4_h88_waitstalls.txt",
    "1.0809": "2026-11-23_m4_h88_waitstalls.txt",
}

PERCENT: dict[str, tuple[str, str]] = {
    # The issue-rate split, each computed from the medians in the H86 run.
    "7.77": ("measured", "2026-11-22_m4_h86_issue.txt"),
    "0.16": ("measured", "2026-11-22_m4_h86_issue.txt"),
    "92.07": ("measured", "2026-11-22_m4_h86_issue.txt"),
    "95.3": ("measured", "2026-11-17_m4_barrier.txt"),
    "2.4": ("measured", "2026-11-20_m4_burst5_rebuild.txt"),
    "0.7": ("measured", "2026-11-20_m4_burst5_rebuild.txt"),
    "17": ("measured", "2026-11-19_m4_burst10_saturated.txt"),
    "12": ("measured", "2026-11-19_m4_burst10_saturated.txt"),
    "8": ("cite", "emurian2014pitfalls"),
    "5.8": ("cite", "emurian2014pitfalls"),
    "3": ("cite", "khayet2026arm"),
    "95": ("cite", "khayet2026arm"),
    "1": ("cite", "khayet2026arm"),
    "8.94": ("cite", "wrenger2024m1"),
    "7.3": ("measured", "2026-11-21_m4_h77_cycles.txt"),
    "0.0317": ("measured", "2026-11-21_m4_probe.txt"),
    "2.1": ("measured", "2026-11-21_m4_h77_cycles.txt"),
    "1.8": ("measured", "2026-11-21_m4_h77_cycles.txt"),
}

# Numbers that are part of the typesetting rather than the science. Each needs
# a reason, so the list cannot quietly become a place to hide a figure.
TYPESETTING: dict[str, str] = {}


def manuscript_numbers() -> list[tuple[int, str]]:
    """Every decimal number in the body, with its line. Comments excluded."""
    out: list[tuple[int, str]] = []
    for n, line in enumerate(TEX.read_text(encoding="utf-8").splitlines(), 1):
        if line.lstrip().startswith("%"):
            continue
        # Drop trailing comments, but not an escaped percent sign.
        line = re.sub(r"(?<!\\)%.*$", "", line)
        for m in re.finditer(r"(?<![\w.])\d+\.\d+(?![\w.])", line):
            out.append((n, m.group(0)))
    return out


def results_corpus() -> dict[str, str]:
    return {
        os.path.basename(f): Path(f).read_text(encoding="utf-8", errors="replace")
        for f in sorted(glob.glob(str(ROOT / "results" / "*.txt")))
    }


def find_measured(value: str, corpus: dict[str, str]) -> list[str]:
    """Files containing the value.

    A trailing unit is allowed: the results files write ratios as 16.33x and
    percentages as 95.3 percent. Requiring a non-word character after the
    number would reject the first of those, which is how an earlier version of
    this check reported a sourced figure as unsourced.
    """
    pat = re.compile(r"(?<![\w.])" + re.escape(value) + r"(?![\d.])")
    return [name for name, text in corpus.items() if pat.search(text)]


def preliminary_only(value: str) -> bool:
    """True when every experiment row mentioning the value is preliminary."""
    rows = [
        line
        for line in EXPERIMENTS.read_text(encoding="utf-8").splitlines()
        if re.match(r"^\| [HE]\d+[a-c]? \|", line)
        and re.search(r"(?<![\w.])" + re.escape(value) + r"(?![\d.])", line)
    ]
    if not rows:
        return False
    # The flag on a row is the phrase followed by a colon introducing the
    # explanation. H65 and H67 use the same phrase as a noun, describing
    # preliminary figures they re-measured under the launch protocol, and an
    # earlier version of this check counted those as flagged and reported a
    # launch-protocol figure as preliminary.
    return all("PRELIMINARY PROTOCOL:" in r for r in rows)


def check_cited(tex: str, res_dir, broken: list) -> set:
    """Every run specific number must appear in the run that produced it."""
    absent: list = []
    for num, fname in sorted(CITED.items()):
        if num not in tex:
            continue
        target = res_dir / fname
        if not target.exists():
            # A run committed from the machine that produced it is not present
            # in a checkout that has not fetched it. That is not a false claim
            # and it is not a verification either, so it is named rather than
            # passed or failed, the same treatment check_power_lines gives an
            # absent run.
            absent.append(f"{num} is cited to {fname}, which is not in this checkout")
            continue
        body = target.read_text(encoding="utf-8", errors="replace")
        if num not in body:
            broken.append(f"{num} is quoted but {fname}, the run it is cited from, "
                          f"does not contain it")
    print()
    print(f"  run specific numbers: {sum(1 for n in CITED if n in tex) - len(absent)} "
          f"checked against the file each is cited from")
    for a in absent:
        print(f"    NOT VERIFIED HERE: {a}")
    return {a.split()[0] for a in absent}


def main() -> int:
    if not TEX.exists():
        print(f"no manuscript at {TEX}, nothing to check")
        return 0
    corpus = results_corpus()
    bib = BIB.read_text(encoding="utf-8") if BIB.exists() else ""

    seen: dict[str, list[int]] = {}
    for line, value in manuscript_numbers():
        seen.setdefault(value, []).append(line)

    measured: list[tuple[str, str]] = []
    derived: list[tuple[str, str]] = []
    external: list[tuple[str, str]] = []
    unsourced: list[tuple[str, list[int]]] = []
    broken: list[str] = []
    prelim: list[str] = []

    for value in sorted(seen, key=float):
        if value in DERIVED:
            expr, places = DERIVED[value]
            try:
                got = round(eval(expr, {"__builtins__": {}}, {}), places)  # noqa: S307
            except Exception as exc:
                broken.append(f"{value}: derivation '{expr}' did not evaluate: {exc}")
                continue
            if f"{got:.{places}f}" != value:
                broken.append(
                    f"{value}: derivation '{expr}' gives {got:.{places}f}, "
                    f"which is not what the manuscript prints"
                )
                continue
            # The inputs of a derivation have to be measured themselves.
            for operand in re.findall(r"\d+\.\d+", expr):
                if not find_measured(operand, corpus):
                    broken.append(
                        f"{value}: derivation input {operand} is itself in no results file"
                    )
            derived.append((value, expr))
            continue
        if value in EXTERNAL:
            key = EXTERNAL[value]
            if f"{{{key}," not in bib.replace(" ", ""):
                broken.append(f"{value}: cited to '{key}', which is not in refs.bib")
                continue
            external.append((value, key))
            continue
        if value in TYPESETTING:
            continue
        files = find_measured(value, corpus)
        if files:
            measured.append((value, files[0]))
            if preliminary_only(value):
                prelim.append(value)
        else:
            unsourced.append((value, seen[value]))

    total = len(seen)
    print(f"manuscript asserts {total} distinct numbers")
    print(f"  measured in results/ : {len(measured)}")
    print(f"  derived and verified : {len(derived)}")
    print(f"  quoted from a source : {len(external)}")
    print(f"  UNSOURCED            : {len(unsourced)}")
    print()
    for value, expr in derived:
        print(f"  derived  {value:>9}  = {expr}")
    for value, key in external:
        print(f"  external {value:>9}  cited to {key}")
    if prelim:
        print()
        print("  measured, but every experiment row mentioning it is PRELIMINARY")
        print("  PROTOCOL. Not a failure, and worth knowing before submission:")
        for v in prelim:
            print(f"    {v}")
    # Every percentage in the manuscript must declare a source that exists.
    pct = set(re.findall(r"([0-9][0-9.]*) percent", re.sub(r"\s+", " ", TEX.read_text(encoding="utf-8"))))
    bibkeys = set(re.findall(r"@\w+\{(\w+),", BIB.read_text(encoding="utf-8")))
    for p in sorted(pct):
        if p not in PERCENT:
            broken.append(f"{p} percent is in the manuscript and not declared in PERCENT")
            continue
        kind, src = PERCENT[p]
        if kind == "cite" and src not in bibkeys:
            broken.append(f"{p} percent is declared as cited to {src}, absent from refs.bib")
        if kind == "measured" and not (ROOT / "results" / src).exists():
            broken.append(f"{p} percent is declared as measured in {src}, which does not exist")
    print()
    print(f"  percentages: {len(pct)} in the manuscript, each with a declared source")

    unverifiable = check_cited(TEX.read_text(encoding="utf-8"), ROOT / "results", broken)
    # A number whose own run is absent from this checkout cannot be in the
    # corpus either, so it is not reported twice.
    unsourced = [(v, l) for v, l in unsourced if v not in unverifiable]

    if unsourced:
        print()
        print("  UNSOURCED. These appear in the manuscript and in no results file,")
        print("  and are not declared as derived, cited or typesetting:")
        for value, lines in unsourced:
            print(f"    {value}  main.tex line(s) {', '.join(map(str, lines))}")


    if broken:
        print()
        print("  BROKEN DECLARATIONS:")
        for b in broken:
            print(f"    {b}")

    bad = len(unsourced) + len(broken)
    print()
    print(f"{'PASS' if bad == 0 else 'FAIL'}: {bad} number(s) unaccounted for")
    return 1 if bad else 0


if __name__ == "__main__":
    sys.exit(main())
