"""H77 verdicts: the cycle-level replication of the barrier cross.

Thresholds were fixed and pushed before this data existed. This file reads the
raw records with its own parsing and its own median, and reports each registered
part without reference to what the manuscript says.

    python3 -I scripts/h77_verdicts.py [--emit]
"""

import math
import sys
from collections import defaultdict
from pathlib import Path

ROOT = Path(__file__).resolve().parent.parent
CYCLES = sorted((ROOT / "results").glob("*_m4_h77_cycles.txt"))
NS = ROOT / "results" / "2026-11-20_m4_barrier_raw.txt"
ORDERS = ("acqrel", "fenced", "fencedload", "fencedstore", "unordered")
CORRECT = ("acqrel", "fenced", "fencedload", "fencedstore")


def fields(line):
    f = {}
    for tok in line.split():
        if "=" in tok:
            k, _, v = tok.partition("=")
            f[k] = v
    return f


def med(xs):
    xs = sorted(xs)
    h = len(xs) // 2
    return xs[h] if len(xs) % 2 else (xs[h - 1] + xs[h]) / 2.0


def sign_p(k, n):
    if n == 0:
        return 1.0
    tail = sum(math.comb(n, i) for i in range(k, n + 1))
    return min(1.0, 2.0 * tail / float(2 ** n))


def read(path, key):
    """key -> (order, cached, launch) -> list of values"""
    d = defaultdict(list)
    ok = defaultdict(lambda: [0, 0])
    for line in path.open(encoding="utf-8", errors="replace"):
        if not line.startswith("bd "):
            continue
        f = fields(line)
        if "order" not in f or key not in f:
            continue
        o, c = f["order"], f.get("cached", "?")
        d[(o, c, f.get("launch", "0"))].append(float(f[key]))
        if "sum_ok" in f:
            ok[o][0] += int(f["sum_ok"] == "1")
            ok[o][1] += 1
    return d, ok


def per_launch(d, order, cached):
    """median over epochs, per launch"""
    out = {}
    for (o, c, l), v in d.items():
        if o == order and c == cached:
            out[l] = med(v)
    return out


def ratio_test(d, top, bottom, cached):
    a = per_launch(d, top, cached)
    b = per_launch(d, bottom, cached)
    ls = sorted(set(a) & set(b))
    wins = sum(1 for l in ls if a[l] > b[l])
    return med([a[l] for l in ls]) / med([b[l] for l in ls]), wins, len(ls), sign_p(wins, len(ls))


def main():
    if not CYCLES:
        print("FAIL: no results/*_m4_h77_cycles.txt found")
        return 1
    path = CYCLES[-1]
    cyc, ok = read(path, "msg_cycles")
    ghz, _ = read(path, "core_ghz")
    ns, _ = read(NS, "ns_per_msg")
    print(f"cycles file : {path.name}")
    print(f"ns file     : {NS.name}")
    print(f"launches    : {len({k[2] for k in cyc})}")
    print()

    v = {}

    print("THE INVERSION, IN CYCLES PER MESSAGE")
    print(f"  {'state':<12} {'comparison':<28} {'ratio':>8} {'wins':>8} {'p':>12}")
    r0, w0, n0, p0 = ratio_test(cyc, "acqrel", "fencedstore", "0")
    r1, w1, n1, p1 = ratio_test(cyc, "fencedstore", "acqrel", "1")
    print(f"  {'contended':<12} {'acqrel / fencedstore':<28} {r0:>8.4f} {w0:>5}/{n0:<3} {p0:>12.3g}")
    print(f"  {'resident':<12} {'fencedstore / acqrel':<28} {r1:>8.4f} {w1:>5}/{n1:<3} {p1:>12.3g}")
    print()
    v["a"] = (r0 >= 2.0 and w0 >= 21,
              f"contended cycle ratio {r0:.4f} (threshold 2.0), sign {w0} of {n0} "
              f"(threshold 21), p={p0:.3g}")
    v["b"] = (r1 >= 2.0 and w1 >= 21,
              f"resident cycle ratio {r1:.4f} (threshold 2.0), sign {w1} of {n1} "
              f"(threshold 21), p={p1:.3g}. The comparison changes sign in cycles.")

    print("RANKING BY MEDIAN, CYCLES AGAINST NANOSECONDS")
    same = True
    for c in ("0", "1"):
        rc = sorted(ORDERS, key=lambda o: med([x for (oo, cc, _), vs in cyc.items()
                                               if oo == o and cc == c for x in vs]))
        rn = sorted(ORDERS, key=lambda o: med([x for (oo, cc, _), vs in ns.items()
                                               if oo == o and cc == c for x in vs]))
        print(f"  cached={c} cycles : {' < '.join(rc)}")
        print(f"  cached={c} ns     : {' < '.join(rn)}")
        same = same and rc == rn
    v["c"] = (same, "the orders rank identically in cycles and in nanoseconds at both "
              "caching states" if same else "THE RANKINGS DIFFER between cycles and nanoseconds")
    print()

    print("THE DISCRIMINATING THRESHOLD: core frequency across arms")
    print(f"  {'state':<10} {'lowest GHz':>12} {'highest GHz':>12} {'spread':>8}")
    worst = 0.0
    for c in ("0", "1"):
        g = {o: med([x for (oo, cc, _), vs in ghz.items() if oo == o and cc == c for x in vs])
             for o in ORDERS}
        lo, hi = min(g.values()), max(g.values())
        worst = max(worst, hi / lo)
        print(f"  cached={c:<3} {lo:>12.3f} {hi:>12.3f} {hi / lo:>8.4f}")
    v["d"] = (worst <= 1.15,
              f"worst frequency spread across the five orders {worst:.4f} (threshold 1.15). "
              "This is the assumption H76 could not verify and the paper's first three "
              "contributions rest on.")
    print()

    bad = [f"{o} {ok[o][0]} of {ok[o][1]}" for o in CORRECT if ok[o][0] != ok[o][1]]
    u = ok["unordered"]
    v["e"] = (not bad and u[0] == 0,
              f"correct rings all pass ({', '.join(f'{o} {ok[o][0]}/{ok[o][1]}' for o in CORRECT)}), "
              f"unordered {u[0]} of {u[1]}" if not bad else "; ".join(bad))

    if "--emit" in sys.argv:
        print()
        print(f"h77_inversion state=contended ratio={r0:.4f} wins={w0} n={n0} p={p0:.3g}")
        print(f"h77_inversion state=resident ratio={r1:.4f} wins={w1} n={n1} p={p1:.3g}")
        for c in ("0", "1"):
            g = {o: med([x for (oo, cc, _), vs in ghz.items() if oo == o and cc == c for x in vs])
                 for o in ORDERS}
            print(f"h77_freq cached={c} low_ghz={min(g.values()):.3f} "
                  f"high_ghz={max(g.values()):.3f} spread={max(g.values()) / min(g.values()):.4f}")
            for o in ORDERS:
                mc = med([x for (oo, cc, _), vs in cyc.items() if oo == o and cc == c for x in vs])
                print(f"h77_arm cached={c} order={o} msg_cycles={mc:.3f} core_ghz={g[o]:.3f}")
        return 0

    print("REGISTERED VERDICTS")
    for k in "abcde":
        okk, why = v[k]
        print(f"  ({k}) {'SUPPORTED' if okk else 'REFUTED  '}  {why}")
    print()
    print(f"{sum(1 for k in 'abcde' if v[k][0])} of 5 registered parts supported")
    return 0


if __name__ == "__main__":
    sys.exit(main())
