"""H88: is the frontend stall the ordering construct or the isb in the wait loop.

Thresholds were fixed and pushed before this data existed. The question exists
because every run in this project carrying stall or waiting counters also
carries wait=isb, and isb is context synchronizing and forces an instruction
refetch, so a spin loop built on it generates frontend stalls by construction.

    python3 -I scripts/h88_verdicts.py [--emit]
"""

import sys
from collections import defaultdict
from pathlib import Path

ROOT = Path(__file__).resolve().parent.parent
FILE = ROOT / "results" / "2026-11-23_m4_h88_waitstalls.txt"
ARMS = ("acqrel", "fencedstore")
WAITS = ("spin", "isb", "yield", "wfe")
KEYS = ("msg_cycles", "msg_stall_any", "msg_stall_frontend", "msg_stall_backend",
        "ns_per_msg", "core_ghz")


def fields(line):
    return {k: v for k, _, v in (t.partition("=") for t in line.split()) if k}


def med(xs):
    xs = sorted(xs)
    h = len(xs) // 2
    return xs[h] if len(xs) % 2 else (xs[h - 1] + xs[h]) / 2.0


def main():
    if not FILE.exists():
        print(f"FAIL: {FILE.name} not found")
        return 1
    per = defaultdict(lambda: defaultdict(list))
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
        key = (f["order"], f.get("wait", "?"))
        for k in KEYS:
            if k in f:
                per[key][k].append(float(f[k]))
        ok[key][0] += int(f.get("sum_ok") == "1")
        ok[key][1] += 1

    print(f"file     : {FILE.name}")
    print(f"arms={arms_mode}  counters={counters}  measured_side={side}")
    if arms_mode != "waitsweep":
        print(f"FAIL: H88 requires arms=waitsweep, file says {arms_mode}")
        return 1
    if counters != "stallprobe":
        print(f"FAIL: H88 requires counters=stallprobe, file says {counters}")
        return 1
    print()
    print(f"  {'arm':<13} {'wait':<6} {'cycles':>9} {'frontend':>9} {'fe share':>9} "
          f"{'backend':>9} {'be share':>9} {'fe+be-any':>10} {'ok':>9}")
    cells = {}
    for a in ARMS:
        for w in WAITS:
            q = per[(a, w)]
            if not q:
                continue
            cy = med(q["msg_cycles"])
            fe, be = med(q["msg_stall_frontend"]), med(q["msg_stall_backend"])
            an = med(q["msg_stall_any"])
            cells[(a, w)] = (cy, an, fe, be)
            print(f"  {a:<13} {w:<6} {cy:>9.3f} {fe:>9.4f} {fe / cy:>8.2%} "
                  f"{be:>9.4f} {be / cy:>8.2%} {fe + be - an:>10.4f} "
                  f"{ok[(a, w)][0]:>4}/{ok[(a, w)][1]:<4}")
    print()

    v = {}
    v["a"] = (len(cells) == 8 and all(an > 0 and fe > 0 and be > 0
                                      for _, an, fe, be in cells.values()),
              f"{len(cells)} of 8 cells, "
              f"{sum(1 for _, an, fe, be in cells.values() if an > 0 and fe > 0 and be > 0)}"
              f" with all three stall counters above zero")

    if not v["a"][0]:
        v["b"] = (None, "UNEVALUABLE, gated by (a)")
        v["c"] = (None, "UNEVALUABLE, gated by (a)")
    else:
        hi = cells[("acqrel", "isb")]
        lo = cells[("acqrel", "spin")]
        shi, slo = hi[2] / hi[0], lo[2] / lo[0]
        v["b"] = (shi >= 0.80 and slo <= 0.50,
                  f"acqrel frontend share of cycles, {shi:.4f} under isb against a "
                  f"threshold of 0.80, and {slo:.4f} under spin against a threshold of "
                  f"0.50. Both holding means the frontend stall is the wait loop and not "
                  f"the ordering construct, and the stall profile of every earlier run "
                  f"carrying wait=isb is confounded the same way")
        r_spin = cells[("fencedstore", "spin")][0] / cells[("acqrel", "spin")][0]
        r_isb = cells[("fencedstore", "isb")][0] / cells[("acqrel", "isb")][0]
        every = ", ".join(f"{w} {cells[('fencedstore', w)][0] / cells[('acqrel', w)][0]:.4f}"
                          for w in WAITS if ("fencedstore", w) in cells)
        v["c"] = (max(r_spin, r_isb) / min(r_spin, r_isb) <= 1.25,
                  f"cost ratio of fencedstore to acqrel by wait strategy: {every}; spin "
                  f"against isb {max(r_spin, r_isb) / min(r_spin, r_isb):.4f}, band 1.25")

    if not v["a"][0]:
        v["d"] = (None, "UNEVALUABLE, gated by (a)")
    else:
        worst, bad_cells = 0.0, []
        for (a, w), (cy, an, fe, be) in sorted(cells.items()):
            parts = fe + be
            r = max(an, parts) / min(an, parts) if min(an, parts) else float("inf")
            worst = max(worst, r)
            if r > 1.10:
                bad_cells.append(f"{a} {w} {r:.4f}")
        v["d"] = (worst <= 1.10,
                  f"frontend plus backend against stall_any, largest departure {worst:.4f}, "
                  f"band 1.10"
                  + (f"; outside in {', '.join(bad_cells)}" if bad_cells else
                     "; every cell inside, so the counters do partition here"))

    bad = [f"{a} {w} {ok[(a, w)][0]}/{ok[(a, w)][1]}"
           for a in ARMS for w in WAITS
           if (a, w) in cells and ok[(a, w)][0] != ok[(a, w)][1]]
    v["e"] = (not bad, "; ".join(bad) if bad else
              "every record of all eight cells reports a correct sum")

    if "--emit" in sys.argv:
        for (a, w), (cy, an, fe, be) in sorted(cells.items()):
            print(f"h88_cell arm={a} wait={w} msg_cycles={cy:.3f} stall_any={an:.4f} "
                  f"stall_frontend={fe:.4f} stall_backend={be:.4f} "
                  f"frontend_share={fe / cy:.4f} backend_share={be / cy:.4f}")
        for w in WAITS:
            if ("fencedstore", w) in cells and ("acqrel", w) in cells:
                print(f"h88_ratio wait={w} "
                      f"cycles_fencedstore_over_acqrel="
                      f"{cells[('fencedstore', w)][0] / cells[('acqrel', w)][0]:.4f} "
                      f"backend_fencedstore_over_acqrel="
                      f"{cells[('fencedstore', w)][3] / cells[('acqrel', w)][3]:.4f}")
        # Part (c) is decided by a ratio of two of the ratios above, and the
        # manuscript quotes it. A quoted figure has to exist in a file rather
        # than be recoverable from one by division, so it is emitted here. The
        # line carries its own prefix so a file holding the twelve lines above
        # can be topped up without rewriting them.
        if ("fencedstore", "spin") in cells and ("fencedstore", "isb") in cells:
            rs = cells[("fencedstore", "spin")][0] / cells[("acqrel", "spin")][0]
            ri = cells[("fencedstore", "isb")][0] / cells[("acqrel", "isb")][0]
            print(f"h88_stability cost_ratio_spin={rs:.4f} cost_ratio_isb={ri:.4f} "
                  f"spin_over_isb={max(rs, ri) / min(rs, ri):.4f} band=1.25")
        return 0

    print("REGISTERED VERDICTS, guard before the claims it gates")
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
    print()
    print("Read the H88 row of docs/experiments.md for what these license, and D40")
    print("for which earlier runs the wait strategy confound reaches.")
    return 0


if __name__ == "__main__":
    sys.exit(main())
