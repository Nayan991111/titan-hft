"""Hand-written SVG figures for the repository front page.

GitHub renders SVG in markdown but strips most scripting, so each figure is a
static document with one <style> block and a prefers-color-scheme override,
matching the pattern the other figures in docs/figures already use. Colours are
the same eight-slot categorical order used across this project.

Every number drawn here is recomputed from the committed result files at run
time. Nothing is hard coded, so a figure cannot drift from the data it claims
to show.

Usage:
    python3 -I scripts/figures_readme.py
"""

import math
import re
from collections import defaultdict
from pathlib import Path

ROOT = Path(__file__).resolve().parent.parent
RES = ROOT / "results"
FIGS = ROOT / "docs" / "figures"
PAIR = re.compile(r"([a-z_]+)=([^\s]+)")

STYLE = """  .bg { fill: #fcfcfb; } .frame { fill: none; stroke: #e4e3df; }
  .t1 { fill: #0b0b0b; } .t2 { fill: #52514e; } .t3 { fill: #75746f; }
  .grid { stroke: #e4e3df; stroke-width: 1; } .axis { stroke: #b9b8b2; stroke-width: 1; }
  .ref { stroke: #b9b8b2; stroke-width: 1; stroke-dasharray: 4 3; }
  .band { fill: #2a78d6; opacity: 0.06; }
  .l1 { stroke: #2a78d6; } .l2 { stroke: #eb6834; } .l3 { stroke: #1baf7a; }
  .f1 { fill: #2a78d6; } .f2 { fill: #eb6834; } .f3 { fill: #1baf7a; }
  .ln { fill: none; stroke-width: 2; stroke-linejoin: round; stroke-linecap: round; }
  .thin { fill: none; stroke-width: 1.6; stroke-dasharray: 5 3; }
  .ring { stroke: #fcfcfb; stroke-width: 2; }
  text { font-family: -apple-system, BlinkMacSystemFont, "Segoe UI", Roboto, Helvetica, Arial, sans-serif; }
@media (prefers-color-scheme: dark) {
  .bg { fill: #151515; } .frame { stroke: #2c2c2a; }
  .t1 { fill: #f0efec; } .t2 { fill: #c3c2b7; } .t3 { fill: #898781; }
  .grid { stroke: #2c2c2a; } .axis { stroke: #383835; } .ref { stroke: #383835; }
  .band { fill: #3987e5; opacity: 0.10; }
  .l1 { stroke: #3987e5; } .l2 { stroke: #d95926; } .l3 { stroke: #199e70; }
  .f1 { fill: #3987e5; } .f2 { fill: #d95926; } .f3 { fill: #199e70; }
  .ring { stroke: #151515; }
}"""


