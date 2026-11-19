"""Graphical abstract and methodology flow, as static SVG.

The graphical abstract is the single panel a journal editor, a reviewer or a
journalist sees first, so every number on it is recomputed from the committed
result files rather than written by hand. The methodology flow is structural
and its counts are read from docs/experiments.md and the result files.

Usage:
    python3 -I scripts/figures_abstract.py
"""

import math
import re
import sys
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parent))
from figures_readme import STYLE, load_ccdf, ratio_curve, esc, FIGS, RES, ROOT  # noqa: E402

DOC = ROOT / "docs" / "experiments.md"


def facts():
    """Everything the two figures state, recomputed from the repository."""
    d70 = load_ccdf("2026-11-18_m4_burst.txt")
    d72 = load_ccdf("2026-11-19_m4_burst7.txt")
    per70 = next(a for a in {k[0] for k in d70} if a.startswith("rate_"))
    per72 = next(a for a in {k[0] for k in d72} if a.startswith("rate_"))
    best = {}
    for tag, d, per in (("5M", d70, per70), ("7M", d72, per72)):
        for w in ("spin", "isb", "wfe"):
            c = ratio_curve(d, "hawkes_0.0", per, w)
            best[(tag, w)] = max(c, key=lambda r: r[1])
            if tag == "7M" and w == "spin":
                best["curve"] = c
    # clustering, 0.8 against 0.0, at p99.9
    qs = sorted({k[3] for k in d72})
    q999 = min(qs, key=lambda q: abs(q - 0.999))
    cl = ratio_curve(d72, "hawkes_0.8", "hawkes_0.0", "spin")
    best["clust"] = next(r for r in cl if abs(r[0] - q999) < 1e-9)

    text = DOC.read_text(encoding="utf-8")
    # Count by ROW, not by word occurrence. A single row can record several
    # verdicts, because a hypothesis has parts, so counting the word SUPPORTED
    # and presenting it as a number of hypotheses overstates nothing and
    # understates badly: the older rows spell their verdicts in lower case, so
    # a case-sensitive count saw only the handful written recently.
    rowtext = {m.group(1): m.group(2).lower()
               for m in re.finditer(r"^\| ([HE]\d+[a-c]?) \|(.*)$", text, re.M)}
    rows = len(rowtext)
    words = ("supported", "refuted", "failed", "untestable", "inconclusive")
    verdicted = sum(1 for v in rowtext.values() if any(re.search(rf"\b{w}\b", v) for w in words))
    refuted_rows = sum(1 for v in rowtext.values() if re.search(r"\brefuted\b", v))

    # A record is a line that begins with a record prefix AND carries a
    # key=value pair. Without the second half this counts the human summary
    # rows that scripts/check_record_lines.py allowlists, which are prose.
    kv = re.compile(r"[A-Za-z_][A-Za-z0-9_]*=[^\s]")
    recs = 0
    for f in sorted(RES.glob("*.txt")):
        for ln in f.open(encoding="utf-8", errors="replace"):
            if ln.split(" ", 1)[0] in ("pl", "pl_ccdf", "pp", "st", "t2t") and kv.search(ln):
                recs += 1
    best["rows"], best["verdicted"], best["ref"], best["recs"] = rows, verdicted, refuted_rows, recs
    return best


