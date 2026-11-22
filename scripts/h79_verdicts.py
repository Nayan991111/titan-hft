"""H79 verdicts: H78 part (a) on the thread that executes the release store.

Thresholds were fixed and pushed before this data existed. Measurability is
checked first and gates part (a), because D19 showed that a zero counter
reported as a result is worse than a missing result.

    python3 -I scripts/h79_verdicts.py [--emit]
"""

import math
import sys
from collections import defaultdict
from pathlib import Path

ROOT = Path(__file__).resolve().parent.parent
FILE = ROOT / "results" / "2026-11-21_m4_h79_prodside.txt"
REF = ROOT / "results" / "2026-11-21_m4_h77_cycles.txt"
ORDERS = ("acqrel", "fencedload", "fenced", "fencedstore", "unordered")
CORRECT = ("acqrel", "fenced", "fencedload", "fencedstore")


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


def load(path, keys):
    per = defaultdict(lambda: defaultdict(list))
    launch = defaultdict(lambda: defaultdict(list))
    ok = defaultdict(lambda: [0, 0])
    side = None
    for line in path.open(encoding="utf-8", errors="replace"):
        if line.startswith("bd_config") and side is None:
            side = fields(line).get("measured_side")
        if not line.startswith("bd "):
            continue
        f = fields(line)
        if "order" not in f:
            continue
        key = (f["order"], f["cached"])
        for k in keys:
            if k in f:
                per[key][k].append(float(f[k]))
                launch[(key, f.get("launch", "0"))][k].append(float(f[k]))
        ok[f["order"]][0] += int(f.get("sum_ok") == "1")
        ok[f["order"]][1] += 1
    return per, launch, ok, side


def ranking(per, cc, field):
    have = [(o, med(per[(o, cc)][field])) for o in ORDERS if per[(o, cc)].get(field)]
    return [o for o, _ in sorted(have, key=lambda t: t[1])]


def wins(launch, top, bottom, cc, field):
    a, b, w, n = {}, {}, 0, 0
    for (key, l), v in launch.items():
        if key == (top, cc) and field in v:
            a[l] = med(v[field])
        if key == (bottom, cc) and field in v:
            b[l] = med(v[field])
    for l in sorted(set(a) & set(b)):
        n += 1
        w += a[l] > b[l]
    return w, n


