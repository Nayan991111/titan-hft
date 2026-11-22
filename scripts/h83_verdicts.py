"""H83: arm specific coherence traffic, with a lever that moves both arms.

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
FILE = ROOT / "results" / "2026-11-22_m4_h83_capsweep.txt"
ARMS = ("acqrel", "fencedstore")
CAPS = ("2", "16", "128", "1024")


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
    if arms_mode != "capsweep":
        print(f"FAIL: H83 requires arms=capsweep, file says {arms_mode}")
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
    v["a"] = (len(cells) == 8 and all(wb > 0 and rf > 0 for _, _, _, wb, rf in cells),
              f"{len(cells)} of 8 cells, "
              f"{sum(1 for _, _, _, wb, rf in cells if wb > 0 and rf > 0)} with both "
              f"writebacks and refills above zero")

    spans = []
    for a in ARMS:
        cy = [c for arm, _, c, _, _ in cells if arm == a]
        if len(cy) >= 2:
            spans.append((a, max(cy) / min(cy)))
    v["b"] = (len(spans) == 2 and all(s >= 1.50 for _, s in spans),
              "the lever acts on both arms, "
              + ", ".join(f"{a} {s:.4f}" for a, s in spans) + ", threshold 1.50")

    gated = v["a"][0] and v["b"][0]
    if not gated:
        v["c"] = (None, "UNEVALUABLE, gated by (a) and (b)")
        v["d"] = (None, "UNEVALUABLE, gated by (a) and (b)")
    else:
        arate = [wb / c for a, _, c, wb, _ in cells if a == "acqrel"]
        frate = [wb / c for a, _, c, wb, _ in cells if a == "fencedstore"]
        nonoverlap = min(frate) > max(arate)
        per_cap = []
        allpass = True
        for cp in CAPS:
            w = n = 0
            for L in sorted({l for (_k, l) in byl}):
                qa = byl[(("acqrel", cp), L)]
                qf = byl[(("fencedstore", cp), L)]
                if not qa or not qf:
                    continue
                ra = med(qa["msg_l1d_writeback"]) / med(qa["msg_cycles"])
                rf2 = med(qf["msg_l1d_writeback"]) / med(qf["msg_cycles"])
                n += 1
                w += rf2 > ra
            per_cap.append(f"cap {cp}: {w} of {n}")
            allpass &= n > 0 and w >= 17
        v["c"] = (nonoverlap and allpass,
                  f"non overlap, acqrel rate at most {max(arate):.6f} and fencedstore "
                  f"at least {min(frate):.6f}, separated: {nonoverlap}; per launch at "
                  f"matched capacity, threshold 17 of 20, " + "; ".join(per_cap))

        ax = [c for a, _, c, _, _ in cells if a == "acqrel"]
        ay = [wb for a, _, _, wb, _ in cells if a == "acqrel"]
        f2 = fit(ax, ay)
        if f2 is None:
            v["d"] = (None, "UNEVALUABLE, the acqrel cells show no spread in cycles")
        else:
            m, b0 = f2
            errs = [(cp, abs((m * c + b0) - wb) / wb, m * c + b0, wb)
                    for a, cp, c, wb, _ in cells if a == "fencedstore" and wb]
            mare = med([e for _, e, _, _ in errs])
            v["d"] = (mare > 0.20,
                      f"a line fitted to the four acqrel cells, writebacks = "
                      f"{m:.6f} x cycles + {b0:.4f}, misses the fencedstore cells by a "
                      f"median absolute relative error of {mare:.4f}, threshold above "
                      f"0.20; " + ", ".join(f"cap {cp} predicted {pr:.4f} against "
                                            f"{ac:.4f}" for cp, _, pr, ac in errs))

    bad = [f"{a} cap {cp} {ok[(a, cp)][0]}/{ok[(a, cp)][1]}"
           for a in ARMS for cp in CAPS
           if per[(a, cp)] and ok[(a, cp)][0] != ok[(a, cp)][1]]
    v["e"] = (not bad, "; ".join(bad) if bad else
              "every record of all eight cells reports a correct sum")

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
    if v["c"][0] and v["d"][0]:
        print()
        print("REGISTERED READING: there is arm specific coherence traffic that")
        print("duration does not explain, and the residual above is its size. This is")
        print("the first positive mechanism this project has for the inversion.")
    elif v["c"][0] is False or v["d"][0] is False:
        print()
        print("REGISTERED READING: the traffic difference is duration, and the paper")
        print("says the mechanism remains open.")
    return 0


if __name__ == "__main__":
    sys.exit(main())