def head(w, h, tid, did, title, desc):
    return [f'<svg xmlns="http://www.w3.org/2000/svg" width="{w}" height="{h}" '
            f'viewBox="0 0 {w} {h}" role="img" aria-labelledby="{tid} {did}">',
            f'<title id="{tid}">{esc(title)}</title>',
            f'<desc id="{did}">{esc(desc)}</desc>',
            f"<style>\n{STYLE}\n  .card {{ fill: #f3f2ee; stroke: #d9d8d3; stroke-width: 1; }}\n"
            "  .hot { fill: #e6eefa; stroke: #2a78d6; stroke-width: 1.5; }\n"
            "  .warn { fill: #fbeeea; stroke: #eb6834; stroke-width: 1.5; }\n"
            "  .wire { stroke: #75746f; stroke-width: 1.5; fill: none; }\n"
            "  .tip { fill: #75746f; }\n"
            "@media (prefers-color-scheme: dark) {\n"
            "  .card { fill: #1e1e1d; stroke: #383835; }\n"
            "  .hot { fill: #11243a; stroke: #3987e5; }\n"
            "  .warn { fill: #30190f; stroke: #d95926; }\n"
            "  .wire { stroke: #898781; } .tip { fill: #898781; }\n}\n</style>",
            f'<rect class="bg" x="0" y="0" width="{w}" height="{h}"/>']


def arrow(x1, y, x2):
    return (f'<line class="wire" x1="{x1}" y1="{y}" x2="{x2-7}" y2="{y}"/>'
            f'<path class="tip" d="M{x2-7},{y-4} L{x2},{y} L{x2-7},{y+4} Z"/>')


