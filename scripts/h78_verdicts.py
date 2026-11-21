"""H78 verdicts: does the release store serialise what a plain store pipelines.

Thresholds were fixed and pushed before this data existed. Every ratio here
reports "unevaluable" when its denominator is zero rather than raising, which
is the D19 correction: the first run of this analysis ended on a
ZeroDivisionError after the contended table, because a registered counter read
zero and the script treated that as an error instead of as the finding.

    python3 -I scripts/h78_verdicts.py [--emit]
"""

import math
import sys
from collections import defaultdict
from pathlib import Path

ROOT = Path(__file__).resolve().parent.parent
FILE = ROOT / "results" / "2026-11-21_m4_h78_storeprobe.txt"
ORDERS = ("acqrel", "fencedload", "fenced", "fencedstore", "unordered")
CORRECT = ("acqrel", "fenced", "fencedload", "fencedstore")
FIELDS = ("msg_cycles", "msg_l1d_store_miss", "msg_ldst_wait_cycles", "core_ghz")


def fields(line):
    return {k: v for k, _, v in (t.partition("=") for t in line.split()) if k}


def med(xs):
    xs = sorted(xs)
    h = len(xs) // 2
    return xs[h] if len(xs) % 2 else (xs[h - 1] + xs[h]) / 2.0


def sign_p(k, n):
    """Exact two-sided sign test, symmetric in the direction of the effect.

    The first version summed only the upper tail from k, so a clean sweep in
    the predicted-low direction (0 of 30) came back p=1 and read as no
    evidence, when 0 of 30 is as unlikely as 30 of 30. Taking the larger of
    the two counts restores the symmetry. Caught before any record or
    manuscript number rested on it.
    """
    if n == 0:
        return 1.0
    k = max(k, n - k)
    return min(1.0, 2.0 * sum(math.comb(n, i) for i in range(k, n + 1)) / float(2 ** n))


def ratio(a, b):
    """None rather than an exception when the denominator is zero."""
    return None if b == 0 else a / b


def load():
    per = defaultdict(lambda: defaultdict(list))
    launch = defaultdict(lambda: defaultdict(list))
    ok = defaultdict(lambda: [0, 0])
    for line in FILE.open(encoding="utf-8", errors="replace"):
        if not line.startswith("bd "):
            continue
        f = fields(line)
        if "order" not in f:
            continue
        key = (f["order"], f["cached"])
        for k in FIELDS:
            if k in f:
                per[key][k].append(float(f[k]))
                launch[(key, f.get("launch", "0"))][k].append(float(f[k]))
        ok[f["order"]][0] += int(f.get("sum_ok") == "1")
        ok[f["order"]][1] += 1
    return per, launch, ok


def per_launch_ratio(launch, top, bottom, cached, field):
    """Sign count over launches, skipping any launch with a zero denominator."""
    a, b, wins, n, skipped = {}, {}, 0, 0, 0
    for (key, l), v in launch.items():
        if key == (top, cached) and field in v:
            a[l] = med(v[field])
        if key == (bottom, cached) and field in v:
            b[l] = med(v[field])
    for l in sorted(set(a) & set(b)):
        if b[l] == 0:
            skipped += 1
            continue
        n += 1
        wins += a[l] > b[l]
    return wins, n, skipped, sign_p(wins, n)


