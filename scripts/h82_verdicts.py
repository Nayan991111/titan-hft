"""H82: is coherence traffic a cause of the inversion or a consequence of it.

Thresholds were fixed and pushed before this data existed. The two guards are
scoped to exactly the cells the claims they gate use, which is the correction
D25 records and D28 extends: a quantity must not be normalised by the thing
under comparison.

    python3 -I scripts/h82_verdicts.py [--emit]
"""

import sys
from collections import defaultdict
from pathlib import Path

ROOT = Path(__file__).resolve().parent.parent
FILE = ROOT / "results" / "2026-11-22_m4_h82_waitsweep.txt"
ARMS = ("acqrel", "fencedstore")
WAITS = ("spin", "isb", "yield", "wfe")


def fields(line):
    return {k: v for k, _, v in (t.partition("=") for t in line.split()) if k}


def med(xs):
    xs = sorted(xs)
    h = len(xs) // 2
    return xs[h] if len(xs) % 2 else (xs[h - 1] + xs[h]) / 2.0


def pearson(xs, ys):
    n = len(xs)
    if n < 3:
        return None
    mx, my = sum(xs) / n, sum(ys) / n
    sx = sum((x - mx) ** 2 for x in xs) ** 0.5
    sy = sum((y - my) ** 2 for y in ys) ** 0.5
    if sx == 0 or sy == 0:
        return None
    return sum((x - mx) * (y - my) for x, y in zip(xs, ys)) / (sx * sy)


def fit(xs, ys):
    """Least squares slope and intercept, or None if x does not vary."""
    n = len(xs)
    mx, my = sum(xs) / n, sum(ys) / n
    den = sum((x - mx) ** 2 for x in xs)
    if den == 0:
        return None
    m = sum((x - mx) * (y - my) for x, y in zip(xs, ys)) / den
    return m, my - m * mx