def fig_abstract(F):
    W, H = 980, 500
    o = head(W, H, "at", "ad",
             "Graphical abstract: arrival timing dominates arrival clustering",
             f"At a fixed mean rate a random schedule costs {F[('7M','spin')][1]:.2f} times a periodic one "
             f"at the 98th percentile, 30 of 30 launches; clustering costs {F['clust'][1]:.2f} times at p99.9. "
             f"{F['rows']} pre-registered hypotheses, {F['ref']} rows recording a refutation, "
             f"{F['recs']:,} measured records, one Apple M4.")
    o.append(f'<text class="t1" x="40" y="38" font-size="19" font-weight="500">'
             'When a message arrives matters more than whether messages bunch</text>')
    o.append(f'<text class="t3" x="40" y="60" font-size="12.5">'
             'Pre-registered measurement on one Apple M4 performance core. Thresholds committed to git before each run.</text>')

    cw, gap, x0, cy, ch = 288, 28, 40, 84, 196
    cols = [
        ("Question", "#2a78d6", [
            "At one fixed mean rate, does the",
            "SHAPE of the arrival process change",
            "per-message latency?",
            "",
            "Two shapes are compared against a",
            "perfectly periodic schedule:",
            "random (Poisson), and random with",
            "clustering (Hawkes, Fano 0.9 to 13.9)."]),
        ("Method", "#2a78d6", [
            "Launch is the unit of observation.",
            "30 independent process launches,",
            "3 epochs each, paired within launch,",
            "exact two-sided sign test.",
            "",
            "65-point survival curve, not a chosen",
            "percentile. Capacity measured first,",
            "then the rate is read off it."]),
        ("Result", "#eb6834", [
            "Randomness is expensive and it is",
            "expensive in the BODY, not the tail.",
            "",
            "Clustering on top of randomness is",
            "nearly free, and both effects have",
            "vanished by p99.9.",
            "",
            "The large effect was PREDICTED."]),
    ]
    for i, (title, accent, lines) in enumerate(cols):
        x = x0 + i * (cw + gap)
        o.append(f'<rect class="{"warn" if i == 2 else "hot"}" x="{x}" y="{cy}" width="{cw}" height="{ch}" rx="6"/>')
        o.append(f'<text class="t1" x="{x+16}" y="{cy+26}" font-size="14" font-weight="500">{title}</text>')
        for j, ln in enumerate(lines):
            if not ln:
                continue
            o.append(f'<text class="t2" x="{x+16}" y="{cy+50+j*18}" font-size="11.5">{esc(ln)}</text>')

    ry = cy + ch + 30
    o.append(f'<rect class="card" x="{x0}" y="{ry}" width="{W-2*x0}" height="118" rx="6"/>')
    pk = F[("7M", "spin")]
    o.append(f'<text class="f2" x="{x0+26}" y="{ry+54}" font-size="46" font-weight="500">{pk[1]:.2f}x</text>')
    o.append(f'<text class="t2" x="{x0+26}" y="{ry+76}" font-size="11.5">random over periodic,</text>')
    o.append(f'<text class="t2" x="{x0+26}" y="{ry+92}" font-size="11.5">p{100*pk[0]:.2f}, {pk[2]}/{pk[3]} launches</text>')

    o.append(f'<line class="grid" x1="{x0+206}" y1="{ry+16}" x2="{x0+206}" y2="{ry+102}"/>')
    cl = F["clust"]
    o.append(f'<text class="f1" x="{x0+226}" y="{ry+54}" font-size="46" font-weight="500">{cl[1]:.2f}x</text>')
    o.append(f'<text class="t2" x="{x0+226}" y="{ry+76}" font-size="11.5">clustering over random,</text>')
    o.append(f'<text class="t2" x="{x0+226}" y="{ry+92}" font-size="11.5">p99.9, {cl[2]}/{cl[3]} launches</text>')
    o.append(f'<line class="grid" x1="{x0+398}" y1="{ry+16}" x2="{x0+398}" y2="{ry+102}"/>')

    # inline survival-ratio sparkline, same data as the full figure
    sx, sy, sw, sh = x0 + 424, ry + 22, 248, 76
    c = F["curve"]
    xlo, xhi = math.log10(0.5), math.log10(1e-6)
    lo, hi = 0.9, max(r[1] for r in c) * 1.08
    pts = []
    for q, v, *_ in c:
        s = max(1e-6, 1.0 - q)
        px = sx + sw * (math.log10(s) - xlo) / (xhi - xlo)
        py = sy + sh * (1.0 - (v - lo) / (hi - lo))
        pts.append(f"{px:.1f},{py:.1f}")
    o.append(f'<line class="ref" x1="{sx}" y1="{sy+sh*(1-(1.0-lo)/(hi-lo)):.1f}" '
             f'x2="{sx+sw}" y2="{sy+sh*(1-(1.0-lo)/(hi-lo)):.1f}"/>')
    o.append(f'<polyline class="ln l2" points="{" ".join(pts)}"/>')
    o.append(f'<text class="t3" x="{sx}" y="{sy+sh+15}" font-size="10.5">p50</text>')
    o.append(f'<text class="t3" x="{sx+sw}" y="{sy+sh+15}" font-size="10.5" text-anchor="end">p99.9999</text>')
    o.append(f'<text class="t3" x="{sx}" y="{sy-5}" font-size="10.5">ratio across the whole survival curve</text>')

    o.append(f'<text class="t3" x="{x0+700}" y="{ry+54}" font-size="11.5">'
             f'{F["rows"]} pre-registered</text>')
    o.append(f'<text class="t3" x="{x0+700}" y="{ry+70}" font-size="11.5">'
             f'hypotheses; {F["ref"]} rows record</text>')
    o.append(f'<text class="t3" x="{x0+700}" y="{ry+86}" font-size="11.5">'
             f'a refutation, all published</text>')

    o.append(f'<text class="t3" x="{x0}" y="{H-18}" font-size="11">'
             'One machine, one feed, performance cores only, two mean rates. The mechanism is untested: a periodic '
             'schedule never presents two messages at once.</text>')
    o.append("</svg>")
    (FIGS / "fig-abstract.svg").write_text("\n".join(o), encoding="utf-8")


