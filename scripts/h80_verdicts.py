"""H80 and H81: coherence work and memory order violations, both threads.

Thresholds were fixed and pushed before either run existed. Each measurability
guard is evaluated on exactly the cells the claim it guards uses, which is the
correction D25 records after four thresholds were scoped to the wrong thing: a
pair read as five arms, a composition read as an absolute, the wrong thread,
and a guard wider than the comparison it guarded.

    python3 -I scripts/h80_verdicts.py [--emit]
"""

import math
import sys
from collections import defaultdict
from pathlib import Path

ROOT = Path(__file__).resolve().parent.parent
RUNS = (("H80", "producer", ROOT / "results" / "2026-11-22_m4_h80_viol_prodside.txt"),
        ("H81", "consumer", ROOT / "results" / "2026-11-22_m4_h81_viol_consumer.txt"))
ORDERS = ("acqrel", "fencedload", "fenced", "fencedstore", "unordered")
CORRECT = ("acqrel", "fenced", "fencedload", "fencedstore")
KEYS = ("msg_cycles", "msg_l1d_writeback", "msg_mem_order_viol",
        "msg_ldst_wait_cycles", "ns_per_msg", "core_ghz")


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


def load(path):
    per = defaultdict(lambda: defaultdict(list))
    launch = defaultdict(lambda: defaultdict(list))
    ok = defaultdict(lambda: [0, 0])
    side, counters = None, None
    for line in path.open(encoding="utf-8", errors="replace"):
        if line.startswith("bd_config"):
            f = fields(line)
            side = side or f.get("measured_side")
            counters = counters or f.get("counters")
        if not line.startswith("bd ") or "order=" not in line:
            continue
        f = fields(line)
        key = (f["order"], f["cached"])
        for k in KEYS:
            if k in f:
                per[key][k].append(float(f[k]))
                launch[(key, f.get("launch", "0"))][k].append(float(f[k]))
        ok[f["order"]][0] += int(f.get("sum_ok") == "1")
        ok[f["order"]][1] += 1
    return per, launch, ok, side, counters


def launch_wins(launch, top, bottom, cc, field, floor):
    """Launches where top/bottom is at or above `floor`, skipping zero denominators."""
    a, b, w, n, skip = {}, {}, 0, 0, 0
    for (key, l), v in launch.items():
        if key == (top, cc) and field in v:
            a[l] = med(v[field])
        if key == (bottom, cc) and field in v:
            b[l] = med(v[field])
    for l in sorted(set(a) & set(b)):
        if b[l] == 0:
            skip += 1
            continue
        n += 1
        w += (a[l] / b[l]) >= floor
    return w, n, skip


def verdicts(per, launch, ok):
    v = {}
    wb_a = med(per[("acqrel", "0")]["msg_l1d_writeback"]) if per[("acqrel", "0")].get("msg_l1d_writeback") else 0.0
    wb_f = med(per[("fencedstore", "0")]["msg_l1d_writeback"]) if per[("fencedstore", "0")].get("msg_l1d_writeback") else 0.0
    v["a"] = (wb_a > 0 and wb_f > 0,
              f"contended writebacks per message, acqrel {wb_a:.4f} and fencedstore "
              f"{wb_f:.4f}; both must exceed zero")

    if not v["a"][0]:
        v["b"] = (None, "UNEVALUABLE, gated by (a): a writeback counter reading zero "
                  "in a cell this comparison uses is a failed measurement of it")
    else:
        w, n, sk = launch_wins(launch, "fencedstore", "acqrel", "0",
                               "msg_l1d_writeback", 1.0)
        r = wb_f / wb_a
        v["b"] = (r >= 0.95,
                  f"contended writeback ratio {r:.4f}, threshold 0.95, per-launch at or "
                  f"above 1.0 in {w} of {n}, p={sign_p(w, n):.3g}"
                  + (f", {sk} skipped for a zero denominator" if sk else "")
                  + f". {'Supported, so the throttling account is refuted' if r >= 0.95 else 'Refuted, so the faster arm does do less coherence work and the throttling account survives'}")

    mv_a = med(per[("acqrel", "0")]["msg_mem_order_viol"]) if per[("acqrel", "0")].get("msg_mem_order_viol") else 0.0
    mv_f = med(per[("fencedstore", "0")]["msg_mem_order_viol"]) if per[("fencedstore", "0")].get("msg_mem_order_viol") else 0.0
    v["c"] = (max(mv_a, mv_f) > 0,
              f"contended memory order violations per message, acqrel {mv_a:.4f} and "
              f"fencedstore {mv_f:.4f}; at least one must exceed zero")

    if not v["c"][0]:
        v["d"] = (None, "UNEVALUABLE, gated by (c): neither compared arm triggered a "
                  "measurable violation, so no ratio exists")
    elif mv_f == 0:
        v["d"] = (None, f"UNEVALUABLE, fencedstore read 0.0000 while acqrel read "
                  f"{mv_a:.4f}, so the ratio is unbounded rather than at or above 2.0. "
                  f"The direction is recorded and the threshold is not met as written")
    else:
        w, n, sk = launch_wins(launch, "acqrel", "fencedstore", "0",
                               "msg_mem_order_viol", 2.0)
        r = mv_a / mv_f
        v["d"] = (r >= 2.0 and w >= 21,
                  f"contended violation ratio {r:.4f}, threshold 2.0, at or above 2.0 in "
                  f"{w} of {n} launches, p={sign_p(w, n):.3g}"
                  + (f", {sk} skipped for a zero denominator" if sk else ""))

    worst, det = 0.0, []
    for cc, lab in (("0", "contended"), ("1", "resident")):
        ga = per[("acqrel", cc)].get("core_ghz")
        gf = per[("fencedstore", cc)].get("core_ghz")
        if not ga or not gf:
            continue
        s = max(med(ga), med(gf)) / min(med(ga), med(gf))
        worst = max(worst, s)
        det.append(f"{lab} {s:.4f}")
    # `worst <= 1.10 and det` returns the list, not a bool, so the verdict
    # printed as SUPPORTED while the tally, which tests `is True`, skipped it:
    # six parts were listed and five counted. A summary that disagrees with the
    # lines above it is worse than no summary.
    v["e"] = (bool(det) and worst <= 1.10,
              f"frequency between the compared pair, {', '.join(det)}, threshold 1.10")

    bad = [f"{o} {ok[o][0]}/{ok[o][1]}" for o in CORRECT if ok[o][0] != ok[o][1]]
    v["f"] = (not bad,
              ("; ".join(bad) if bad else
               ", ".join(f"{o} {ok[o][0]}/{ok[o][1]}" for o in CORRECT))
              + f"; unordered {ok['unordered'][0]} of {ok['unordered'][1]}, recorded "
                f"and not thresholded per D24")
    return v


