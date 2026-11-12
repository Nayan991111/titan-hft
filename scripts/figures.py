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
PL = ROOT / "results" / "2026-11-12_m4.txt"

FONT = "ui-sans-serif, -apple-system, BlinkMacSystemFont, 'Segoe UI', Helvetica, Arial, sans-serif"
MONO = "ui-monospace, SFMono-Regular, Menlo, Consolas, monospace"

STYLE = """
<style>
  .bg { fill: #fcfcfb; } .frame { fill: none; stroke: #e4e3df; }
  .t1 { fill: #0b0b0b; } .t2 { fill: #52514e; } .t3 { fill: #75746f; }
  .grid { stroke: #e4e3df; stroke-width: 1; } .axis { stroke: #b9b8b2; stroke-width: 1; }
  .s1 { fill: #2a78d6; } .s2 { fill: #eb6834; } .s3 { fill: #1baf7a; }
  .l1 { stroke: #2a78d6; } .l2 { stroke: #eb6834; } .l3 { stroke: #1baf7a; }
  .ring { stroke: #fcfcfb; stroke-width: 2; }
  .box { fill: #f3f2ee; stroke: #d9d8d3; stroke-width: 1; }
  .hot { fill: #e6eefa; stroke: #2a78d6; stroke-width: 1.5; }
  .wire { stroke: #75746f; stroke-width: 1.5; fill: none; }
  .tip { fill: #75746f; }
  .bracket { stroke: #2a78d6; stroke-width: 1.5; fill: none; stroke-dasharray: 5 4; }
  .tile { fill: #f3f2ee; } .big { fill: #0b0b0b; } .accent { fill: #2a78d6; }
  .good { fill: #0ca30c; } .bad { fill: #d03b3b; } .warn { fill: #fab219; } .none { fill: #b9b8b2; }
  .chip { fill: #ffffff; stroke: #d9d8d3; stroke-width: 1; }
  @media (prefers-color-scheme: dark) {
    .bg { fill: #1a1a19; } .frame { stroke: #383835; }
    .t1 { fill: #f0efec; } .t2 { fill: #c3c2b7; } .t3 { fill: #9a998f; }
    .grid { stroke: #2e2e2c; } .axis { stroke: #57564f; }
    .s1 { fill: #3987e5; } .s2 { fill: #d95926; } .s3 { fill: #199e70; }
    .l1 { stroke: #3987e5; } .l2 { stroke: #d95926; } .l3 { stroke: #199e70; }
    .ring { stroke: #1a1a19; }
    .box { fill: #242422; stroke: #46453f; }
    .hot { fill: #1c2a3d; stroke: #3987e5; }
    .wire { stroke: #9a998f; } .tip { fill: #9a998f; }
    .bracket { stroke: #3987e5; }
    .tile { fill: #242422; } .big { fill: #f0efec; } .accent { fill: #3987e5; }
    .none { fill: #57564f; } .chip { fill: #1f1f1e; stroke: #46453f; }
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


EXPERIMENTS = ROOT / "docs" / "experiments.md"

ITCH_NAMES = {"A": "Add order", "F": "Add order, attributed", "E": "Order executed", "C": "Executed at price",
              "X": "Order cancel", "D": "Order delete", "U": "Order replace", "P": "Trade, non-cross",
              "N": "Retail price improvement", "H": "Trading action", "R": "Stock directory",
              "Y": "Reg SHO restriction", "L": "Market participant position", "S": "System event",
              "V": "MWCB decline level"}
BOOK_TYPES = set("AFECXDU")


def real_feed():
    """Message counts by type and verification counts for the real day."""
    types, msgs, mism, unk = {}, None, None, None
    for line in T2T.read_text().splitlines():
        if line.startswith("rp_feed ") and "types=" in line:
            for kv in line.split("types=")[1].split(","):
                k, v = kv.split(":")
                types[k] = int(v)
        if line.startswith("rp_verify "):
            f = dict(x.split("=", 1) for x in line.split()[1:])
            msgs, mism, unk = int(f["messages"]), int(f["top_of_book_mismatches"]), int(f["unknown_order_refs"])
    if not types or msgs is None:
        sys.exit(f"{T2T}: no rp_feed or rp_verify line")
    return types, msgs, mism, unk


def replay_pmu():
    """{impl: {metric: value}} from the PMU summary of the real-data replay."""
    out = {}
    pat = re.compile(r"^impl=(\w+) mix=20190730_BX_ITCH_50\s+n=\d+\s+([\d.]+) \[.*?msg_cycles=")
    for line in T2T.read_text().splitlines():
        m = pat.match(line)
        if m:
            f = dict(x.split("=", 1) for x in line.split() if "=" in x)
            out[m.group(1)] = {k: float(f[k]) for k in ("msg_cycles", "msg_instructions", "msg_l1d_miss")}
            out[m.group(1)]["ns"] = float(m.group(2))
    if not {"l3", "ref"} <= out.keys():
        sys.exit(f"{T2T}: no replay PMU summary")
    return out


def hypotheses():
    rows = []
    for line in EXPERIMENTS.read_text().splitlines():
        p = [c.strip() for c in line.split("|")]
        if len(p) > 4 and re.fullmatch(r"H\d+b?", p[1]):
            rows.append((p[1], p[3]))
    if not rows:
        sys.exit(f"{EXPERIMENTS}: no hypothesis table")
    return rows


def status_class(st):
    s = st.lower()
    if s.startswith(("supported", "confirmed")):
        return "good", "supported"
    if s.startswith("not supported"):
        return "bad", "not supported"
    if s.startswith("refuted"):
        return "bad", "refuted"
    if s.startswith("inconclusive"):
        return "warn", "inconclusive"
    return "none", "pending"


def fig_hero():
    types, msgs, mism, unk = real_feed()
    full = t2t_cycles()["20190730_BX_ITCH_50"]["full"]
    one_way = None
    for line in PP.read_text().splitlines():
        p = line.split()
        if len(p) == 4 and p[0] == "pp":
            one_way = float(p[3]) if one_way is None else min(one_way, float(p[3]))
    W, H = 980, 270
    b = [text(36, 58, "Titan", size=34, weight=700),
         text(36, 88, "What one Apple M4 core costs to read a market-data message, update a full order book "
              "and decide,", "t2", 15),
         text(36, 109, "measured with hardware counters and checked against a reference on a real NASDAQ day.",
              "t2", 15)]
    tiles = [(f"{msgs / 1e6:.1f}M", f"real NASDAQ messages, {mism} book mismatches"),
             (f"{full[0]:.0f}", f"cycles per message, full trading path ({full[1]:.1f} ns)"),
             (f"{one_way:.1f} ns", "one-way hand-off between two cores")]
    tw, gap, x, y = (W - 72 - 2 * 18) / 3, 18, 36, 136
    for big, small in tiles:
        b.append(f'<rect class="tile" x="{x:.1f}" y="{y}" width="{tw:.1f}" height="102" rx="10"/>')
        b.append(f'<rect class="accent" x="{x:.1f}" y="{y}" width="4" height="102" rx="2"/>')
        b.append(text(x + 22, y + 52, big, "big", 32, weight=700))
        b.append(text(x + 22, y + 80, small, "t2", 12.5))
        x += tw + gap
    desc = (f"{msgs} real messages verified with {mism} mismatches; full path {full[0]:.0f} cycles per message; "
            f"one-way hand-off {one_way} ns.")
    return svg(W, H, b, "Titan headline results", desc)


def fig_mix():
    types, msgs, _, _ = real_feed()
    total = sum(types.values())
    top = sorted(types.items(), key=lambda kv: -kv[1])[:8]
    rest = total - sum(v for _, v in top)
    rows = [(f"{k}  {ITCH_NAMES.get(k, k)}", v, k in BOOK_TYPES) for k, v in top]
    rows.append((f"{len(types) - len(top)} other types", rest, False))
    book = sum(v for k, v in types.items() if k in BOOK_TYPES)
    W, left, right = 820, 250, 110
    top_y, band, bh = 112, 30, 16
    H = top_y + band * len(rows) + 74
    vmax = max(v for _, v, _ in rows) * 1.05
    scale = (W - left - right) / vmax
    b = [text(28, 38, "What a real trading day looks like to the order book", size=17, weight=600),
         text(28, 60, f"NASDAQ BX, 2019-07-30: {total:,} ITCH 5.0 messages by type. "
              f"{book / total:.1%} of them change the order book.", "t2", 13),
         f'<rect class="s1" x="28" y="76" width="12" height="12" rx="2"/>',
         text(46, 86.5, "changes the book", "t2", 12.5),
         f'<rect class="none" x="190" y="76" width="12" height="12" rx="2"/>',
         text(208, 86.5, "other message types", "t2", 12.5)]
    for i, (label, v, is_book) in enumerate(rows):
        y = top_y + i * band
        b.append(text(28, y + bh - 3, label, "t1", 12.5, family=MONO if False else FONT))
        b.append(bar(left, y, max(v * scale, 2), bh, "s1" if is_book else "none"))
        b.append(text(left + max(v * scale, 2) + 6, y + bh - 3, f"{v / total:.1%}", "t2", 11.5))
    b.append(f'<line class="axis" x1="{left}" y1="{top_y - 6}" x2="{left}" y2="{top_y + band * len(rows)}"/>')
    b.append(text(W - 28, H - 14, "Source: results/2026-11-11_m4.txt (message counts from titan-replay)",
                  "t3", 11, "end"))
    desc = ", ".join(f"{k} {v}" for k, v in sorted(types.items(), key=lambda kv: -kv[1]))
    return svg(W, H, b, "ITCH message mix on a real day", desc)


def fig_replay():
    d = replay_pmu()
    panels = [("msg_cycles", "CPU cycles"), ("msg_instructions", "instructions"), ("msg_l1d_miss", "L1D load misses")]
    impls = [("l3", "Titan L3 book", "s1"), ("ref", "BTreeMap reference book", "s2")]
    W, H = 820, 330
    pw, gap, x0, base, ph = 236, 26, 28, 262, 110
    cyc_ratio = d["ref"]["msg_cycles"] / d["l3"]["msg_cycles"]
    b = [text(28, 38, f"Book update on real data: where the {cyc_ratio:.1f}x comes from", size=17, weight=600),
         text(28, 60, "Per message, decode plus book update, NASDAQ BX day, median of 3 epochs (lower is better).",
              "t2", 13)]
    lx = 28
    for _, label, cls in impls:
        b.append(f'<rect class="{cls}" x="{lx}" y="76" width="12" height="12" rx="2"/>')
        b.append(text(lx + 18, 86.5, label, "t2", 12.5))
        lx += 18 + 7.2 * len(label) + 26
    for i, (key, name) in enumerate(panels):
        x = x0 + i * (pw + gap)
        vmax = max(d[k][key] for k, _, _ in impls)
        b.append(text(x, 122, name, "t1", 13.5, weight=600))
        b.append(f'<line class="axis" x1="{x}" y1="{base}" x2="{x + pw}" y2="{base}"/>')
        for j, (k, _, cls) in enumerate(impls):
            v = d[k][key]
            h = v / vmax * ph
            bx = x + 30 + j * 92
            r = min(4.0, h)
            b.append(f'<path class="{cls}" d="M{bx},{base} V{base - h + r:.1f} Q{bx},{base - h:.1f} {bx + r},{base - h:.1f} '
                     f'H{bx + 56 - r} Q{bx + 56},{base - h:.1f} {bx + 56},{base - h + r:.1f} V{base} Z"/>')
            b.append(text(bx + 28, base - h - 7, f"{v:.2f}" if v < 10 else (f"{v:.1f}" if v < 100 else f"{v:.0f}"),
                          "t2", 12, "middle"))
        ratio = d["ref"][key] / d["l3"][key]
        b.append(text(x, base + 22, f"{ratio:.1f}x fewer with Titan", "t3", 12))
    b.append(text(W - 28, H - 14, "Source: results/2026-11-11_m4.txt (titan-replay with PMU, commit c3b850d)",
                  "t3", 11, "end"))
    desc = "; ".join(f"{name}: Titan {d['l3'][k]:.3g}, reference {d['ref'][k]:.3g}" for k, name in panels)
    return svg(W, H, b, "L3 book vs reference book", desc)


def fig_hypotheses():
    rows = hypotheses()
    counts = {}
    for _, st in rows:
        cls = status_class(st)[0]
        counts[cls] = counts.get(cls, 0) + 1
    cols = 11
    W = 820
    cw, ch, gx, gy, x0, y0 = 64, 54, 6, 8, 28, 104
    nrows = (len(rows) + cols - 1) // cols
    H = y0 + nrows * (ch + gy) + 34
    b = [text(28, 38, "Every prediction is committed before the run that tests it", size=17, weight=600),
         text(28, 60, f"{len(rows)} hypotheses so far: {counts.get('good', 0)} supported, "
              f"{counts.get('bad', 0)} refuted or not supported, {counts.get('warn', 0)} inconclusive, "
              f"{counts.get('none', 0)} pending. Failed predictions stay in the log.", "t2", 13)]
    lx = 28
    for cls, label in (("good", "supported"), ("bad", "refuted or not supported"), ("warn", "inconclusive"),
                       ("none", "pending")):
        b.append(f'<rect class="{cls}" x="{lx}" y="76" width="12" height="12" rx="2"/>')
        b.append(text(lx + 18, 86.5, label, "t2", 12.5))
        lx += 18 + 7.0 * len(label) + 24
    for i, (hid, st) in enumerate(rows):
        cls, label = status_class(st)
        x = x0 + (i % cols) * (cw + gx)
        y = y0 + (i // cols) * (ch + gy)
        b.append(f'<rect class="chip" x="{x}" y="{y}" width="{cw}" height="{ch}" rx="7"/>')
        b.append(f'<rect class="{cls}" x="{x}" y="{y}" width="{cw}" height="5" rx="2"/>')
        b.append(text(x + cw / 2, y + 27, hid, "t1", 14, "middle", 700))
        b.append(text(x + cw / 2, y + 44, label, "t2", 9.5 if len(label) > 12 else 10.5, "middle"))
    b.append(text(W - 28, H - 14, "Source: docs/experiments.md", "t3", 11, "end"))
    desc = "; ".join(f"{h} {status_class(s)[1]}" for h, s in rows)
    return svg(W, H, b, "Pre-registered hypotheses", desc)


def pipeline_rows():
    """{(load, wait, feed): {quantile: ns}} from the pipeline summaries."""
    out = {}
    pat = re.compile(r"^load=(\S+) wait=(\w+) path=full feed=(\S+)\s+n=\d+\s+([\d.]+) \[")
    for line in PL.read_text().splitlines():
        m = pat.match(line)
        if m:
            f = dict(x.split("=", 1) for x in line.split() if "=" in x)
            if f.get("saturated") != "0":
                continue
            out[(m.group(1), m.group(2), m.group(3))] = {
                "p50": float(f["p50_ns"]), "p99": float(m.group(4)), "p99.9": float(f["p999_ns"]),
                "p99.99": float(f["p9999_ns"]), "max": float(f["max_ns"])}
    return out


def fig_pipeline():
    import math
    rows = pipeline_rows()
    wait = "wfe"
    series = [(("rate_1000000", wait, "synthetic_default"), "synthetic, constant 1M msg/s", "1"),
              (("rate_10000000", wait, "synthetic_default"), "synthetic, constant 10M msg/s", "2"),
              (("speed_1", wait, "20190730_BX_ITCH_50"), "NASDAQ BX open, real timestamps", "3")]
    missing = [k for k, _, _ in series if k not in rows]
    if missing:
        sys.exit(f"{PL}: no pipeline summary for {missing}")
    qs = ["p50", "p99", "p99.9", "p99.99", "max"]
    W, H = 820, 470
    left, right, top, bottom = 92, 236, 96, 384
    lo, hi = math.log10(50), math.log10(100_000)
    xs = [left + i * (W - left - right) / (len(qs) - 1) for i in range(len(qs))]

    def y(v):
        return bottom - (math.log10(v) - lo) / (hi - lo) * (bottom - top)

    b = [text(28, 38, "Per-message latency, feed release to trading decision, two cores", size=17, weight=600),
         text(28, 60, f"Open-loop, measured from the schedule; wait = {wait}; median over epochs of each "
              "percentile (log scale, lower is better).", "t2", 13)]
    for v, lab in ((100, "100 ns"), (1_000, "1 us"), (10_000, "10 us"), (100_000, "100 us")):
        b.append(f'<line class="grid" x1="{left}" y1="{y(v):.1f}" x2="{W - right}" y2="{y(v):.1f}"/>')
        b.append(text(left - 10, y(v) + 4, lab, "t3", 12, "end"))
    for x, q in zip(xs, qs):
        b.append(text(x, bottom + 22, q, "t2", 12.5, "middle", family=MONO))
    b.append(f'<line class="axis" x1="{left}" y1="{bottom}" x2="{W - right}" y2="{bottom}"/>')
    ends = []
    for key, label, k in series:
        vals = [rows[key][q] for q in qs]
        pts = " ".join(f"{x:.1f},{y(v):.1f}" for x, v in zip(xs, vals))
        b.append(f'<polyline class="l{k}" fill="none" stroke-width="2" stroke-linejoin="round" points="{pts}"/>')
        for x, v in zip(xs, vals):
            b.append(f'<circle class="s{k} ring" cx="{x:.1f}" cy="{y(v):.1f}" r="5"/>')
        ends.append([y(vals[1]), label, k, vals[1]])
    # Direct labels at the p99 column, nudged apart.
    ends.sort()
    for i in range(1, len(ends)):
        ends[i][0] = max(ends[i][0], ends[i - 1][0] + 34)
    lx = W - right + 20
    for yy, label, k, p99 in ends:
        b.append(f'<circle class="s{k}" cx="{lx:.1f}" cy="{yy - 4:.1f}" r="5"/>')
        b.append(text(lx + 12, yy, label, "t1", 12))
        b.append(text(lx + 12, yy + 15, f"p99 {p99:,.0f} ns", "t2", 11.5))
    real = rows[series[2][0]]
    syn = rows[series[0][0]]
    b.append(text(28, H - 40, f"At the real open the p99 is {real['p99'] / syn['p99']:.1f}x the constant-rate p99, "
                  f"while the p99.99 is {real['p99.99'] / syn['p99.99']:.1f}x: bursts hurt the p99 to p99.9 band.",
                  "t2", 12.5))
    b.append(text(W - 28, H - 14, "Source: results/2026-11-12_m4.txt (commit b20bbf1, battery power, preliminary)",
                  "t3", 11, "end"))
    desc = (f"Percentile ladder for three schedules. p99: {syn['p99']:.0f} ns at 1M msg/s, "
            f"{rows[series[1][0]]['p99']:.0f} ns at 10M msg/s, {real['p99']:.0f} ns at the real BX open.")
    return svg(W, H, b, "Pipeline latency percentiles", desc)


def main():
    OUT.mkdir(parents=True, exist_ok=True)
    figs = (("hero.svg", fig_hero), ("t2t-path.svg", fig_path), ("t2t-stages.svg", fig_stages),
            ("pingpong.svg", fig_pingpong), ("itch-mix.svg", fig_mix), ("replay.svg", fig_replay),
            ("hypotheses.svg", fig_hypotheses), ("pipeline.svg", fig_pipeline))
    for name, fn in figs:
        (OUT / name).write_text(fn())
        print(f"wrote {(OUT / name).relative_to(ROOT)}")


if __name__ == "__main__":
    main()
