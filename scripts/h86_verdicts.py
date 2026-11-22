"""H86: does the full barrier win by suppressing the issue rate.

Thresholds were fixed and pushed before this data existed. Every counter was
chosen by reading its description in results/kperf-events.txt rather than from
memory, which is the D25 correction.

    python3 -I scripts/h86_verdicts.py [--emit]
"""

import math
import sys
from collections import defaultdict
from pathlib import Path

ROOT = Path(__file__).resolve().parent.parent
FILE = ROOT / "results" / "2026-11-22_m4_h86_issue.txt"
ORDERS = ("acqrel", "fencedload", "fenced", "fencedstore", "unordered")
CORRECT = ("acqrel", "fenced", "fencedload", "fencedstore")
KEYS = ("msg_cycles", "msg_issue_active", "msg_stall_backend",
        "msg_retired_uops", "ns_per_msg", "core_ghz")


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
    if counters != "issueprobe":
        print(f"FAIL: H86 requires counters=issueprobe, file says {counters}")
        return 1
    print()
    for cc, lab in (("0", "CONTENDED"), ("1", "RESIDENT ")):
        print(f"  {lab} {'arm':<13} {'cycles':>9} {'issue_act':>10} {'issue/cyc':>10} "
              f"{'stall_be':>9} {'uops':>8}")
        for o in ORDERS:
            q = per[(o, cc)]
            if not q:
                continue
            cy, ia = med(q["msg_cycles"]), med(q["msg_issue_active"])
            print(f"  {'':<9} {o:<13} {cy:>9.3f} {ia:>10.4f} {ia / cy:>10.6f} "
                  f"{med(q['msg_stall_backend']):>9.4f} "
                  f"{med(q['msg_retired_uops']):>8.2f}")
        print()

    v = {}
    need = ("msg_issue_active", "msg_stall_backend", "msg_retired_uops")
    vals = {(o, k): (med(per[(o, "0")][k]) if per[(o, "0")].get(k) else 0.0)
            for o in ("acqrel", "fencedstore") for k in need}
    v["a"] = (all(x > 0 for x in vals.values()),
              "contended, each counter above zero for both arms: "
              + ", ".join(f"{o} {k.replace('msg_', '')} {x:.4f}"
                          for (o, k), x in sorted(vals.items())))

    if not v["a"][0]:
        v["b"] = (None, "UNEVALUABLE, gated by (a)")
    else:
        fa = med(per[("acqrel", "0")]["msg_issue_active"]) / med(per[("acqrel", "0")]["msg_cycles"])
        ff = med(per[("fencedstore", "0")]["msg_issue_active"]) / med(per[("fencedstore", "0")]["msg_cycles"])
        w = n = 0
        for L in sorted({l for (_k, l) in byl}):
            qa, qf = byl[((("acqrel"), "0"), L)], byl[((("fencedstore"), "0"), L)]
            if not qa or not qf:
                continue
            n += 1
            ra = med(qa["msg_issue_active"]) / med(qa["msg_cycles"])
            rf = med(qf["msg_issue_active"]) / med(qf["msg_cycles"])
            w += rf < ra
        v["b"] = (ff / fa <= 0.90 and w >= 21,
                  f"issue-active fraction, acqrel {fa:.6f} and fencedstore {ff:.6f}, "
                  f"ratio {ff / fa:.4f} against a threshold of 0.90; fencedstore lower "
                  f"in {w} of {n} launches, p={sign_p(w, n):.3g}")

    ua = med(per[("acqrel", "0")]["msg_retired_uops"]) if per[("acqrel", "0")].get("msg_retired_uops") else 0.0
    uf = med(per[("fencedstore", "0")]["msg_retired_uops"]) if per[("fencedstore", "0")].get("msg_retired_uops") else 0.0
    v["c"] = (ua > 0 and uf > 0 and max(ua, uf) / min(ua, uf) <= 1.25,
              f"retired uops per message, acqrel {ua:.2f} and fencedstore {uf:.2f}, "
              f"ratio {max(ua, uf) / min(ua, uf) if min(ua, uf) else float('inf'):.4f}, "
              f"band 1.25")

    sa = med(per[("acqrel", "0")]["msg_stall_backend"]) if per[("acqrel", "0")].get("msg_stall_backend") else 0.0
    sf = med(per[("fencedstore", "0")]["msg_stall_backend"]) if per[("fencedstore", "0")].get("msg_stall_backend") else 0.0
    v["d"] = (sa > 0 and sf / sa <= 0.50,
              f"backend stall cycles per message, acqrel {sa:.4f} and fencedstore "
              f"{sf:.4f}, ratio {sf / sa if sa else float('inf'):.4f} against 0.50, which "
              f"the H78 waiting ratio of 0.2338 implies")

    worst, det = 0.0, []
    for cc, lab in (("0", "contended"), ("1", "resident")):
        ga, gf = per[("acqrel", cc)].get("core_ghz"), per[("fencedstore", cc)].get("core_ghz")
        if not ga or not gf:
            continue
        s = max(med(ga), med(gf)) / min(med(ga), med(gf))
        worst = max(worst, s)
        det.append(f"{lab} {s:.4f}")
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
                ia = med(q["msg_issue_active"])
                print(f"h86_arm state={lab} order={o} msg_cycles={cy:.3f} "
                      f"issue_active={ia:.4f} issue_fraction={ia / cy:.6f} "
                      f"stall_backend={med(q['msg_stall_backend']):.4f} "
                      f"retired_uops={med(q['msg_retired_uops']):.2f} "
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
    print("Read the H86 row of docs/experiments.md for what (b) licenses either way.")
    return 0


if __name__ == "__main__":
    sys.exit(main())
