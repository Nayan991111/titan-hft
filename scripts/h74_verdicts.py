"""H74 verdicts: does the tail penalty track cv squared or the Fano factor.

Thresholds were fixed and pushed as 49c710b before any of this data existed.
This file reads the raw records with its own parsing, its own median and its
own exact binomial, and reports each registered part as supported or refuted
without reference to what the manuscript says.

    python3 -I scripts/h74_verdicts.py
"""

import math
import sys
from collections import defaultdict
from pathlib import Path

ROOT = Path(__file__).resolve().parent.parent
FILE = ROOT / "results" / "2026-11-20_m4_h74_beta.txt"
WAITS = ("spin", "isb", "wfe")
LO = "hbeta_0.8_1e5"
LADDER = ["hbeta_0.8_1e6", "hbeta_0.8_3e6", "hbeta_0.8_2.5e7"]
HI = "hbeta_0.8_2.5e7"
CV2 = {"hbeta_0.8_1e5": 1.1005, "hbeta_0.8_1e6": 2.3620,
       "hbeta_0.8_3e6": 4.7086, "hbeta_0.8_2.5e7": 8.1921}


def fields(line):
    f = {}
    for tok in line.split():
        if "=" in tok:
            k, _, v = tok.partition("=")
            f[k] = v
    return f


def med(xs):
    xs = sorted(xs)
    h = len(xs) // 2
    return xs[h] if len(xs) % 2 else (xs[h - 1] + xs[h]) / 2.0


def sign_p(k, n):
    if n == 0:
        return 1.0
    tail = sum(math.comb(n, i) for i in range(k, n + 1))
    return min(1.0, 2.0 * tail / float(2 ** n))


def load_ccdf():
    d = defaultdict(list)
    for line in FILE.open(encoding="utf-8", errors="replace"):
        if not line.startswith("pl_ccdf "):
            continue
        f = fields(line)
        if not {"load", "wait", "launch", "q", "ns"} <= f.keys():
            continue
        d[(f["load"], f["wait"], int(f["launch"]), float(f["q"]))].append(float(f["ns"]))
    return d


def load_pl():
    rows = []
    for line in FILE.open(encoding="utf-8", errors="replace"):
        if not line.startswith("pl "):
            continue
        f = fields(line)
        if "load" not in f:
            continue
        rows.append(f)
    return rows


def curve(d, top, bottom, wait):
    """Per quantile: median over launches of each side, their ratio, and the
    per-launch sign count. Epochs are collapsed by median first."""
    out = []
    launches = sorted({k[2] for k in d})
    for q in sorted({k[3] for k in d}):
        hi, lo, wins, n = [], [], 0, 0
        for l in launches:
            a = d.get((top, wait, l, q))
            b = d.get((bottom, wait, l, q))
            if not a or not b:
                continue
            A, B = med(a), med(b)
            hi.append(A)
            lo.append(B)
            n += 1
            wins += A > B
        if n:
            out.append((q, med(hi) / med(lo), wins, n, sign_p(wins, n), med(hi), med(lo)))
    return out


def _skipcurve(d, top, bottom, wait, skip):
    out = []
    launches = [l for l in sorted({k[2] for k in d}) if l not in skip]
    for q in sorted({k[3] for k in d}):
        hi, lo, wins, n = [], [], 0, 0
        for l in launches:
            a = d.get((top, wait, l, q))
            b = d.get((bottom, wait, l, q))
            if not a or not b:
                continue
            A, B = med(a), med(b)
            hi.append(A)
            lo.append(B)
            n += 1
            wins += A > B
        if n:
            out.append((q, med(hi) / med(lo), wins, n, sign_p(wins, n), med(hi), med(lo)))
    return out


