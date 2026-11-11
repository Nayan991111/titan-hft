#!/usr/bin/env python3
"""Draw the README figures as SVG from the committed result summaries.

    scripts/figures.py            writes docs/figures/*.svg

Every number drawn is parsed from a file in results/; nothing is typed in by
hand. Standard library only. The SVGs carry a light and a dark palette and
follow the reader's colour scheme.
"""

import re
import sys
from pathlib import Path

ROOT = Path(__file__).resolve().parent.parent
OUT = ROOT / "docs" / "figures"
T2T = ROOT / "results" / "2026-11-11_m4.txt"
PP = ROOT / "results" / "2026-11-11_m4_spsc.txt"

FONT = "ui-sans-serif, -apple-system, BlinkMacSystemFont, 'Segoe UI', Helvetica, Arial, sans-serif"
MONO = "ui-monospace, SFMono-Regular, Menlo, Consolas, monospace"

STYLE = """
<style>
  .bg { fill: #fcfcfb; } .frame { fill: none; stroke: #e4e3df; }
  .t1 { fill: #0b0b0b; } .t2 { fill: #52514e; } .t3 { fill: #75746f; }
  .grid { stroke: #e4e3df; stroke-width: 1; } .axis { stroke: #b9b8b2; stroke-width: 1; }
  .s1 { fill: #2a78d6; } .s2 { fill: #eb6834; }
  .box { fill: #f3f2ee; stroke: #d9d8d3; stroke-width: 1; }
  .hot { fill: #e6eefa; stroke: #2a78d6; stroke-width: 1.5; }
  .wire { stroke: #75746f; stroke-width: 1.5; fill: none; }
  .tip { fill: #75746f; }
  .bracket { stroke: #2a78d6; stroke-width: 1.5; fill: none; stroke-dasharray: 5 4; }
  @media (prefers-color-scheme: dark) {
    .bg { fill: #1a1a19; } .frame { stroke: #383835; }
    .t1 { fill: #f0efec; } .t2 { fill: #c3c2b7; } .t3 { fill: #9a998f; }
    .grid { stroke: #2e2e2c; } .axis { stroke: #57564f; }
    .s1 { fill: #3987e5; } .s2 { fill: #d95926; }
    .box { fill: #242422; stroke: #46453f; }
    .hot { fill: #1c2a3d; stroke: #3987e5; }
    .wire { stroke: #9a998f; } .tip { fill: #9a998f; }
    .bracket { stroke: #3987e5; }
  }
</style>"""


def esc(s):
    return s.replace("&", "&amp;").replace("<", "&lt;").replace(">", "&gt;")


def text(x, y, s, cls="t1", size=13, anchor="start", weight=400, family=FONT):
    return (f'<text x="{x:.1f}" y="{y:.1f}" class="{cls}" font-family="{family}" font-size="{size}" '
            f'font-weight="{weight}" text-anchor="{anchor}">{esc(s)}</text>')


def bar(x0, y, length, h, cls):
    """Horizontal bar, square at the baseline, 4 px rounded data end."""
    r = min(4.0, h / 2, length)
    x1 = x0 + length
    return (f'<path class="{cls}" d="M{x0:.1f},{y:.1f} H{x1 - r:.1f} Q{x1:.1f},{y:.1f} {x1:.1f},{y + r:.1f} '
            f'V{y + h - r:.1f} Q{x1:.1f},{y + h:.1f} {x1 - r:.1f},{y + h:.1f} H{x0:.1f} Z"/>')


def svg(w, h, body, title, desc):
    return (f'<svg xmlns="http://www.w3.org/2000/svg" width="{w}" height="{h}" viewBox="0 0 {w} {h}" '
            f'role="img" aria-labelledby="title desc">\n<title id="title">{esc(title)}</title>\n'
            f'<desc id="desc">{esc(desc)}</desc>{STYLE}\n'
            f'<rect class="bg" x="0.5" y="0.5" width="{w - 1}" height="{h - 1}" rx="10"/>\n'
            f'<rect class="frame" x="0.5" y="0.5" width="{w - 1}" height="{h - 1}" rx="10"/>\n'
            + "\n".join(body) + "\n</svg>\n")


def t2t_cycles():
    """{feed: {stage: (cycles, ns)}} from the PMU summary blocks."""
    rows = {}
    pat = re.compile(r"^stage=(\w+) feed=(\S+)\s+n=\d+\s+([\d.]+) \[.*?msg_cycles=([\d.]+)")
    for line in T2T.read_text().splitlines():
        m = pat.match(line)
        if m:
            stage, feed, ns, cyc = m.group(1), m.group(2), float(m.group(3)), float(m.group(4))
            rows.setdefault(feed, {})[stage] = (cyc, ns)
    return rows


