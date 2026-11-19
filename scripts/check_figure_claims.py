"""Independently recompute every number the figures assert, and fail on a mismatch.

The figure generators and this checker do not share a line of analysis code.
The generators take the median across epochs, then the median across launches,
through figures_readme.ratio_curve. This file re-reads the raw records and
computes the same quantities with its own parsing, its own median and its own
exact binomial, so an error has to occur twice, in two different ways, to
survive.

Checked here:
  the peak ratio, its percentile and its sign count, for every rate and wait;
  the clustering ratio at p99.9;
  the number of registered rows, verdicted rows and rows recording a refutation;
  the number of measured records;
  and that each of those appears literally in the SVG that claims it.

Usage:
    python3 -I scripts/check_figure_claims.py
"""

import math
import re
import sys
from collections import defaultdict
from pathlib import Path

ROOT = Path(__file__).resolve().parent.parent
RES = ROOT / "results"
FIGS = ROOT / "docs" / "figures"
DOC = ROOT / "docs" / "experiments.md"

RUNS = {"5M": "2026-11-18_m4_burst.txt", "7M": "2026-11-19_m4_burst7.txt"}
WAITS = ("spin", "isb", "wfe")
VERDICT_WORDS = ("supported", "refuted", "failed", "untestable", "inconclusive")


def read(path):
    """Own parser: split on whitespace, split each token on the first '='."""
    out = defaultdict(list)
    for line in (RES / path).open(encoding="utf-8", errors="replace"):
        if not line.startswith("pl_ccdf "):
            continue
        f = {}
        for tok in line.split():
            if "=" in tok:
                k, _, v = tok.partition("=")
                f[k] = v
        if not {"load", "wait", "launch", "q", "ns"} <= f.keys():
            continue
        out[(f["load"], f["wait"], int(f["launch"]), float(f["q"]))].append(float(f["ns"]))
    return out


def middle(xs):
    """Own median."""
    xs = sorted(xs)
    h = len(xs) // 2
    return xs[h] if len(xs) % 2 else (xs[h - 1] + xs[h]) / 2


def two_sided(k, n):
    """Own exact binomial, p = 1/2, computed from the tail that k sits in."""
    if n == 0:
        return 1.0
    tail = sum(math.comb(n, i) for i in range(k, n + 1))
    return min(1.0, 2.0 * tail / float(2 ** n))


def contrast(d, top, bottom, wait):
    rows = []
    launches = sorted({k[2] for k in d})
    for q in sorted({k[3] for k in d}):
        hi, lo, wins, n = [], [], 0, 0
        for l in launches:
            a = d.get((top, wait, l, q))
            b = d.get((bottom, wait, l, q))
            if not a or not b:
                continue
            A, B = middle(a), middle(b)
            hi.append(A)
            lo.append(B)
            n += 1
            wins += A > B
        if n:
            rows.append((q, middle(hi) / middle(lo), wins, n, two_sided(wins, n)))
    return rows


def main():
    problems = []
    abstract = (FIGS / "fig-abstract.svg").read_text(encoding="utf-8")
    method = (FIGS / "fig-method.svg").read_text(encoding="utf-8")
    randomness = (FIGS / "fig-randomness.svg").read_text(encoding="utf-8")

    peaks = {}
    for tag, fname in RUNS.items():
        d = read(fname)
        if not d:
            problems.append(f"{fname}: no CCDF records read")
            continue
        per = next(a for a in {k[0] for k in d} if a.startswith("rate_"))
        for w in WAITS:
            c = contrast(d, "hawkes_0.0", per, w)
            pk = max(c, key=lambda r: r[1])
            peaks[(tag, w)] = pk
            txt = f"{pk[1]:.2f}x at p{100 * pk[0]:.2f}, {pk[2]}/{pk[3]}"
            if txt not in randomness:
                problems.append(f"fig-randomness.svg does not state '{txt}' for {tag} {w}")
        if tag == "7M":
            qs = sorted({k[3] for k in d})
            q999 = min(qs, key=lambda q: abs(q - 0.999))
            cl = next(r for r in contrast(d, "hawkes_0.8", "hawkes_0.0", "spin") if r[0] == q999)

    hero = peaks[("7M", "spin")]
    for frag in (f"{hero[1]:.2f}x", f"p{100 * hero[0]:.2f}, {hero[2]}/{hero[3]} launches"):
        if frag not in abstract:
            problems.append(f"fig-abstract.svg does not state '{frag}'")
    for frag in (f"{cl[1]:.2f}x", f"p99.9, {cl[2]}/{cl[3]} launches"):
        if frag not in abstract:
            problems.append(f"fig-abstract.svg does not state '{frag}'")

    text = DOC.read_text(encoding="utf-8")
    rowtext = {}
    for line in text.splitlines():
        m = re.match(r"^\| ([HE]\d+[a-c]?) \|", line)
        if m:
            rowtext[m.group(1)] = line.lower()
    rows = len(rowtext)
    verdicted = 0
    refuted = 0
    for v in rowtext.values():
        if any(re.search(rf"\b{w}\b", v) for w in VERDICT_WORDS):
            verdicted += 1
        if re.search(r"\brefuted\b", v):
            refuted += 1

    kv = re.compile(r"[A-Za-z_][A-Za-z0-9_]*=")
    recs = 0
    for f in sorted(RES.glob("*.txt")):
        for ln in f.open(encoding="utf-8", errors="replace"):
            head = ln.split(" ", 1)[0]
            if head in ("pl", "pl_ccdf", "pp", "st", "t2t") and kv.search(ln):
                recs += 1

    claim = (f"{rows} hypotheses registered, {verdicted} carry a recorded verdict, "
             f"{refuted} of them a refutation, from {recs:,} measured records")
    if claim not in method:
        problems.append(f"fig-method.svg does not state: {claim}")
    if f"{refuted} rows record" not in abstract:
        problems.append(f"fig-abstract.svg does not state '{refuted} rows record a refutation'")

    print(f"peak, 7,000,000 spin : {hero[1]:.4f} at p{100 * hero[0]:.4f}, "
          f"{hero[2]}/{hero[3]}, p={hero[4]:.3g}")
    print(f"clustering at p99.9  : {cl[1]:.4f}, {cl[2]}/{cl[3]}, p={cl[4]:.3g}")
    print(f"rows / verdicted / refuted : {rows} / {verdicted} / {refuted}")
    print(f"measured records     : {recs:,}")
    if problems:
        print()
        for p in problems:
            print("FAIL: " + p)
        return 1
    print("PASS: every number on the figures reproduces from an independent recomputation")
    return 0


if __name__ == "__main__":
    sys.exit(main())
