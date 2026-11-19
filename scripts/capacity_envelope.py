"""Saturation and achieved rate from `pl` lines, by arm, rate and run length.

Reads the raw output of a titan-pipeline sweep and answers two questions the
CCDF cannot answer once a run is excluded: what fraction of cells saturated,
and what rate the machine actually achieved. Saturated cells carry no CCDF by
design, so without this the only record of an overloaded sweep is that it
produced nothing.

Usage:
    python3 -I scripts/capacity_envelope.py RAWFILE [RAWFILE ...]
"""

import re
import sys
from collections import defaultdict


def parse(paths):
    rows = []
    for p in paths:
        for line in open(p, encoding="utf-8", errors="replace"):
            parts = line.split()
            if not parts or parts[0] != "pl":
                continue
            d = {}
            for kv in parts[1:]:
                if "=" in kv:
                    k, v = kv.split("=", 1)
                    d[k] = v
            if "load" not in d or "saturated" not in d:
                continue
            rows.append(d)
    return rows


def family(load):
    """Arm name with the rate stripped out of the periodic arm.

    The periodic arm is named `rate_<R>`, so its name changes with every rate
    in a ladder. Comparing raw names across rates therefore never finds the
    same arm set twice, which silently made the fully-unsaturated search below
    report `none of the rates tested` for every run length.
    """
    return "periodic" if re.fullmatch(r"rate_\d+", load) else load


def f(d, k):
    try:
        return float(d[k])
    except (KeyError, ValueError):
        return None


def median(xs):
    xs = sorted(xs)
    n = len(xs)
    if n == 0:
        return None
    return xs[n // 2] if n % 2 else (xs[n // 2 - 1] + xs[n // 2]) / 2.0


def main():
    if len(sys.argv) < 2:
        print(__doc__)
        return 2
    rows = parse(sys.argv[1:])
    if not rows:
        print("FAIL: no pl lines found")
        return 1

    print(f"pl lines: {len(rows)}")
    sat = sum(1 for r in rows if r["saturated"] == "1")
    print(f"saturated: {sat} of {len(rows)} ({100.0 * sat / len(rows):.1f}%)")
    print()

    # By arm, which is what H71 (a) is stated over.
    print("BY ARM")
    print(f"  {'arm':<18} {'cells':>6} {'saturated':>10} {'share':>8} "
          f"{'med_offered':>12} {'med_achieved':>13} {'med_ratio':>10}")
    by_arm = defaultdict(list)
    for r in rows:
        by_arm[family(r["load"])].append(r)
    for arm in sorted(by_arm):
        rs = by_arm[arm]
        s = sum(1 for r in rs if r["saturated"] == "1")
        off = [f(r, "offered_mps") for r in rs if f(r, "offered_mps") is not None]
        ach = [f(r, "achieved_mps") for r in rs if f(r, "achieved_mps") is not None]
        rat = [f(r, "achieved_mps") / f(r, "offered_mps") for r in rs
               if f(r, "offered_mps") and f(r, "achieved_mps") is not None]
        print(f"  {arm:<18} {len(rs):>6} {s:>10} {100.0 * s / len(rs):>7.1f}% "
              f"{median(off):>12.3f} {median(ach):>13.3f} {median(rat):>10.4f}")
    print()

    # By arm and run length, which is what separates a rate ceiling from a
    # run-length ceiling.
    print("BY ARM AND RUN LENGTH (messages per epoch)")
    print(f"  {'arm':<18} {'messages':>10} {'offered':>9} {'cells':>6} "
          f"{'sat_share':>10} {'med_ratio':>10}")
    by_cell = defaultdict(list)
    for r in rows:
        by_cell[(family(r["load"]), r.get("messages", "?"), r.get("offered_mps", "?"))].append(r)
    for key in sorted(by_cell, key=lambda k: (k[0], float(k[1]) if k[1].isdigit() else 0)):
        rs = by_cell[key]
        s = sum(1 for r in rs if r["saturated"] == "1")
        rat = [f(r, "achieved_mps") / f(r, "offered_mps") for r in rs
               if f(r, "offered_mps") and f(r, "achieved_mps") is not None]
        m = median(rat)
        print(f"  {key[0]:<18} {key[1]:>10} {key[2]:>9} {len(rs):>6} "
              f"{100.0 * s / len(rs):>9.1f}% {m if m is None else f'{m:10.4f}'}")
    print()

    # The highest offered rate at which EVERY arm stays unsaturated, per run
    # length. This is the number a later registration needs.
    print("HIGHEST FULLY UNSATURATED OFFERED RATE, BY RUN LENGTH")
    lengths = sorted({r.get("messages", "?") for r in rows},
                     key=lambda m: float(m) if m.replace(".", "").isdigit() else 0)
    for msgs in lengths:
        here = [r for r in rows if r.get("messages") == msgs]
        arms = {family(r["load"]) for r in here}
        best = None
        rates = sorted({float(r["offered_mps"]) for r in here if f(r, "offered_mps")})
        for rate in rates:
            cells = [r for r in here if abs(f(r, "offered_mps") - rate) < 0.05]
            if {family(r["load"]) for r in cells} != arms:
                continue
            if all(r["saturated"] == "0" for r in cells):
                best = rate
        label = "none of the rates tested" if best is None else f"{best:.3f} Mmsg/s"
        print(f"  messages={msgs:<10} arms={len(arms)}  highest clean offered rate: {label}")
    return 0


if __name__ == "__main__":
    sys.exit(main())