def fig_stages():
    data = t2t_cycles()
    feeds = [("synthetic_default", "Synthetic feed (5M messages, 500 instruments)", "s1"),
             ("20190730_BX_ITCH_50", "NASDAQ BX, 2019-07-30 (first 20M messages)", "s2")]
    stages = [("ring", "ring", "pop + push only"),
              ("decode", "decode", "pop, ITCH decode"),
              ("book", "book", "+ L3 book update"),
              ("direct", "direct", "+ signal, rings removed"),
              ("full", "full", "+ rings: the trading path")]
    for f, _, _ in feeds:
        missing = [s for s, _, _ in stages if s not in data.get(f, {})]
        if missing:
            sys.exit(f"{T2T}: no PMU summary for {f} {missing}")
    W, left, right = 820, 238, 70
    top, band, bh, gap = 112, 46, 14, 2
    H = top + band * len(stages) + 84
    vmax = 200.0
    scale = (W - left - right) / vmax
    b = [text(28, 38, "Single-core tick-to-trade: CPU cycles per message by stage", size=17, weight=600),
         text(28, 60, "Apple M4 performance core, median of 5 epochs. Each stage adds work to the one above it;",
              "t2", 13),
         text(28, 78, "values are means over batches of 4,096 messages (lower is better).", "t2", 13)]
    lx = 28
    for f, label, cls in feeds:
        b.append(f'<rect class="{cls}" x="{lx}" y="92" width="12" height="12" rx="2"/>')
        b.append(text(lx + 18, 102.5, label, "t2", 12.5))
        lx += 18 + 7.0 * len(label) + 26
    for v in range(0, int(vmax) + 1, 50):
        x = left + v * scale
        b.append(f'<line class="grid" x1="{x:.1f}" y1="{top - 6}" x2="{x:.1f}" y2="{top + band * len(stages)}"/>')
        b.append(text(x, top + band * len(stages) + 18, f"{v}", "t3", 12, "middle"))
    b.append(text(left + (W - left - right) / 2, top + band * len(stages) + 40, "CPU cycles per message",
                  "t2", 12.5, "middle"))
    for i, (key, name, what) in enumerate(stages):
        y = top + i * band
        b.append(text(28, y + 15, name, "t1", 13.5, weight=600, family=MONO))
        b.append(text(28, y + 32, what, "t2", 12))
        for j, (f, _, cls) in enumerate(feeds):
            cyc, ns = data[f][key]
            by = y + 4 + j * (bh + gap)
            b.append(bar(left, by, cyc * scale, bh, cls))
            b.append(text(left + cyc * scale + 6, by + bh - 3, f"{cyc:.0f}  ({ns:.1f} ns)", "t2", 11.5))
    b.append(f'<line class="axis" x1="{left}" y1="{top - 6}" x2="{left}" y2="{top + band * len(stages)}"/>')
    b.append(text(W - 28, H - 14, "Source: results/2026-11-11_m4.txt (commit c3b850d, battery power, preliminary)",
                  "t3", 11, "end"))
    full = data["20190730_BX_ITCH_50"]["full"]
    desc = (f"Grouped bar chart. Full trading path: {data['synthetic_default']['full'][0]:.0f} cycles synthetic, "
            f"{full[0]:.0f} cycles ({full[1]:.1f} ns) on real NASDAQ BX data.")
    return svg(W, H, b, "titan-t2t stage costs", desc)


