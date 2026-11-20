"""Medians and inversion ratios for the barrier cross, from the raw records.

The published barrier figures were, until 2026-11-20, available only as a
summary: the records they came from had been aggregated and discarded, so no
median or ratio behind the paper's first three contributions could be checked.
See docs/defects.md D4. This file closes the path from record to number. It
reads the retained records and prints every statistic the manuscript quotes.

Usage:
    python3 -I scripts/barrier_stats.py results/2026-11-20_m4_barrier_raw.txt
"""

import re
import sys
from collections import defaultdict
from pathlib import Path

PAIR = re.compile(r"([a-z_0-9]+)=([^\s]+)")
CORRECT = ("acqrel", "fenced", "fencedload", "fencedstore")


def median(xs):
    s = sorted(xs)
    n = len(s)
    return s[n // 2] if n % 2 else 0.5 * (s[n // 2 - 1] + s[n // 2])


def main():
    if len(sys.argv) < 2:
        print(__doc__)
        return 2
    path = Path(sys.argv[1])
    cells = defaultdict(list)
    okc = defaultdict(lambda: [0, 0])
    waits = set()
    for line in path.open(encoding="utf-8"):
        if not line.startswith("bd launch="):
            continue
        r = dict(PAIR.findall(line))
        cells[(r["order"], r["cached"])].append(float(r["ns_per_msg"]))
        waits.add(r.get("wait", "?"))
        okc[r["order"]][0] += 1
        okc[r["order"]][1] += r.get("sum_ok") == "1"
    if not cells:
        print("FAIL: no bd records found")
        return 1

    total = sum(len(v) for v in cells.values())
    print(f"bar_meta file={path.name} records={total} waits={','.join(sorted(waits))}")
    for arm in CORRECT + ("unordered",):
        for cached, tag in (("0", "contended"), ("1", "resident")):
            v = cells.get((arm, cached))
            if v:
                print(f"bar_med order={arm} cached={cached} config={tag} "
                      f"n={len(v)} median_ns_per_msg={median(v):.3f}")
    for arm in CORRECT + ("unordered",):
        n, good = okc[arm]
        print(f"bar_ok order={arm} records={n} sum_ok_1={good} sum_ok_0={n - good}")

    cont = sorted(CORRECT, key=lambda a: median(cells[(a, "0")]))
    res = sorted(CORRECT, key=lambda a: median(cells[(a, "1")]))
    print("bar_rank config=contended order=" + ",".join(cont))
    print("bar_rank config=resident order=" + ",".join(res))

    a = median(cells[("acqrel", "0")]) / median(cells[("fencedstore", "0")])
    b = median(cells[("fencedstore", "1")]) / median(cells[("acqrel", "1")])
    print(f"bar_ratio name=contended_inversion numerator=acqrel denominator=fencedstore value={a:.4f}")
    print(f"bar_ratio name=resident_inversion numerator=fencedstore denominator=acqrel value={b:.4f}")
    return 0


if __name__ == "__main__":
    sys.exit(main())