def main():
    if not FILE.exists():
        print(f"FAIL: {FILE.name} not found")
        return 1
    per, launch, ok = load()
    print(f"file     : {FILE.name}")
    print(f"launches : {len({l for (_k, l) in launch})}")
    print()

    for cc, lab in (("0", "CONTENDED"), ("1", "RESIDENT ")):
        print(f"  {lab}  {'arm':<13} {'cycles':>9} {'st_miss':>9} {'wait':>10} "
              f"{'not waiting':>12} {'GHz':>6}")
        for o in ORDERS:
            v = per[(o, cc)]
            if not v:
                continue
            c, w = med(v["msg_cycles"]), med(v["msg_ldst_wait_cycles"])
            print(f"  {'':<10} {o:<13} {c:>9.3f} {med(v['msg_l1d_store_miss']):>9.4f} "
                  f"{w:>10.4f} {c - w:>12.3f} {med(v['core_ghz']):>6.3f}")
        print()

    v = {}
    sm_a = med(per[("acqrel", "0")]["msg_l1d_store_miss"])
    sm_f = med(per[("fencedstore", "0")]["msg_l1d_store_miss"])
    r = ratio(sm_f, sm_a)
    if r is None:
        v["a"] = (None,
                  f"UNEVALUABLE. L1D_CACHE_MISS_ST_NONSPEC read {sm_a} for both arms, so "
                  f"the ratio is 0/0. The throttling account is not excluded by this run. "
                  f"See docs/defects.md D19.")
    else:
        w, n, sk, p = per_launch_ratio(launch, "fencedstore", "acqrel", "0",
                                       "msg_l1d_store_miss")
        v["a"] = (r >= 0.95, f"store-miss ratio {r:.4f}, threshold 0.95, {w} of {n}"
                  + (f", {sk} launch(es) skipped for a zero denominator" if sk else ""))

    for cc, key, lo, hi, lab in (("0", "b", None, 0.60, "contended"),
                                 ("1", "c", 0.80, 1.25, "resident")):
        wa = med(per[("acqrel", cc)]["msg_ldst_wait_cycles"])
        wf = med(per[("fencedstore", cc)]["msg_ldst_wait_cycles"])
        r = ratio(wf, wa)
        if r is None:
            v[key] = (None, f"UNEVALUABLE, the acquire-release arm waited 0 cycles {lab}")
            continue
        w, n, sk, p = per_launch_ratio(launch, "fencedstore", "acqrel", cc,
                                       "msg_ldst_wait_cycles")
        inside = (r <= hi) if lo is None else (lo <= r <= hi)
        band = f"at most {hi}" if lo is None else f"between {lo} and {hi}"
        agree = (n - w) if lo is None else w
        word = "waited less in" if lo is None else "exceeded in"
        v[key] = (inside, f"{lab} waiting ratio {r:.4f}, registered {band}; "
                  f"{wf:.4f} against {wa:.4f} cycles per message; the fenced-store arm "
                  f"{word} {agree} of {n} launches, p={p:.3g}")

    worst, detail = 0.0, []
    for cc, lab in (("1", "resident"), ("0", "contended")):
        ga = med(per[("acqrel", cc)]["core_ghz"])
        gf = med(per[("fencedstore", cc)]["core_ghz"])
        s = max(ga, gf) / min(ga, gf)
        worst = max(worst, s)
        detail.append(f"{lab} {s:.4f}")
    v["d"] = (worst <= 1.10, f"frequency between the compared arms, {', '.join(detail)}, "
              f"threshold 1.10")

    bad = [f"{o} {ok[o][0]}/{ok[o][1]}" for o in CORRECT if ok[o][0] != ok[o][1]]
    v["e"] = (not bad and ok["unordered"][0] == 0,
              "; ".join(bad) if bad else
              f"{', '.join(f'{o} {ok[o][0]}/{ok[o][1]}' for o in CORRECT)}, "
              f"unordered {ok['unordered'][0]} of {ok['unordered'][1]}")

    if "--emit" in sys.argv:
        for cc, lab in (("0", "contended"), ("1", "resident")):
            for o in ORDERS:
                p2 = per[(o, cc)]
                if not p2:
                    continue
                c, w = med(p2["msg_cycles"]), med(p2["msg_ldst_wait_cycles"])
                print(f"h78_arm state={lab} order={o} msg_cycles={c:.3f} "
                      f"wait_cycles={w:.4f} not_waiting={c - w:.3f} "
                      f"store_miss={med(p2['msg_l1d_store_miss']):.4f} "
                      f"core_ghz={med(p2['core_ghz']):.3f}")
            wa = med(per[("acqrel", cc)]["msg_ldst_wait_cycles"])
            wf = med(per[("fencedstore", cc)]["msg_ldst_wait_cycles"])
            rr = ratio(wf, wa)
            print(f"h78_wait state={lab} ratio={'unevaluable' if rr is None else f'{rr:.4f}'} "
                  f"acqrel={wa:.4f} fencedstore={wf:.4f}")
        return 0

    print("REGISTERED VERDICTS")
    for k in "abcde":
        okk, why = v[k]
        tag = "UNEVALUABLE" if okk is None else ("SUPPORTED  " if okk else "REFUTED    ")
        print(f"  ({k}) {tag}  {why}")
    print()
    s = sum(1 for k in "abcde" if v[k][0] is True)
    r_ = sum(1 for k in "abcde" if v[k][0] is False)
    u = sum(1 for k in "abcde" if v[k][0] is None)
    print(f"{s} supported, {r_} refuted, {u} unevaluable, of 5 registered parts")
    return 0


if __name__ == "__main__":
    sys.exit(main())