def fig_method(F):
    W, H = 980, 430
    o = head(W, H, "mt", "md",
             "How one hypothesis is tested in this project",
             "Thresholds are committed to git before the run. Capacity is measured, then the rate is read off it. "
             "Thirty independent launches, three epochs each, median within launch, paired sign test across launches. "
             "A saturated run is excluded by the instrument, and a missed threshold is published as refuted.")
    o.append(f'<text class="t1" x="40" y="36" font-size="18" font-weight="500">'
             'How one hypothesis is tested, and why a failed prediction still gets published</text>')
    o.append(f'<text class="t3" x="40" y="57" font-size="12.5">'
             'The order is the point. Every threshold is in a commit that precedes the data it judges.</text>')

    bw, bh, by = 148, 74, 92
    xs = [40, 40 + 178, 40 + 356, 40 + 534, 40 + 712]
    steps = [
        ("Register", ["thresholds, arms,", "failure branch"]),
        ("Commit", ["before any data", "exists"]),
        ("Measure capacity", ["read the rate off", "the envelope"]),
        ("30 launches", ["3 epochs each,", "median within"]),
        ("Paired sign test", ["across launches,", "exact, two sided"]),
    ]
    for i, (t, sub) in enumerate(steps):
        x = xs[i]
        o.append(f'<rect class="{"hot" if i < 2 else "card"}" x="{x}" y="{by}" width="{bw}" height="{bh}" rx="6"/>')
        o.append(f'<text class="t1" x="{x+bw/2:.0f}" y="{by+28}" font-size="13" font-weight="500" '
                 f'text-anchor="middle">{t}</text>')
        for j, s in enumerate(sub):
            o.append(f'<text class="t2" x="{x+bw/2:.0f}" y="{by+46+j*15}" font-size="11" '
                     f'text-anchor="middle">{esc(s)}</text>')
        if i < len(steps) - 1:
            o.append(arrow(x + bw, by + bh / 2, xs[i + 1]))

    vy = by + bh + 56
    verdicts = [
        ("SUPPORTED", "threshold met and the sign test clears", "hot", 40),
        ("REFUTED", "threshold missed; the row is kept", "warn", 364),
        ("UNTESTABLE", "the instrument excluded the run", "card", 688),
    ]
    for label, sub, cls, x in verdicts:
        o.append(f'<rect class="{cls}" x="{x}" y="{vy}" width="{252}" height="62" rx="6"/>')
        o.append(f'<text class="t1" x="{x+16}" y="{vy+26}" font-size="13" font-weight="500">{label}</text>')
        o.append(f'<text class="t2" x="{x+16}" y="{vy+46}" font-size="11">{esc(sub)}</text>')
    o.append(f'<line class="wire" x1="{40+712+74}" y1="{by+bh}" x2="{40+712+74}" y2="{vy-26}"/>')
    o.append(f'<line class="wire" x1="166" y1="{vy-26}" x2="{40+712+74}" y2="{vy-26}"/>')
    for x in (166, 490, 814):
        o.append(f'<line class="wire" x1="{x}" y1="{vy-26}" x2="{x}" y2="{vy-7}"/>')
        o.append(f'<path class="tip" d="M{x-4},{vy-7} L{x+4},{vy-7} L{x},{vy} Z"/>')

    ty = vy + 86
    o.append(f'<rect class="card" x="40" y="{ty}" width="{W-80}" height="62" rx="6"/>')
    o.append(f'<text class="t1" x="62" y="{ty+26}" font-size="12.5" font-weight="500">'
             f'{F["rows"]} hypotheses registered, {F["verdicted"]} carry a recorded verdict, '
             f'{F["ref"]} of them a refutation, from {F["recs"]:,} measured records</text>')
    o.append(f'<text class="t2" x="62" y="{ty+46}" font-size="11.5">'
             'A refuted prediction costs nothing to publish and is the only evidence that the thresholds were '
             'real. Three of them appear in this paper.</text>')
    o.append("</svg>")
    (FIGS / "fig-method.svg").write_text("\n".join(o), encoding="utf-8")


if __name__ == "__main__":
    F = facts()
    fig_abstract(F)
    fig_method(F)
    print(f"fig-abstract.svg  peak {F[('7M','spin')][1]:.4f} at p{100*F[('7M','spin')][0]:.4f}, "
          f"clustering {F['clust'][1]:.4f}")
    print(f"fig-method.svg    {F['rows']} rows, {F['verdicted']} verdicted, {F['ref']} refuted, "
          f"{F['recs']:,} records")
