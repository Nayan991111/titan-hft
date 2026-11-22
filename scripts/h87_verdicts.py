"""H87: where the unaccounted cycles go.

Thresholds were fixed and pushed before this data existed. Every counter was
chosen by reading its description in results/kperf-events.txt rather than from
memory, which is the D25 correction.

    python3 -I scripts/h87_verdicts.py [--emit]
"""

import math
import sys
from collections import defaultdict
from pathlib import Path

ROOT = Path(__file__).resolve().parent.parent
FILE = ROOT / "results" / "2026-11-22_m4_h87_stalls.txt"
ORDERS = ("acqrel", "fencedload", "fenced", "fencedstore", "unordered")
CORRECT = ("acqrel", "fenced", "fencedload", "fencedstore")
KEYS = ("msg_cycles", "msg_stall_any", "msg_stall_frontend",
        "msg_stall_backend", "ns_per_msg", "core_ghz")


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


def main():
    if not FILE.exists():
        print(f"FAIL: {FILE.name} not found")
        return 1
    per = defaultdict(lambda: defaultdict(list))
    byl = defaultdict(lambda: defaultdict(list))
    ok = defaultdict(lambda: [0, 0])
    counters = side = None
    for line in FILE.open(encoding="utf-8", errors="replace"):
        if line.startswith("bd_config"):
            f = fields(line)
            counters = counters or f.get("counters")
            side = side or f.get("measured_side")
        if not line.startswith("bd ") or "order=" not in line:
            continue
        f = fields(line)
        key = (f["order"], f["cached"])
        for k in KEYS:
            if k in f:
                per[key][k].append(float(f[k]))
                byl[(key, f.get("launch", "0"))][k].append(float(f[k]))
        ok[f["order"]][0] += int(f.get("sum_ok") == "1")
        ok[f["order"]][1] += 1

    print(f"file     : {FILE.name}")
    print(f"counters={counters}  measured_side={side}  "
          f"launches={len({l for (_k, l) in byl})}")
    if counters != "stallprobe":
        print(f"FAIL: H87 requires counters=stallprobe, file says {counters}")
        return 1
    print()
    for cc, lab in (("0", "CONTENDED"), ("1", "RESIDENT ")):
        print(f"  {lab} {'arm':<13} {'cycles':>9} {'stall_any':>10} {'stall/cyc':>9} "
              f"{'frontend':>10} {'backend':>10} {'fe+be-any':>9}")
        for o in ORDERS:
            q = per[(o, cc)]
            if not q:
                continue
            cy = med(q["msg_cycles"])
            sa, sf2, sb = (med(q["msg_stall_any"]), med(q["msg_stall_frontend"]),
                           med(q["msg_stall_backend"]))
            print(f"  {'':<9} {o:<13} {cy:>9.3f} {sa:>10.4f} {sa / cy:>9.4f} "
                  f"{sf2:>10.4f} {sb:>10.4f} {sf2 + sb - sa:>9.4f}")
        print()

    v = {}
    need = ("msg_stall_any", "msg_stall_frontend", "msg_stall_backend")
    vals = {(o, k): (med(per[(o, "0")][k]) if per[(o, "0")].get(k) else 0.0)
            for o in ("acqrel", "fencedstore") for k in need}
    v["a"] = (all(x > 0 for x in vals.values()),
              "contended, each stall counter above zero for both arms: "
              + ", ".join(f"{o} {k.replace('msg_stall_', '')} {x:.4f}"
                          for (o, k), x in sorted(vals.items())))

    if not v["a"][0]:
        v["b"] = (None, "UNEVALUABLE, gated by (a)")
    else:
        det, worst = [], 0.0
        for o in ("acqrel", "fencedstore"):
            q = per[(o, "0")]
            tot = med(q["msg_stall_any"])
            parts = med(q["msg_stall_frontend"]) + med(q["msg_stall_backend"])
            r = max(tot, parts) / min(tot, parts) if min(tot, parts) else float("inf")
            worst = max(worst, r)
            det.append(f"{o} frontend+backend {parts:.4f} against any {tot:.4f}, "
                       f"ratio {r:.4f}")
        v["b"] = (worst <= 1.10,
                  "the three counters partition the stalled cycles: "
                  + "; ".join(det) + ", band 1.10")

    gated = v["a"][0] and v["b"][0]
    if not gated:
        v["c"] = (None, "UNEVALUABLE, gated by (a) and (b)")
        v["d"] = (None, "UNEVALUABLE, gated by (a) and (b)")
    else:
        q = per[("acqrel", "0")]
        frac = med(q["msg_stall_any"]) / med(q["msg_cycles"])
        v["c"] = (frac >= 0.80,
                  f"acqrel contended, stalled cycles {med(q['msg_stall_any']):.4f} of "
                  f"{med(q['msg_cycles']):.3f} total, fraction {frac:.4f}, threshold 0.80")
        if not v["c"][0]:
            v["d"] = (None, "UNEVALUABLE, gated by (c)")
        else:
            fe = med(q["msg_stall_frontend"]) / med(q["msg_stall_any"])
            v["d"] = (fe >= 0.50,
                      f"acqrel contended, frontend share of stalled cycles "
                      f"{med(q['msg_stall_frontend']):.4f} of "
                      f"{med(q['msg_stall_any']):.4f}, fraction {fe:.4f}, threshold 0.50; "
                      f"at or above means frontend bound, below means backend bound and "
                      f"contradicts the 0.3507 backend stall cycles H86 measured on this "
                      f"arm, in which case the two runs disagree and neither is used")

    worst, det = 0.0, []
    for cc, lab in (("0", "contended"), ("1", "resident")):
        ga, gf = per[("acqrel", cc)].get("core_ghz"), per[("fencedstore", cc)].get("core_ghz")
        if not ga or not gf:
            continue
        s2 = max(med(ga), med(gf)) / min(med(ga), med(gf))
        worst = max(worst, s2)
        det.append(f"{lab} {s2:.4f}")
    v["e"] = (bool(det) and worst <= 1.10,
              f"frequency between the compared pair, {', '.join(det)}, threshold 1.10")

    bad = [f"{o} {ok[o][0]}/{ok[o][1]}" for o in CORRECT if ok[o][0] != ok[o][1]]
    v["f"] = (not bad,
              ("; ".join(bad) if bad else
               ", ".join(f"{o} {ok[o][0]}/{ok[o][1]}" for o in CORRECT))
              + f"; unordered {ok['unordered'][0]} of {ok['unordered'][1]}, recorded "
                f"and not thresholded per D24")

    if "--emit" in sys.argv:
        for cc, lab in (("0", "contended"), ("1", "resident")):
            for o in ORDERS:
                q = per[(o, cc)]
                if not q:
                    continue
                cy = med(q["msg_cycles"])
                print(f"h87_arm state={lab} order={o} msg_cycles={cy:.3f} "
                      f"stall_any={med(q['msg_stall_any']):.4f} "
                      f"stall_fraction={med(q['msg_stall_any']) / cy:.6f} "
                      f"stall_frontend={med(q['msg_stall_frontend']):.4f} "
                      f"stall_backend={med(q['msg_stall_backend']):.4f} "
                      f"ns_per_msg={med(q['ns_per_msg']):.3f}")
        return 0

    print("REGISTERED VERDICTS, guard before the claim it gates")
    for k in "abcdef":
        o, why = v[k]
        tag = "UNEVALUABLE" if o is None else ("SUPPORTED  " if o else "REFUTED    ")
        print(f"  ({k}) {tag}  {why}")
    s = sum(1 for k in v if v[k][0] is True)
    r = sum(1 for k in v if v[k][0] is False)
    u = sum(1 for k in v if v[k][0] is None)
    assert s + r + u == len(v), "the tally does not account for every part"
    print()
    print(f"{s} supported, {r} refuted, {u} unevaluable, of {len(v)} parts")
    print()
    print("Read the H87 row of docs/experiments.md for what (b) licenses either way.")
    return 0


if __name__ == "__main__":
    sys.exit(main())