def fig_path():
    data = t2t_cycles()["20190730_BX_ITCH_50"]
    W, H = 980, 310
    y, h = 112, 64
    boxes = [("ITCH 5.0 feed", "pinned pages", "box", None),
             ("ingress ring", "descriptors", "hot", None),
             ("decode", "in place", "hot", None),
             ("L3 order book", "orders + levels", "hot", None),
             ("signal", "best bid and ask", "hot", None),
             ("egress ring", "order intents", "hot", None),
             ("gateway", "drains intents", "box", None)]
    n = len(boxes)
    x0, x1 = 24, W - 24
    bw = 112
    step = (x1 - x0 - bw) / (n - 1)
    b = [text(28, 38, "The measured trading path: one core, no allocation, no system call", size=17, weight=600),
         text(28, 60, "A feed-handler step fills the ingress ring and a gateway step drains the egress ring, both "
              "outside the timed region (dashed).", "t2", 13),
         '<defs><marker id="arrow" viewBox="0 0 10 10" refX="9" refY="5" markerWidth="7" markerHeight="7" '
         'orient="auto-start-reverse"><path class="tip" d="M0,0 L10,5 L0,10 z"/></marker></defs>']
    xs = [x0 + i * step for i in range(n)]
    for i, (name, sub, cls, _) in enumerate(boxes):
        x = xs[i]
        b.append(f'<rect class="{cls}" x="{x:.1f}" y="{y}" width="{bw}" height="{h}" rx="8"/>')
        b.append(text(x + bw / 2, y + 28, name, "t1", 14, "middle", 600))
        b.append(text(x + bw / 2, y + 48, sub, "t2", 12.5, "middle"))
        if i < n - 1:
            b.append(f'<line class="wire" x1="{x + bw + 2:.1f}" y1="{y + h / 2}" x2="{xs[i + 1] - 3:.1f}" '
                     f'y2="{y + h / 2}" marker-end="url(#arrow)"/>')
    bx0, bx1 = xs[1] - 10, xs[5] + bw + 10
    b.append(f'<rect class="bracket" x="{bx0:.1f}" y="{y - 22}" width="{bx1 - bx0:.1f}" height="{h + 44}" rx="12"/>')
    b.append(text(bx0 + 10, y - 30, "timed region, PMU cycles per batch", "t2", 12.5))
    fc, fns = data["full"]
    dc, dns = data["direct"]
    rows = [f"Real NASDAQ BX day, mean per message: {fc:.0f} cycles, {fns:.1f} ns for the full path; "
            f"{dc:.0f} cycles without the rings.",
            "This is software path cost on data already in memory. It excludes the NIC, the network stack and "
            "the kernel,",
            "so it is not a wire-to-wire latency. A hand-off to another core costs about 40.5 ns one way on its own "
            "(ping-pong RTT/2)."]
    for k, r in enumerate(rows):
        b.append(text(28, 234 + 21 * k, r, "t1" if k == 0 else "t2", 13.5, weight=600 if k == 0 else 400))
    desc = (f"Pipeline: feed, ingress ring, decode, L3 book, signal, egress ring, gateway. Full path "
            f"{fc:.0f} cycles per message on real data.")
    return svg(W, H, b, "titan-t2t trading path", desc)


def fig_pingpong():
    rows = []
    for line in PP.read_text().splitlines():
        p = line.split()
        if len(p) == 4 and p[0] == "pp":
            rows.append((p[1], float(p[2]), float(p[3])))
    if not rows:
        sys.exit(f"{PP}: no pp rows")
    W, left, right = 820, 250, 70
    top, band, bh = 92, 34, 18
    H = top + band * len(rows) + 80
    vmax = 160.0
    scale = (W - left - right) / vmax
    b = [text(28, 38, "Cross-core round trip by ordering, caching and wait strategy", size=17, weight=600),
         text(28, 60, "Two performance cores, SPSC ring, one message in flight; batch round trip, median of 3 "
              "epochs (lower is better).", "t2", 13)]
    for v in range(0, int(vmax) + 1, 40):
        x = left + v * scale
        b.append(f'<line class="grid" x1="{x:.1f}" y1="{top - 6}" x2="{x:.1f}" y2="{top + band * len(rows)}"/>')
        b.append(text(x, top + band * len(rows) + 18, f"{v}", "t3", 12, "middle"))
    b.append(text(left + (W - left - right) / 2, top + band * len(rows) + 40, "round trip, ns", "t2", 12.5, "middle"))
    for i, (case, rtt, one) in enumerate(rows):
        order, pad, cache, wait = case.split("/")
        y = top + i * band
        label = f"{order} · {cache} · {wait}"
        b.append(text(28, y + bh - 4, label, "t1", 12.5, family=MONO))
        b.append(bar(left, y, rtt * scale, bh, "s1"))
        b.append(text(left + rtt * scale + 6, y + bh - 4, f"{rtt:.1f}", "t2", 11.5))
    b.append(f'<line class="axis" x1="{left}" y1="{top - 6}" x2="{left}" y2="{top + band * len(rows)}"/>')
    b.append(text(W - 28, H - 14, "Source: results/2026-11-11_m4_spsc.txt (commit abc7890, 128-byte index "
                  "padding, battery power, preliminary)", "t3", 11, "end"))
    desc = f"Bar chart of round trips; fastest {rows[0][0]} at {rows[0][1]} ns."
    return svg(W, H, b, "Cross-core ping-pong", desc)


def main():
    OUT.mkdir(parents=True, exist_ok=True)
    for name, fn in (("t2t-path.svg", fig_path), ("t2t-stages.svg", fig_stages), ("pingpong.svg", fig_pingpong)):
        (OUT / name).write_text(fn())
        print(f"wrote {(OUT / name).relative_to(ROOT)}")


if __name__ == "__main__":
    main()
