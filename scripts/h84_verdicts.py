"""H84: does the comparison change sign with ring capacity.

Thresholds were fixed and pushed before this data existed. The lever check is
a guard rather than an observation, which is the D29 correction: H82 chose a
lever that moved one arm and discovered it in the analysis.

    python3 -I scripts/h83_verdicts.py [--emit]
"""

import math
import sys
from collections import defaultdict
from pathlib import Path

ROOT = Path(__file__).resolve().parent.parent
FILE = ROOT / "results" / "2026-11-22_m4_h84_capfine.txt"
ARMS = ("acqrel", "fencedstore")
CAPS = ("2", "4", "8", "16", "1024")


def fields(line):
    return {k: v for k, _, v in (t.partition("=") for t in line.split()) if k}


def med(xs):
    xs = sorted(xs)
    h = len(xs) // 2
    return xs[h] if len(xs) % 2 else (xs[h - 1] + xs[h]) / 2.0


def sign_p(k, n):
    if n == 0:
        return 1.0
    k = max(k, n - k)
    return min(1.0, 2.0 * sum(math.comb(n, i) for i in range(k, n + 1)) / float(2 ** n))


def fit(xs, ys):
    n = len(xs)
    mx, my = sum(xs) / n, sum(ys) / n
    den = sum((x - mx) ** 2 for x in xs)
    return None if den == 0 else (
        sum((x - mx) * (y - my) for x, y in zip(xs, ys)) / den,
        my - sum((x - mx) * (y - my) for x, y in zip(xs, ys)) / den * mx)