def main():
    d = load_ccdf()
    pl = load_pl()
    arms = sorted({r["load"] for r in pl})
    print(f"file     : {FILE.name}")
    print(f"arms     : {len(arms)}  {arms}")
    print(f"launches : {len({int(r['launch']) for r in pl})}")
    print(f"pl rows  : {len(pl)}   ccdf cells: {len(d)}")
    print()

    verdicts = {}

    # (a) and (b): the headline contrast at the quantile where it peaks, spin.
    peaks = {}
    print("PEAK CONTRAST, beta 2.5e7 against beta 1e5, per wait")
    print(f"  {'wait':<6} {'peak q':>12} {'ratio':>8} {'wins':>8} {'p':>12}")
    for w in WAITS:
        c = curve(d, HI, LO, w)
        if not c:
            print(f"  {w:<6}  no paired cells")
            continue
        pk = max(c, key=lambda r: r[1])
        peaks[w] = pk
        print(f"  {w:<6} {pk[0]:>12.6f} {pk[1]:>8.4f} {pk[2]:>5}/{pk[3]:<3} {pk[4]:>12.3g}")
    print()

    pk = peaks["spin"]
    verdicts["a"] = (pk[2] >= 21,
                     f"spin peak sign count {pk[2]} of {pk[3]}, p={pk[4]:.3g}, "
                     f"threshold 21 of 30. The Fano account predicts 15 of 30.")
    verdicts["b"] = (pk[1] >= 1.25,
                     f"spin peak median ratio {pk[1]:.4f}, threshold 1.25.")

    # (c) monotone non-decreasing in beta, at each arm's own peak against 1e5.
    print("LADDER AGAINST beta 1e5, spin, each at its own peak quantile")
    print(f"  {'arm':<22} {'cv2':>7} {'peak q':>12} {'ratio':>8} {'wins':>8} {'p':>12}")
    seq = []
    for arm in LADDER:
        c = curve(d, arm, LO, "spin")
        p = max(c, key=lambda r: r[1])
        seq.append((arm, p[1], p))
        print(f"  {arm:<22} {CV2[arm]:>7.4f} {p[0]:>12.6f} {p[1]:>8.4f} {p[2]:>5}/{p[3]:<3} {p[4]:>12.3g}")
    mono = all(seq[i][1] <= seq[i + 1][1] + 1e-12 for i in range(len(seq) - 1))
    verdicts["c"] = (mono,
                     "median peak ratio across beta 1e6, 3e6, 2.5e7 is "
                     + ("monotone non-decreasing: " if mono else "NOT monotone: ")
                     + ", ".join(f"{s[1]:.4f}" for s in seq))
    print()

    # (d) the primary test on isb and wfe.
    ok_d = all(peaks[w][2] >= 21 for w in ("isb", "wfe"))
    verdicts["d"] = (ok_d, "isb " + f"{peaks['isb'][2]}/{peaks['isb'][3]}"
                     + ", wfe " + f"{peaks['wfe'][2]}/{peaks['wfe'][3]}"
                     + ", threshold 21 of 30 on each.")

    # (e) rate fidelity and saturation.
    worst, sat = 0.0, []
    for r in pl:
        if r["load"] not in CV2:
            continue
        off, ach = float(r["offered_mps"]), float(r["achieved_mps"])
        worst = max(worst, abs(ach - off) / off)
        if r.get("saturated") == "1":
            sat.append((r["load"], r["wait"], r["launch"], r["epoch"]))
    sat_all = [(r["load"], r["wait"], r["launch"]) for r in pl if r.get("saturated") == "1"]
    verdicts["e"] = (worst <= 0.01 and not sat_all,
                     f"worst achieved-against-offered gap on a beta arm {100 * worst:.3f} percent "
                     f"(threshold 1 percent); saturated cells in the whole run {len(sat_all)} "
                     f"{sat_all}; on a beta arm {len(sat)}.")

    # (f) control: a small cv2 gap must give a smaller ratio than a large one.
    small = max(curve(d, "hbeta_0.8_1e6", LO, "spin"), key=lambda r: r[1])[1]
    large = pk[1]
    verdicts["f"] = (small < large,
                     f"cv2 gap 2.1 gives {small:.4f}, cv2 gap 7.4 gives {large:.4f}; "
                     "the effect must grow with the gap.")

    if "--emit" in sys.argv:
        # Record lines, so every number the manuscript quotes from this
        # analysis lives in a results file rather than only in a terminal.
        # barrier_stats.py established this pattern for the barrier run.
        print("h74_pred cv2_lo=1.1005 cv2_hi=8.1921 cv2_ratio=7.444 "
              "fano_lo=24.79 fano_hi=25.78 fano_ratio=1.040")
        for w in WAITS:
            c2 = curve(d, HI, LO, w)
            p2 = max(c2, key=lambda r: r[1])
            print(f"h74_peak wait={w} q={p2[0]:.9f} ratio={p2[1]:.4f} "
                  f"hi_ns={p2[5]:.1f} lo_ns={p2[6]:.1f} wins={p2[2]} n={p2[3]} p={p2[4]:.3g}")
        for qq in (0.5, 0.9, 0.99, 0.999):
            r2 = min(curve(d, HI, LO, "spin"), key=lambda r: abs(r[0] - qq))
            print(f"h74_quantile wait=spin target={qq} q={r2[0]:.9f} ratio={r2[1]:.4f} "
                  f"hi_ns={r2[5]:.1f} lo_ns={r2[6]:.1f} wins={r2[2]} n={r2[3]}")
        for arm in LADDER:
            r2 = max(curve(d, arm, LO, "spin"), key=lambda r: r[1])
            print(f"h74_ladder arm={arm} cv2={CV2[arm]:.4f} q={r2[0]:.9f} "
                  f"ratio={r2[1]:.4f} wins={r2[2]} n={r2[3]}")
        for arm in ("poisson", "rate_5000000"):
            r2 = max(curve(d, arm, LO, "spin"), key=lambda r: r[1])
            print(f"h74_reference arm={arm} q={r2[0]:.9f} ratio={r2[1]:.4f} "
                  f"wins={r2[2]} n={r2[3]}")
        for sk, lab in ((set(), "all"), ({3}, "no_launch_3"), ({3, 27}, "no_launch_3_27")):
            r2 = max(_skipcurve(d, HI, LO, "spin", sk), key=lambda r: r[1])
            print(f"h74_robust drop={lab} ratio={r2[1]:.4f} wins={r2[2]} n={r2[3]} p={r2[4]:.3g}")
        return 0

    print("REGISTERED VERDICTS")
    for k in "abcdef":
        ok, why = verdicts[k]
        print(f"  ({k}) {'SUPPORTED' if ok else 'REFUTED  '}  {why}")
    print()
    n_ok = sum(1 for k in "abcdef" if verdicts[k][0])
    print(f"{n_ok} of 6 registered parts supported")
    return 0


if __name__ == "__main__":
    sys.exit(main())