def main():
    if not FILE.exists():
        print(f"FAIL: {FILE.name} not found")
        return 1
    per = defaultdict(lambda: defaultdict(list))
    ok = defaultdict(lambda: [0, 0])
    arms_mode, counters, side = None, None, None
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
        key = (f["order"], f.get("wait", "?"))
        for k in ("msg_cycles", "msg_l1d_writeback", "msg_mem_order_viol", "ns_per_msg"):
            if k in f:
                per[key][k].append(float(f[k]))
        ok[key][0] += int(f.get("sum_ok") == "1")
        ok[key][1] += 1

    print(f"file     : {FILE.name}")
    print(f"arms={arms_mode}  counters={counters}  measured_side={side}")
    if arms_mode != "waitsweep":
        print(f"FAIL: H82 requires arms=waitsweep, file says {arms_mode}")
        return 1
    if counters != "violprobe":
        print(f"FAIL: H82 requires counters=violprobe, file says {counters}")
        return 1
    print()
    print(f"  {'arm':<13} {'wait':<7} {'cycles':>9} {'wrbk':>8} {'wrbk/cyc':>9} {'ns/msg':>8} {'ok':>9}")
    cells = []
    for a in ARMS:
        for w in WAITS:
            q = per[(a, w)]
            if not q:
                continue
            cyc, wb = med(q["msg_cycles"]), med(q["msg_l1d_writeback"])
            cells.append((a, w, cyc, wb))
            print(f"  {a:<13} {w:<7} {cyc:>9.3f} {wb:>8.4f} {wb / cyc:>9.6f} "
                  f"{med(q['ns_per_msg']):>8.3f} {ok[(a, w)][0]:>4}/{ok[(a, w)][1]:<4}")
    print()

    v = {}
    v["a"] = (len(cells) == 8 and all(wb > 0 for _, _, _, wb in cells),
              f"{len(cells)} of 8 cells present, "
              f"{sum(1 for _, _, _, wb in cells if wb > 0)} with writebacks above zero")

    spans = []
    for a in ARMS:
        cy = [c for arm, _, c, _ in cells if arm == a]
        if len(cy) < 2:
            continue
        spans.append((a, max(cy) / min(cy)))
    v["b"] = (bool(spans) and len(spans) == 2 and all(s >= 1.25 for _, s in spans),
              "duration axis within each arm, "
              + ", ".join(f"{a} {s:.4f}" for a, s in spans) + ", threshold 1.25")

    gated = v["a"][0] and v["b"][0]
    cyc = [c for _, _, c, _ in cells]
    wb = [w for _, _, _, w in cells]
    if not gated:
        v["c"] = (None, "UNEVALUABLE, gated by (a) and (b)")
        v["d"] = (None, "UNEVALUABLE, gated by (a) and (b)")
    else:
        r = pearson(cyc, wb)
        v["c"] = (r is not None and r >= 0.90,
                  f"correlation between writebacks per message and cycles per message "
                  f"over the eight cells, r={r:.4f}, threshold 0.90"
                  if r is not None else "UNEVALUABLE, a column did not vary")
        ax = [c for a, _, c, _ in cells if a == "acqrel"]
        ay = [w for a, _, _, w in cells if a == "acqrel"]
        f2 = fit(ax, ay)
        if f2 is None:
            v["d"] = (None, "UNEVALUABLE, the acqrel cells show no spread in cycles")
        else:
            m, b0 = f2
            errs = []
            for a, w, c, wbv in cells:
                if a != "fencedstore":
                    continue
                pred = m * c + b0
                errs.append((w, abs(pred - wbv) / wbv if wbv else float("inf"), pred, wbv))
            mare = med([e for _, e, _, _ in errs])
            v["d"] = (mare <= 0.20,
                      f"a line fitted to the four acqrel cells, writebacks = "
                      f"{m:.6f} x cycles + {b0:.4f}, predicts the fencedstore cells with "
                      f"median absolute relative error {mare:.4f}, threshold 0.20; "
                      + ", ".join(f"{w} predicted {pr:.4f} against {ac:.4f}"
                                  for w, _, pr, ac in errs))

    bad = [f"{a} {w} {ok[(a, w)][0]}/{ok[(a, w)][1]}"
           for a in ARMS for w in WAITS
           if per[(a, w)] and ok[(a, w)][0] != ok[(a, w)][1]]
    v["e"] = (not bad, "; ".join(bad) if bad else
              "every record of all eight cells reports a correct sum")

    if "--emit" in sys.argv:
        for a, w, c, wbv in cells:
            print(f"h82_cell arm={a} wait={w} msg_cycles={c:.3f} writeback={wbv:.4f} "
                  f"writeback_per_cycle={wbv / c:.6f}")
        if gated and pearson(cyc, wb) is not None:
            print(f"h82_fit pearson={pearson(cyc, wb):.4f}")
        return 0

    print("REGISTERED VERDICTS, guards before the claims they gate")
    for k in "abcde":
        o, why = v[k]
        tag = "UNEVALUABLE" if o is None else ("SUPPORTED  " if o else "REFUTED    ")
        print(f"  ({k}) {tag}  {why}")
    s = sum(1 for k in v if v[k][0] is True)
    r2 = sum(1 for k in v if v[k][0] is False)
    u = sum(1 for k in v if v[k][0] is None)
    assert s + r2 + u == len(v), "the tally does not account for every part"
    print()
    print(f"{s} supported, {r2} refuted, {u} unevaluable, of {len(v)} parts")
    if v["d"][0] is True:
        print()
        print("REGISTERED READING: both arms lie on one duration traffic line, so the")
        print("ordering instruction carries no coherence traffic beyond the time it")
        print("costs, and the throttling account is dead as an explanation.")
    elif v["d"][0] is False:
        print()
        print("REGISTERED READING: the arms sit on separate lines, so arm specific")
        print("coherence traffic exists beyond duration and the throttling account has")
        print("something real to stand on. The residual above is the size of it.")
    return 0


if __name__ == "__main__":
    sys.exit(main())