def main():
    if not FILE.exists():
        print(f"FAIL: {FILE.name} not found")
        return 1
    keys = ("msg_cycles", "msg_l1d_store_miss", "msg_ldst_wait_cycles",
            "ns_per_msg", "core_ghz")
    per, launch, ok, side = load(FILE, keys)
    print(f"file         : {FILE.name}")
    print(f"measured_side: {side}")
    print(f"launches     : {len({l for (_k, l) in launch})}")
    if side != "producer":
        print(f"FAIL: this file reports measured_side={side}; H79 requires producer")
        return 1
    print()
    for cc, lab in (("0", "CONTENDED"), ("1", "RESIDENT ")):
        print(f"  {lab}  {'arm':<13} {'cycles':>9} {'st_miss':>9} {'wait':>10} {'ns/msg':>9}")
        for o in ORDERS:
            v = per[(o, cc)]
            if not v:
                continue
            print(f"  {'':<10} {o:<13} {med(v['msg_cycles']):>9.3f} "
                  f"{med(v['msg_l1d_store_miss']):>9.4f} "
                  f"{med(v['msg_ldst_wait_cycles']):>10.4f} "
                  f"{med(v['ns_per_msg']):>9.3f}")
        print()

    v = {}
    sm = [med(per[(o, cc)]["msg_l1d_store_miss"])
          for cc in "01" for o in ORDERS if per[(o, cc)].get("msg_l1d_store_miss")]
    hi = max(sm) if sm else 0.0
    v["b"] = (hi > 0.0, f"largest median store miss across all arms and states is "
              f"{hi:.4f}; the threshold is strictly above zero")

    if not v["b"][0]:
        v["a"] = (None, "UNEVALUABLE, gated by (b): the counter read zero on the "
                  "producer thread as well, so the quantity is unmeasured again "
                  "and no ratio is reported")
    else:
        a = med(per[("acqrel", "0")]["msg_l1d_store_miss"])
        f2 = med(per[("fencedstore", "0")]["msg_l1d_store_miss"])
        if a == 0:
            v["a"] = (None, f"UNEVALUABLE, both compared arms read 0.0000 "
                      f"contended, so the ratio is 0/0 again. (b) passed on "
                      f"{hi:.4f} from a cell this comparison does not use. "
                      f"See docs/defects.md D25")
        else:
            w, n = wins(launch, "fencedstore", "acqrel", "0", "msg_l1d_store_miss")
            v["a"] = (f2 / a >= 0.95, f"contended store-miss ratio {f2 / a:.4f}, "
                      f"threshold 0.95; {f2:.4f} against {a:.4f} per message, "
                      f"fencedstore higher in {w} of {n} launches, p={sign_p(w, n):.3g}")

    if REF.exists():
        rper, _, _, rside = load(REF, ("ns_per_msg",))
        detail, same = [], True
        for cc, lab in (("0", "contended"), ("1", "resident")):
            mine, theirs = ranking(per, cc, "ns_per_msg"), ranking(rper, cc, "ns_per_msg")
            same &= mine == theirs
            detail.append(f"{lab} {'same' if mine == theirs else f'{mine} vs {theirs}'}")
        side_txt = rside if rside else "consumer, recorded before the field existed"
        v["c"] = (same, f"ranking by median ns_per_msg against {REF.name} "
                  f"(measured_side={side_txt}): " + "; ".join(detail))
    else:
        v["c"] = (None, f"UNEVALUABLE, {REF.name} not found")

    det, allok = [], True
    for cc, lab, want_faster in (("0", "contended", True), ("1", "resident", False)):
        w, n = wins(launch, "fencedstore", "acqrel", cc, "ns_per_msg")
        agree = (n - w) if want_faster else w
        allok &= agree >= 21
        det.append(f"{lab} {'faster' if want_faster else 'slower'} in {agree} of {n}")
    v["d"] = (allok, "sign of the pair preserved, threshold 21 of 30: " + "; ".join(det))

    bad = [f"{o} {ok[o][0]}/{ok[o][1]}" for o in CORRECT if ok[o][0] != ok[o][1]]
    v["e"] = (not bad and ok["unordered"][0] == 0,
              "; ".join(bad) if bad else
              f"{', '.join(f'{o} {ok[o][0]}/{ok[o][1]}' for o in CORRECT)}, "
              f"unordered {ok['unordered'][0]} of {ok['unordered'][1]}")

    if "--emit" in sys.argv:
        for cc, lab in (("0", "contended"), ("1", "resident")):
            for o in ORDERS:
                q = per[(o, cc)]
                if not q:
                    continue
                print(f"h79_arm side=producer state={lab} order={o} "
                      f"msg_cycles={med(q['msg_cycles']):.3f} "
                      f"store_miss={med(q['msg_l1d_store_miss']):.4f} "
                      f"wait_cycles={med(q['msg_ldst_wait_cycles']):.4f} "
                      f"ns_per_msg={med(q['ns_per_msg']):.3f}")
        return 0

    print("REGISTERED VERDICTS, measurability first")
    for k in ("b", "a", "c", "d", "e"):
        okk, why = v[k]
        tag = "UNEVALUABLE" if okk is None else ("SUPPORTED  " if okk else "REFUTED    ")
        print(f"  ({k}) {tag}  {why}")
    s = sum(1 for k in v if v[k][0] is True)
    r = sum(1 for k in v if v[k][0] is False)
    u = sum(1 for k in v if v[k][0] is None)
    print()
    print(f"{s} supported, {r} refuted, {u} unevaluable, of 5 registered parts")
    if not v["c"][0] or not v["d"][0]:
        print()
        print("REGISTERED CONSEQUENCE FIRES: the comparability check did not hold,")
        print("so this run stands on its own. The paper reports consumer side")
        print("numbers for every timing claim and producer side counters only for")
        print("the store miss question, with no cross mode comparison of magnitudes.")
    return 0


if __name__ == "__main__":
    sys.exit(main())
