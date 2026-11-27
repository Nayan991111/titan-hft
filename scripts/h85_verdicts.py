"""H85: which property of the ring governs the H84 break.

H84 left three candidates moving together: the slot count, the bytes the
slots occupy, and how far the producer may run ahead. The padsweep crosses
two capacities with two slot sizes, so footprint moves by a factor of
sixteen while the slot count and the run-ahead are held.

Thresholds were fixed and pushed before this data existed. Part (b) is
evaluated only if the guard in part (a) holds, which is the D29 discipline:
the manipulation is verified in the data and not assumed from the flag.

    python3 -I scripts/h85_verdicts.py [--emit]
"""

import math
import sys
from collections import defaultdict
from pathlib import Path

ROOT = Path(__file__).resolve().parent.parent
FILE = ROOT / "results" / "2026-11-27_m4_h85_padsweep.txt"
ARMS = ("acqrel", "fencedstore")
SLOTS = ("8", "128")
CAPS = ("8", "16")


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
    foot_ok = foot_n = 0
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
        # The manipulation is checked in the record, not trusted from the flag.
        if "slot_footprint" in f and "slot_bytes" in f and "cap" in f:
            foot_n += 1
            foot_ok += int(int(f["slot_footprint"])
                           == int(f["cap"]) * int(f["slot_bytes"]))
        key = (f["order"], f.get("slot_bytes", "?"), f.get("cap", "?"))
        for k in ("msg_cycles", "msg_l1d_writeback", "msg_l1d_refill",
                  "msg_ldst_wait_cycles", "ns_per_msg"):
            if k in f:
                per[key][k].append(float(f[k]))
                byl[(key, f.get("launch", "0"))][k].append(float(f[k]))
        ok[key][0] += int(f.get("sum_ok") == "1")
        ok[key][1] += 1

    print(f"file     : {FILE.name}")
    print(f"arms={arms_mode}  counters={counters}  measured_side={side}")
    if arms_mode != "padsweep":
        print(f"FAIL: H85 requires arms=padsweep, file says {arms_mode}")
        return 1
    if counters != "trafficprobe":
        print(f"FAIL: H85 requires counters=trafficprobe, file says {counters}")
        return 1
    print()
    print(f"  {'arm':<13} {'slot':>5} {'cap':>4} {'foot':>6} {'cycles':>10} "
          f"{'wrbk':>8} {'refill':>8} {'ns/msg':>9} {'ok':>9}")
    cells = []
    for a in ARMS:
        for s in SLOTS:
            for cp in CAPS:
                q = per[(a, s, cp)]
                if not q:
                    continue
                cyc, wb = med(q["msg_cycles"]), med(q["msg_l1d_writeback"])
                rf = med(q["msg_l1d_refill"])
                cells.append((a, s, cp, cyc, wb, rf))
                print(f"  {a:<13} {s:>5} {cp:>4} {int(s) * int(cp):>6} "
                      f"{cyc:>10.3f} {wb:>8.4f} {rf:>8.4f} "
                      f"{med(q['ns_per_msg']):>9.3f} "
                      f"{ok[(a, s, cp)][0]:>4}/{ok[(a, s, cp)][1]:<4}")
    print()

    launches = sorted({l for (_k, l) in byl})

    def ratio_by_launch(arm, slot, num_cap, den_cap):
        """Per-launch ratio of medians, numerator capacity over denominator."""
        out = []
        for L in launches:
            qn = byl[((arm, slot, num_cap), L)]
            qd = byl[((arm, slot, den_cap), L)]
            if not qn or not qd:
                continue
            out.append(med(qn["msg_cycles"]) / med(qd["msg_cycles"]))
        return out

    def pooled(arm, slot, cap):
        q = per[(arm, slot, cap)]
        return med(q["msg_cycles"]) if q else None

    v = {}
    guard_cells = len(cells) == 8 and all(wb > 0 and rf > 0
                                          for _, _, _, _, wb, rf in cells)
    guard_foot = foot_n > 0 and foot_ok == foot_n
    v["a"] = (guard_cells and guard_foot,
              f"{len(cells)} of 8 cells, "
              f"{sum(1 for _, _, _, _, wb, rf in cells if wb > 0 and rf > 0)} with "
              f"both writebacks and refills above zero; slot footprint equals "
              f"capacity times slot size in {foot_ok} of {foot_n} records")

    if v["a"][0] is not True:
        v["b"] = (None, "UNEVALUABLE, the guard in part (a) does not hold")
    else:
        rs = ratio_by_launch("acqrel", "128", "16", "8")
        hit = sum(1 for x in rs if x >= 2.0)
        hi, lo = pooled("acqrel", "128", "16"), pooled("acqrel", "128", "8")
        v["b"] = (hit >= 17,
                  f"with the 128 byte slot acqrel costs {hi:.3f} cycles at capacity "
                  f"16 against {lo:.3f} at capacity 8, pooled ratio {hi / lo:.4f}; "
                  f"per launch the ratio runs {min(rs):.4f} to {max(rs):.4f} with "
                  f"median {med(rs):.4f} and reaches 2.0 in {hit} of {len(rs)} "
                  f"launches, threshold 17 of 20")

    rc = ratio_by_launch("fencedstore", "128", "16", "8")
    hic, loc = pooled("fencedstore", "128", "16"), pooled("fencedstore", "128", "8")
    inside = sum(1 for x in rc if 1.0 / 1.25 <= x <= 1.25)
    v["c"] = (bool(rc) and 1.0 / 1.25 <= hic / loc <= 1.25,
              f"with the 128 byte slot fencedstore costs {hic:.3f} cycles at capacity "
              f"16 against {loc:.3f} at capacity 8, pooled ratio {hic / loc:.4f}, "
              f"inside the band of 1.25 in {inside} of {len(rc)} launches")

    bad = [f"{a} slot {s} cap {cp} {ok[(a, s, cp)][0]}/{ok[(a, s, cp)][1]}"
           for a in ARMS for s in SLOTS for cp in CAPS
           if per[(a, s, cp)] and ok[(a, s, cp)][0] != ok[(a, s, cp)][1]]
    v["d"] = (not bad, "; ".join(bad) if bad else
              "every record of all eight cells reports a correct sum")

    if "--emit" in sys.argv:
        for a, s, cp, c, wb, rf in cells:
            print(f"h85_cell arm={a} slot_bytes={s} cap={cp} "
                  f"slot_footprint={int(s) * int(cp)} msg_cycles={c:.3f} "
                  f"writeback={wb:.4f} refill={rf:.4f}")
        # The two capacity steps that the discriminator turns on, each held at
        # a fixed slot size so the bytes moved per message are constant across
        # the comparison and only the footprint differs in what it crosses.
        for s in SLOTS:
            for a in ARMS:
                rr = ratio_by_launch(a, s, "16", "8")
                if not rr:
                    continue
                hi, lo = pooled(a, s, "16"), pooled(a, s, "8")
                slower = sum(1 for x in rr if x > 1.0)
                print(f"h85_step arm={a} slot_bytes={s} "
                      f"cap8_cycles={lo:.3f} cap16_cycles={hi:.3f} "
                      f"ratio={hi / lo:.4f} slower_in={slower} of={len(rr)} "
                      f"p={sign_p(slower, len(rr)):.3g} "
                      f"reaching_2x={sum(1 for x in rr if x >= 2.0)}")
        # Traffic against footprint, which is where the mechanism is visible:
        # both counters saturate at one 128 byte line and stay flat across a
        # further factor of sixteen.
        for a in ARMS:
            for s in SLOTS:
                for cp in CAPS:
                    q = per[(a, s, cp)]
                    if not q:
                        continue
                    print(f"h85_traffic arm={a} slot_footprint={int(s) * int(cp)} "
                          f"writeback={med(q['msg_l1d_writeback']):.4f} "
                          f"refill={med(q['msg_l1d_refill']):.4f} "
                          f"msg_cycles={med(q['msg_cycles']):.3f}")
        # The arm comparison at each footprint. The sign at the smallest
        # footprint is not resolved by this data and the count says so.
        for s in SLOTS:
            for cp in CAPS:
                ca, cf = pooled("acqrel", s, cp), pooled("fencedstore", s, cp)
                if ca is None or cf is None:
                    continue
                w = n = 0
                for L in launches:
                    qa = byl[(("acqrel", s, cp), L)]
                    qf = byl[(("fencedstore", s, cp), L)]
                    if not qa or not qf:
                        continue
                    n += 1
                    w += med(qa["msg_cycles"]) > med(qf["msg_cycles"])
                print(f"h85_arms slot_footprint={int(s) * int(cp)} "
                      f"acqrel_cycles={ca:.3f} fencedstore_cycles={cf:.3f} "
                      f"ratio={ca / cf:.4f} acqrel_slower_in={w} of={n} "
                      f"p={sign_p(w, n):.3g}")
        # Launch to launch spread of the per-launch median, which is the
        # fanless clock and not the manipulation. The widest cell is the
        # denominator of the eight byte step.
        for a, s, cp, _c, _wb, _rf in cells:
            vals = [med(byl[((a, s, cp), L)]["msg_cycles"]) for L in launches
                    if byl[((a, s, cp), L)]]
            print(f"h85_spread arm={a} slot_footprint={int(s) * int(cp)} "
                  f"min={min(vals):.3f} median={med(vals):.3f} max={max(vals):.3f} "
                  f"max_over_min={max(vals) / min(vals):.2f}")
        return 0

    print("REGISTERED VERDICTS, guards before the claims they gate")
    for k in "abcd":
        o, why = v[k]
        tag = "UNEVALUABLE" if o is None else ("SUPPORTED  " if o else "REFUTED    ")
        print(f"  ({k}) {tag}  {why}")
    s_ = sum(1 for k in v if v[k][0] is True)
    r_ = sum(1 for k in v if v[k][0] is False)
    u_ = sum(1 for k in v if v[k][0] is None)
    assert s_ + r_ + u_ == len(v), "the tally does not account for every part"
    print()
    print(f"{s_} supported, {r_} refuted, {u_} unevaluable, of {len(v)} parts")
    # D31 again: the arithmetic belongs here, the reading of it belongs in the
    # register where it can be argued with. The refutation of (b) is the
    # informative outcome and the row says why.
    print()
    print("Read the H85 row of docs/experiments.md. In short: the break is absent")
    print("when the footprint is large at both capacities and present when the")
    print("capacity step crosses one 128 byte line, so the H84 sign change is a")
    print("cache geometry result and not a capacity result. The padded slot also")
    print("changes the bytes moved per message, which is why the discriminator is")
    print("a within-slot-size comparison and why no claim rests on the absolute")
    print("cost of the padded arms.")
    return 0


if __name__ == "__main__":
    sys.exit(main())
