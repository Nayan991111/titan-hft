"""H89: does H86's exclusion of the throttling account survive the removal of isb.

H86 refuted the account that a full barrier wins by suppressing the issue rate,
on the grounds that the fenced-store arm issues on 2.7363 times more cycles than
the acquire-release arm. H88 then measured the acquire-release arm 96.61 percent
frontend stalled under wait=isb and 5.42 percent under wait=spin. An issue-active
fraction measured under a context-synchronizing wait loop is a measurement of the
wait loop, so a ratio of two such fractions says little about either arm, and
H86's exclusion rests on exactly that ratio.

Thresholds were fixed and pushed before this data existed. Part (b) is the
deciding test and it has three outcomes, not two: at or above 1.10 keeps H86
standing, below 0.90 revives the throttling account and forces a retraction, and
between the two is inconclusive and downgrades H86 to unsupported.

    python3 -I scripts/h89_verdicts.py [--emit]
"""

import sys
from collections import defaultdict
from pathlib import Path

ROOT = Path(__file__).resolve().parent.parent
FILE = ROOT / "results" / "2026-11-23_m4_h89_waitissue.txt"
ARMS = ("acqrel", "fencedstore")
WAITS = ("spin", "isb", "yield", "wfe")
KEYS = ("msg_cycles", "msg_issue_active", "msg_stall_backend",
        "msg_retired_uops", "ns_per_msg", "core_ghz")


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
        print(f"FAIL: H89 requires arms=waitsweep, file says {arms_mode}")
        return 1
    if counters != "issueprobe":
        print(f"FAIL: H89 requires counters=issueprobe, file says {counters}")
        return 1
    print()
    print(f"  {'arm':<13} {'wait':<6} {'cycles':>9} {'issuing':>9} {'issue frac':>11} "
          f"{'backend':>9} {'uops':>9} {'ok':>9}")
    cells = {}
    for a in ARMS:
        for w in WAITS:
            q = per[(a, w)]
            if not q:
                continue
            cy = med(q["msg_cycles"])
            ia = med(q["msg_issue_active"])
            be = med(q["msg_stall_backend"])
            uo = med(q["msg_retired_uops"])
            cells[(a, w)] = (cy, ia, be, uo)
            print(f"  {a:<13} {w:<6} {cy:>9.3f} {ia:>9.4f} {ia / cy:>11.6f} "
                  f"{be:>9.4f} {uo:>9.2f} {ok[(a, w)][0]:>4}/{ok[(a, w)][1]:<4}")
    print()

    def frac(a, w):
        cy, ia, _, _ = cells[(a, w)]
        return ia / cy

    v = {}
    v["a"] = (len(cells) == 8 and all(ia > 0 and be > 0 and uo > 0
                                      for _, ia, be, uo in cells.values()),
              f"{len(cells)} of 8 cells, "
              f"{sum(1 for _, ia, be, uo in cells.values() if ia > 0 and be > 0 and uo > 0)}"
              f" with all three counters above zero")

    if not v["a"][0]:
        v["b"] = (None, "UNEVALUABLE, gated by (a)")
        v["c"] = (None, "UNEVALUABLE, gated by (a)")
    else:
        r = frac("fencedstore", "spin") / frac("acqrel", "spin")
        every = ", ".join(f"{w} {frac('fencedstore', w) / frac('acqrel', w):.4f}"
                          for w in WAITS if ("fencedstore", w) in cells)
        # Three outcomes, fixed in advance. None is reported here as a pass.
        if r >= 1.10:
            out = True
            reads = ("H86's exclusion of the throttling account stands on its own "
                     "and the paper may state that throttling is excluded")
        elif r < 0.90:
            out = False
            reads = ("the barrier arm issues on fewer cycles once the isb is gone, "
                     "the throttling account is live again, and the paper must "
                     "retract H86's exclusion")
        else:
            out = None
            reads = ("INCONCLUSIVE by the registered band. The two arms issue on "
                     "indistinguishable fractions of their cycles once the isb is "
                     "gone, so this measurement neither excludes the throttling "
                     "account nor revives it, and H86's exclusion is downgraded "
                     "from excluded to unsupported rather than refuted. The paper "
                     "must not state that throttling is excluded")
        v["b"] = (out,
                  f"issue-active fraction of fencedstore over acqrel under spin, "
                  f"{r:.4f}, against 1.10 to keep H86 and 0.90 to refute it; by wait "
                  f"strategy {every}. {reads}")
        s = frac("acqrel", "spin") / frac("acqrel", "isb")
        v["c"] = (s >= 2.0,
                  f"acqrel issue-active fraction under spin over isb, {s:.4f}, "
                  f"against a threshold of 2.0, from {frac('acqrel', 'spin'):.6f} "
                  f"under spin and {frac('acqrel', 'isb'):.6f} under isb. The isb "
                  f"suppressed the quantity H86 compared")

    bad = [f"{a} {w} {ok[(a, w)][0]}/{ok[(a, w)][1]}"
           for a in ARMS for w in WAITS
           if (a, w) in cells and ok[(a, w)][0] != ok[(a, w)][1]]
    v["d"] = (not bad, "; ".join(bad) if bad else
              "every record of all eight cells reports a correct sum")

    if "--emit" in sys.argv:
        for (a, w), (cy, ia, be, uo) in sorted(cells.items()):
            print(f"h89_cell arm={a} wait={w} msg_cycles={cy:.3f} "
                  f"issue_active={ia:.4f} stall_backend={be:.4f} "
                  f"retired_uops={uo:.2f} issue_fraction={ia / cy:.6f}")
        for w in WAITS:
            if ("fencedstore", w) in cells and ("acqrel", w) in cells:
                print(f"h89_ratio wait={w} issue_fraction_fencedstore_over_acqrel="
                      f"{frac('fencedstore', w) / frac('acqrel', w):.4f}")
        for a in ARMS:
            if (a, "spin") in cells and (a, "isb") in cells:
                print(f"h89_isbeffect arm={a} spin_over_isb_issue_fraction="
                      f"{frac(a, 'spin') / frac(a, 'isb'):.4f}")
        # The registered bounds, written here so that a manuscript sentence
        # about how far the measurement sat from one of them has its inputs in
        # a file rather than in a sentence. Part (b) carries three outcomes and
        # therefore two bounds, which is the provision this run needed.
        print("h89_band keep_h86_at_or_above=1.10 retract_h86_below=0.90 "
              "between_the_two_is_inconclusive=1 isb_effect_floor=2.0")
        return 0

    print("REGISTERED VERDICTS, guard before the claim it gates")
    for k in "abcd":
        o, why = v[k]
        tag = "INCONCLUSIVE" if o is None else ("SUPPORTED   " if o else "REFUTED     ")
        print(f"  ({k}) {tag}  {why}")
    s = sum(1 for k in v if v[k][0] is True)
    r = sum(1 for k in v if v[k][0] is False)
    u = sum(1 for k in v if v[k][0] is None)
    assert s + r + u == len(v), "the tally does not account for every part"
    print()
    print(f"{s} supported, {r} refuted, {u} inconclusive or unevaluable, "
          f"of {len(v)} parts")
    print()
    print("Read the H89 row of docs/experiments.md for what these license, D40 for")
    print("which earlier runs the wait strategy confound reaches, and D42 for why a")
    print("test placed on one cell decided a claim whose scope is every cell.")
    return 0


if __name__ == "__main__":
    sys.exit(main())