def main():
    tag = lambda o: "UNEVALUABLE" if o is None else ("SUPPORTED  " if o else "REFUTED    ")
    results, missing = {}, []
    for name, side, path in RUNS:
        if not path.exists():
            missing.append(f"{name}: {path.name} not found")
            continue
        per, launch, ok, got_side, counters = load(path)
        print(f"{name}  file {path.name}")
        print(f"      measured_side={got_side}  counters={counters}  "
              f"launches={len({l for (_k, l) in launch})}")
        if got_side != side:
            print(f"FAIL: {name} requires measured_side={side}, file says {got_side}")
            return 1
        if counters != "violprobe":
            print(f"FAIL: {name} requires counters=violprobe, file says {counters}")
            return 1
        print()
        for cc, lab in (("0", "CONTENDED"), ("1", "RESIDENT ")):
            print(f"  {lab} {'arm':<13} {'cycles':>9} {'wrbk':>8} {'memviol':>9} "
                  f"{'wait':>9} {'ns/msg':>8}")
            for o in ORDERS:
                q = per[(o, cc)]
                if not q:
                    continue
                print(f"  {'':<9} {o:<13} {med(q['msg_cycles']):>9.3f} "
                      f"{med(q['msg_l1d_writeback']):>8.4f} "
                      f"{med(q['msg_mem_order_viol']):>9.4f} "
                      f"{med(q['msg_ldst_wait_cycles']):>9.4f} "
                      f"{med(q['ns_per_msg']):>8.3f}")
            print()
        v = verdicts(per, launch, ok)
        results[name] = v
        if "--emit" not in sys.argv:
            print(f"  {name} REGISTERED VERDICTS, guards before the claims they gate")
            for k in "abcdef":
                print(f"    ({k}) {tag(v[k][0])}  {v[k][1]}")
            s = sum(1 for k in v if v[k][0] is True)
            r = sum(1 for k in v if v[k][0] is False)
            u = sum(1 for k in v if v[k][0] is None)
            assert s + r + u == len(v), (
                f"the tally counts {s + r + u} of {len(v)} parts, so a verdict is "
                f"neither True, False nor None: {[k for k in v if v[k][0] not in (True, False, None)]}")
            print(f"    {s} supported, {r} refuted, {u} unevaluable, of {len(v)} parts")
            print()

    if "--emit" in sys.argv:
        for name, side, path in RUNS:
            if name not in results:
                continue
            per, _, _, _, _ = load(path)
            for cc, lab in (("0", "contended"), ("1", "resident")):
                for o in ORDERS:
                    q = per[(o, cc)]
                    if not q:
                        continue
                    print(f"{name.lower()}_arm side={side} state={lab} order={o} "
                          f"msg_cycles={med(q['msg_cycles']):.3f} "
                          f"writeback={med(q['msg_l1d_writeback']):.4f} "
                          f"mem_order_viol={med(q['msg_mem_order_viol']):.4f} "
                          f"wait_cycles={med(q['msg_ldst_wait_cycles']):.4f} "
                          f"ns_per_msg={med(q['ns_per_msg']):.3f}")
        return 0

    if len(results) == 2:
        agree = []
        for k in ("b", "d"):
            x, y = results["H80"][k][0], results["H81"][k][0]
            agree.append(f"({k}) {'agree' if x == y else 'DISAGREE'}, "
                         f"producer {tag(x).strip()} and consumer {tag(y).strip()}")
        same = all("DISAGREE" not in a for a in agree)
        print(f"  H81 (g) {'SUPPORTED  ' if same else 'REFUTED    '}  "
              f"direction across threads: " + "; ".join(agree))
        if not same:
            print("  Reported as a thread dependent effect, not as a failure of "
                  "either run, which is what the registration required.")
    for m in missing:
        print(f"FAIL: {m}")
    return 1 if missing else 0


if __name__ == "__main__":
    sys.exit(main())