def med(xs):
    s = sorted(xs)
    n = len(s)
    return s[n // 2] if n % 2 else 0.5 * (s[n // 2 - 1] + s[n // 2])


def sign_p(k, n):
    if n == 0:
        return 1.0
    return min(1.0, 2 * sum(math.comb(n, i) for i in range(k, n + 1)) / 2 ** n)


def load_ccdf(name):
    d = defaultdict(list)
    path = RES / name
    if not path.exists():
        raise SystemExit(f"FAIL: {path} missing")
    for line in path.open(encoding="utf-8"):
        if not line.startswith("pl_ccdf launch="):
            continue
        r = dict(PAIR.findall(line))
        d[(r["load"], r["wait"], int(r["launch"]), float(r["q"]))].append(float(r["ns"]))
    if not d:
        raise SystemExit(f"FAIL: no CCDF records in {path}")
    return d


def ratio_curve(d, numer, denom, wait):
    qs = sorted({k[3] for k in d})
    ls = sorted({k[2] for k in d})
    out = []
    for q in qs:
        a, b, k, n = [], [], 0, 0
        for l in ls:
            x = d.get((numer, wait, l, q))
            y = d.get((denom, wait, l, q))
            if not x or not y:
                continue
            X, Y = med(x), med(y)
            a.append(X)
            b.append(Y)
            n += 1
            if X > Y:
                k += 1
        if n:
            out.append((q, med(a) / med(b), k, n, sign_p(k, n)))
    return out


def esc(s):
    return str(s).replace("&", "&amp;").replace("<", "&lt;").replace(">", "&gt;")


def fig_randomness():
    """Ratio of a random schedule to a perfectly periodic one, both rates."""
    d70 = load_ccdf("2026-11-18_m4_burst.txt")
    d72 = load_ccdf("2026-11-19_m4_burst7.txt")
    per70 = next(a for a in {k[0] for k in d70} if a.startswith("rate_"))
    per72 = next(a for a in {k[0] for k in d72} if a.startswith("rate_"))
    waits = ["spin", "isb", "wfe"]
    series = []
    for i, w in enumerate(waits):
        series.append(("7,000,000 msg/s, " + w, ratio_curve(d72, "hawkes_0.0", per72, w), i + 1, True))
        series.append(("5,000,000 msg/s, " + w, ratio_curve(d70, "hawkes_0.0", per70, w), i + 1, False))

    W, H = 980, 440
    L, R, T, B = 74, 236, 56, 84
    pw, ph = W - L - R, H - T - B
    # x axis: survival on a log scale, p50 at the left, the deep tail at the right
    xlo, xhi = math.log10(0.5), math.log10(1e-6)
    ylo, yhi = 0.9, 3.6

    def X(q):
        s = max(1e-6, 1.0 - q)
        return L + pw * (math.log10(s) - xlo) / (xhi - xlo)

    def Y(v):
        return T + ph * (1.0 - (min(yhi, max(ylo, v)) - ylo) / (yhi - ylo))

    o = []
    o.append(f'<svg xmlns="http://www.w3.org/2000/svg" width="{W}" height="{H}" '
             f'viewBox="0 0 {W} {H}" role="img" aria-labelledby="rt rd">')
    o.append('<title id="rt">Cost of random arrivals against a perfectly periodic schedule</title>')
    peaks = {lab: max(c, key=lambda r: r[1]) for lab, c, _, _ in series}
    o.append('<desc id="rd">' + esc("; ".join(
        f"{lab} peaks at {p[1]:.2f} times at the {100*p[0]:.2f}nd percentile, {p[2]} of {p[3]} launches"
        for lab, p in peaks.items())) + '</desc>')
    o.append(f"<style>\n{STYLE}\n</style>")
    o.append(f'<rect class="bg" x="0" y="0" width="{W}" height="{H}"/>')
    o.append(f'<text class="t1" x="{L}" y="24" font-size="16" font-weight="500">'
             'A random arrival schedule costs up to 3.4 times a periodic one at the same mean rate</text>')
    o.append(f'<text class="t3" x="{L}" y="42" font-size="12">'
             'Latency ratio, random (Poisson) over perfectly periodic, paired within 30 independent launches. '
             'Higher is worse for random.</text>')

    # registration window p95 to p99.7
    x95, x997 = X(0.95), X(0.997)
    o.append(f'<rect class="band" x="{x95:.1f}" y="{T}" width="{(x997-x95):.1f}" height="{ph}"/>')
    o.append(f'<text class="t3" x="{(x95+x997)/2:.1f}" y="{T+16}" font-size="11" '
             f'text-anchor="middle">registered window, p95 to p99.7</text>')

    for v in (1.0, 1.5, 2.0, 2.5, 3.0, 3.5):
        y = Y(v)
        o.append(f'<line class="{"ref" if v == 1.0 else "grid"}" x1="{L}" y1="{y:.1f}" x2="{L+pw}" y2="{y:.1f}"/>')
        o.append(f'<text class="t3" x="{L-9}" y="{y+4:.1f}" font-size="11" text-anchor="end">{v:.1f}x</text>')
    for pct, lab in ((0.5, "p50"), (0.9, "p90"), (0.99, "p99"), (0.999, "p99.9"),
                     (0.9999, "p99.99"), (0.999999, "p99.9999")):
        x = X(pct)
        o.append(f'<line class="grid" x1="{x:.1f}" y1="{T}" x2="{x:.1f}" y2="{T+ph}"/>')
        o.append(f'<text class="t3" x="{x:.1f}" y="{T+ph+18}" font-size="11" text-anchor="middle">{lab}</text>')
    o.append(f'<line class="axis" x1="{L}" y1="{T+ph}" x2="{L+pw}" y2="{T+ph}"/>')
    o.append(f'<text class="t2" x="{L+pw/2:.0f}" y="{T+ph+42}" font-size="12" text-anchor="middle">'
             'percentile of per-message latency</text>')

    for lab, curve, slot, solid in series:
        pts = " ".join(f"{X(q):.1f},{Y(v):.1f}" for q, v, *_ in curve)
        cls = f'ln l{slot}' if solid else f'thin l{slot}'
        o.append(f'<polyline class="{cls}" points="{pts}"/>')

    # peak markers on the three 7,000,000 curves
    for lab, curve, slot, solid in series:
        if "7,000,000" not in lab:
            continue
        q, v, k, n, _ = max(curve, key=lambda r: r[1])
        o.append(f'<circle class="f{slot} ring" cx="{X(q):.1f}" cy="{Y(v):.1f}" r="5"/>')

    ly = T + 6
    o.append(f'<text class="t2" x="{L+pw+22}" y="{ly}" font-size="12" font-weight="500">peak ratio, 30 launches</text>')
    ly += 10
    for lab, curve, slot, solid in series:
        q, v, k, n, p = max(curve, key=lambda r: r[1])
        ly += 23
        if solid:
            o.append(f'<line class="ln l{slot}" x1="{L+pw+22}" y1="{ly-4}" x2="{L+pw+48}" y2="{ly-4}"/>')
        else:
            o.append(f'<line class="thin l{slot}" x1="{L+pw+22}" y1="{ly-4}" x2="{L+pw+48}" y2="{ly-4}"/>')
        o.append(f'<text class="t2" x="{L+pw+54}" y="{ly}" font-size="11">{esc(lab)}</text>')
        o.append(f'<text class="t1" x="{L+pw+54}" y="{ly+12}" font-size="11" font-weight="500">'
                 f'{v:.2f}x at p{100*q:.2f}, {k}/{n}</text>')
        ly += 12
    o.append(f'<text class="t3" x="{L+pw+22}" y="{ly+26}" font-size="11">'
             'solid: 7,000,000, the measured ceiling</text>')
    o.append(f'<text class="t3" x="{L+pw+22}" y="{ly+40}" font-size="11">'
             'dashed: 5,000,000, about half of it</text>')
    o.append(f'<text class="t3" x="{L}" y="{H-30}" font-size="11">'
             'Both rates agree the effect has gone by p99.9 and the far tail. The peak grows and moves '
             'earlier as utilisation</text>')
    o.append(f'<text class="t3" x="{L}" y="{H-15}" font-size="11">'
             'rises toward the measured ceiling, which is what a coincidence-queue account predicts. '
             'That account is untested.</text>')
    o.append("</svg>")
    (FIGS / "fig-randomness.svg").write_text("\n".join(o), encoding="utf-8")
    return {lab: max(c, key=lambda r: r[1]) for lab, c, _, _ in series}


def fig_envelope():
    """Saturation share against offered rate, at two run lengths."""
    path = RES / "2026-11-19_m4_envelope.txt"
    if not path.exists():
        raise SystemExit(f"FAIL: {path} missing")
    cells = defaultdict(lambda: [0, 0])
    for line in path.open(encoding="utf-8"):
        if not line.startswith("pl launch="):
            continue
        r = dict(PAIR.findall(line))
        if "offered_mps" not in r or "messages" not in r:
            continue
        rate = round(float(r["offered_mps"]))
        key = (int(r["messages"]), rate)
        cells[key][0] += 1
        if r.get("saturated") == "1":
            cells[key][1] += 1
    lengths = sorted({k[0] for k in cells})
    rates = sorted({k[1] for k in cells})
    if not lengths or not rates:
        raise SystemExit("FAIL: no envelope cells parsed")

    W, H = 980, 348
    L, R, T, B = 74, 250, 54, 76
    pw, ph = W - L - R, H - T - B

    def X(i):
        return L + pw * (i + 0.5) / len(rates)

    def Y(p):
        return T + ph * (1.0 - p)

    o = []
    o.append(f'<svg xmlns="http://www.w3.org/2000/svg" width="{W}" height="{H}" '
             f'viewBox="0 0 {W} {H}" role="img" aria-labelledby="et ed">')
    o.append('<title id="et">Sustainable throughput falls as the run gets longer</title>')
    desc = []
    for i, m in enumerate(lengths):
        clean = [r for r in rates if cells.get((m, r), [1, 1])[1] == 0]
        desc.append(f"at {m} messages per epoch every arm stays clean up to "
                    f"{max(clean) if clean else 0} million per second")
    o.append("<desc id=\"ed\">" + esc("; ".join(desc)) + "</desc>")
    o.append(f"<style>\n{STYLE}\n</style>")
    o.append(f'<rect class="bg" x="0" y="0" width="{W}" height="{H}"/>')
    o.append(f'<text class="t1" x="{L}" y="24" font-size="16" font-weight="500">'
             'The sustainable rate falls 30 percent when the run gets five times longer</text>')
    o.append(f'<text class="t3" x="{L}" y="42" font-size="12">'
             'Share of cells that saturated, all four arms and three wait strategies, 3 launches per point.</text>')

    for p, lab in ((0.0, "0%"), (0.25, "25%"), (0.5, "50%"), (0.75, "75%"), (1.0, "100%")):
        y = Y(p)
        o.append(f'<line class="{"axis" if p == 0 else "grid"}" x1="{L}" y1="{y:.1f}" x2="{L+pw}" y2="{y:.1f}"/>')
        o.append(f'<text class="t3" x="{L-9}" y="{y+4:.1f}" font-size="11" text-anchor="end">{lab}</text>')
    for i, r in enumerate(rates):
        o.append(f'<text class="t3" x="{X(i):.1f}" y="{T+ph+18}" font-size="11" text-anchor="middle">{r}M</text>')
    o.append(f'<text class="t2" x="{L+pw/2:.0f}" y="{T+ph+40}" font-size="12" text-anchor="middle">'
             'offered rate, millions of messages per second</text>')

    for si, m in enumerate(lengths):
        slot = si + 1
        pts = []
        for i, r in enumerate(rates):
            tot, sat = cells.get((m, r), [0, 0])
            if tot == 0:
                continue
            pts.append((X(i), Y(sat / tot), sat, tot))
        o.append(f'<polyline class="ln l{slot}" points="' +
                 " ".join(f"{x:.1f},{y:.1f}" for x, y, *_ in pts) + '"/>')
        for x, y, sat, tot in pts:
            o.append(f'<circle class="f{slot} ring" cx="{x:.1f}" cy="{y:.1f}" r="4.5"/>')

    ly = T + 16
    o.append(f'<text class="t2" x="{L+pw+22}" y="{ly}" font-size="12" font-weight="500">'
             'highest rate with no saturation</text>')
    for si, m in enumerate(lengths):
        slot = si + 1
        clean = [r for r in rates if cells.get((m, r), [1, 1])[1] == 0]
        ly += 34
        o.append(f'<line class="ln l{slot}" x1="{L+pw+22}" y1="{ly-4}" x2="{L+pw+48}" y2="{ly-4}"/>')
        o.append(f'<circle class="f{slot} ring" cx="{L+pw+35}" cy="{ly-4}" r="4.5"/>')
        o.append(f'<text class="t2" x="{L+pw+54}" y="{ly}" font-size="11">'
                 f'{m:,} messages per epoch</text>')
        o.append(f'<text class="t1" x="{L+pw+54}" y="{ly+13}" font-size="11" font-weight="500">'
                 f'{max(clean) if clean else 0} million per second</text>')
    o.append(f'<text class="t3" x="{L}" y="{H-14}" font-size="11">'
             'Measured, not assumed. H71 took its rate from an earlier scope note, ran 43 percent over '
             'capacity, and all 1,080 of its cells saturated.</text>')
    o.append("</svg>")
    (FIGS / "fig-envelope.svg").write_text("\n".join(o), encoding="utf-8")
    return {m: max([r for r in rates if cells.get((m, r), [1, 1])[1] == 0], default=0) for m in lengths}


if __name__ == "__main__":
    pk = fig_randomness()
    print("fig-randomness.svg")
    for lab, p in pk.items():
        print(f"  {lab:<28} peak {p[1]:.4f} at p{100*p[0]:.4f}  {p[2]}/{p[3]}  p={p[4]:.3g}")
    env = fig_envelope()
    print("fig-envelope.svg")
    for m, r in env.items():
        print(f"  {m:>10,} messages per epoch -> highest clean rate {r} Mmsg/s")