def main():
    if not FILE.exists():
        print(f"FAIL: {FILE.name} not found")
        return 1
    per = defaultdict(lambda: defaultdict(list))
    byl = defaultdict(lambda: defaultdict(list))
    ok = defaultdict(lambda: [0, 0])
    arms_mode = counters = side = None
    for line in FILE.open(encoding="utf-8", errors="replace"):
        if line.startswith("bd_config"):
            f = fields(line)
            arms_mode = arms_mode or f.get("arms")
            counters = counters or f.get("counters")
            side = side or f.get("measured_side")
        if not line.startswith("bd ") or "order=" not in line:
            continue
        f = fields(line)
        if f.get("cached") != "0":
            continue
        key = (f["order"], f.get("cap", "?"))
        for k in ("msg_cycles", "msg_l1d_writeback", "msg_l1d_refill",
                  "msg_ldst_wait_cycles", "ns_per_msg"):
            if k in f:
                per[key][k].append(float(f[k]))
                byl[(key, f.get("launch", "0"))][k].append(float(f[k]))
        ok[key][0] += int(f.get("sum_ok") == "1")
        ok[key][1] += 1

    print(f"file     : {FILE.name}")
    print(f"arms={arms_mode}  counters={counters}  measured_side={side}")
    if arms_mode != "capfine":
        print(f"FAIL: H83 requires arms=capfine, file says {arms_mode}")
        return 1
    if counters != "trafficprobe":
        print(f"FAIL: H83 requires counters=trafficprobe, file says {counters}")
        return 1
    print()
    print(f"  {'arm':<13} {'cap':>5} {'cycles':>10} {'wrbk':>8} {'refill':>8} "
          f"{'wrbk/cyc':>9} {'ns/msg':>9} {'ok':>9}")
    cells = []
    for a in ARMS:
        for cp in CAPS:
            q = per[(a, cp)]
            if not q:
                continue
            cyc, wb = med(q["msg_cycles"]), med(q["msg_l1d_writeback"])
            rf = med(q["msg_l1d_refill"])
            cells.append((a, cp, cyc, wb, rf))
            print(f"  {a:<13} {cp:>5} {cyc:>10.3f} {wb:>8.4f} {rf:>8.4f} "
                  f"{wb / cyc:>9.6f} {med(q['ns_per_msg']):>9.3f} "
                  f"{ok[(a, cp)][0]:>4}/{ok[(a, cp)][1]:<4}")
    print()

    v = {}
    v["a"] = (len(cells) == 10 and all(wb > 0 and rf > 0 for _, _, _, wb, rf in cells),
              f"{len(cells)} of 10 cells, "
              f"{sum(1 for _, _, _, wb, rf in cells if wb > 0 and rf > 0)} with both "
              f"writebacks and refills above zero")

    def launch_sign(cp, acq_faster):
        w = n = 0
        for L in sorted({l for (_k, l) in byl}):
            qa = byl[(("acqrel", cp), L)]
            qf = byl[(("fencedstore", cp), L)]
            if not qa or not qf:
                continue
            n += 1
            ca, cf = med(qa["msg_cycles"]), med(qf["msg_cycles"])
            w += (ca < cf) if acq_faster else (ca > cf)
        return w, n

    cyc = {(a, cp): c for a, cp, c, _, _ in cells}
    lo = cyc.get(("acqrel", "2")), cyc.get(("fencedstore", "2"))
    hi = cyc.get(("acqrel", "1024")), cyc.get(("fencedstore", "1024"))
    if None in lo or None in hi:
        v["b"] = (None, "UNEVALUABLE, capacity 2 or 1024 is missing")
    else:
        w2, n2 = launch_sign("2", True)
        w3, n3 = launch_sign("1024", False)
        v["b"] = (lo[0] < lo[1] and hi[0] > hi[1] and w2 >= 17 and w3 >= 17,
                  f"at capacity 2 acqrel {lo[0]:.3f} against fencedstore {lo[1]:.3f} "
                  f"cycles, acqrel faster in {w2} of {n2}, p={sign_p(w2, n2):.3g}; at "
                  f"capacity 1024 acqrel {hi[0]:.3f} against {hi[1]:.3f}, acqrel slower "
                  f"in {w3} of {n3}, p={sign_p(w3, n3):.3g}; threshold 17 of 20 each")

    mid = [cp for cp in ("4", "8", "16")
           if cyc.get(("acqrel", cp)) and cyc.get(("fencedstore", cp))
           and cyc[("acqrel", cp)] > cyc[("fencedstore", cp)]]
    v["c"] = (bool(mid),
              f"capacities with fencedstore faster at the median among 4, 8 and 16: "
              f"{mid if mid else 'none'}, so the sign changes "
              f"{'at or below 16' if mid else 'nowhere in the bracket, and the crossover is above 16'}")

    rates, worst, det = {}, 0.0, []
    for a, cp, c, wb, _ in cells:
        rates[(a, cp)] = wb / c
    for cp in CAPS:
        ra, rf = rates.get(("acqrel", cp)), rates.get(("fencedstore", cp))
        if ra is None or rf is None or min(ra, rf) == 0:
            continue
        s2 = max(ra, rf) / min(ra, rf)
        worst = max(worst, s2)
        det.append(f"cap {cp} {rf / ra:.4f}")
    v["d"] = (bool(det) and worst <= 1.50,
              f"writebacks per cycle, fencedstore over acqrel at matched capacity: "
              + ", ".join(det) + f"; largest departure from unity {worst:.4f}, band 1.50")

    bad = [f"{a} cap {cp} {ok[(a, cp)][0]}/{ok[(a, cp)][1]}"
           for a in ARMS for cp in CAPS
           if per[(a, cp)] and ok[(a, cp)][0] != ok[(a, cp)][1]]
    v["e"] = (not bad, "; ".join(bad) if bad else
              "every record of all ten cells reports a correct sum")

    if "--emit" in sys.argv:
        for a, cp, c, wb, rf in cells:
            print(f"h83_cell arm={a} cap={cp} msg_cycles={c:.3f} writeback={wb:.4f} "
                  f"refill={rf:.4f} writeback_per_cycle={wb / c:.6f}")
        return 0

    print("REGISTERED VERDICTS, guards before the claims they gate")
    for k in "abcde":
        o, why = v[k]
        tag = "UNEVALUABLE" if o is None else ("SUPPORTED  " if o else "REFUTED    ")
        print(f"  ({k}) {tag}  {why}")
    s = sum(1 for k in v if v[k][0] is True)
    r = sum(1 for k in v if v[k][0] is False)
    u = sum(1 for k in v if v[k][0] is None)
    assert s + r + u == len(v), "the tally does not account for every part"
    print()
    print(f"{s} supported, {r} refuted, {u} unevaluable, of {len(v)} parts")
    # D31: a threshold is arithmetic and belongs here; a reading of it is an
    # argument and belongs in the register and the paper, where it can be
    # disagreed with. This script used to print its own interpretation under
    # the verdicts, which is where an extrapolation looks pre-approved. It
    # names the row to read instead.
    print()
    print("Read the H84 row of docs/experiments.md for what these verdicts do and")
    print("do not license. In short: the dependence on capacity is not monotone,")
    print("three sign changes fall across these five points, and capacity is")
    print("confounded here with the slot footprint and the producer run-ahead.")
    return 0


if __name__ == "__main__":
    sys.exit(main())
