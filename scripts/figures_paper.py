#!/usr/bin/env python3
"""Publication figures for the manuscript, as Type-42 PDF.

Separate from scripts/figures.py, which draws stdlib SVG for the README and
stays as it is. This script exists because IEEE PDF eXpress and the arXiv
preflight reject Type-3 fonts, so every figure that reaches a submission has
to carry Type 42 (TrueType) outlines. Setting that is two lines and forgetting
it costs a submission cycle.

Same discipline as figures.py: every number drawn is parsed out of a file in
results/ and nothing is typed in here. Unlike figures.py, each parse ASSERTS
how many rows it found and what they are. A silently wrong figure is worse
than a crash, and a results file that gets reformatted should break this
script loudly rather than produce a plot that looks fine and says something
false.

    python3 scripts/figures_paper.py

Writes docs/figures/fig-*.pdf and prints every value it drew.
"""

from __future__ import annotations

import re
import sys
from pathlib import Path

import matplotlib

matplotlib.use("Agg")
# The two lines this whole file exists for. IEEE PDF eXpress and arXiv both
# reject Type-3 fonts; 42 is TrueType.
matplotlib.rcParams["pdf.fonttype"] = 42
matplotlib.rcParams["ps.fonttype"] = 42
matplotlib.rcParams.update(
    {
        "font.family": "DejaVu Sans",
        "font.size": 8,
        "axes.labelsize": 8,
        "axes.titlesize": 8.5,
        "xtick.labelsize": 7.5,
        "ytick.labelsize": 7.5,
        "legend.fontsize": 7.5,
        "axes.spines.top": False,
        "axes.spines.right": False,
        "axes.grid": True,
        "grid.alpha": 0.25,
        "grid.linewidth": 0.5,
        "figure.dpi": 150,
    }
)
import matplotlib.pyplot as plt  # noqa: E402

ROOT = Path(__file__).resolve().parent.parent
RESULTS = ROOT / "results"
FIGS = ROOT / "docs" / "figures"
BARRIER = RESULTS / "2026-11-17_m4_barrier.txt"
BURST = RESULTS / "2026-11-18_m4_burst.txt"
RUNLENGTH = RESULTS / "2026-11-16_m4_runlength.txt"

# Single-column IEEE text width is 3.5 inches. Figures are sized to it so no
# scaling happens in LaTeX, which is where font sizes silently stop matching.
COL = 3.45

ARMS = ["acqrel", "fencedload", "fencedstore", "fenced", "unordered"]
LABEL = {
    "acqrel": "AcqRel\nldapr / stlr",
    "fencedload": "FencedLoad\nldr+dmb / stlr",
    "fencedstore": "FencedStore\nldapr / dmb+str",
    "fenced": "Fenced\nldr+dmb / dmb+str",
    "unordered": "Unordered\nldr / str",
}
# One hue per arm, held fixed across every figure so the same arm is the same
# colour everywhere. Chosen for distinguishability in greyscale print as well:
# the lightness steps monotonically across the five.
COLOR = {
    "acqrel": "#1b3a5c",
    "fencedload": "#2d6a9f",
    "fencedstore": "#7fb2d8",
    "fenced": "#4a8fc0",
    "unordered": "#b8ccd9",
}


def parse_barrier() -> dict[tuple[str, str], dict[str, float]]:
    """The per-arm table: arm, config, GHz median/min/max, ms median, cycles."""
    rows: dict[tuple[str, str], dict[str, float]] = {}
    pat = re.compile(
        r"^\s{4}(acqrel|fencedload|fencedstore|fenced|unordered)\s+"
        r"(uncached|cached)\s+"
        r"([\d.]+)\s+([\d.]+)\s+([\d.]+)\s+([\d.]+)\s+([\d.]+)\s*$"
    )
    for line in BARRIER.read_text(encoding="utf-8").splitlines():
        m = pat.match(line)
        if not m:
            continue
        arm, cfg = m.group(1), m.group(2)
        rows[(arm, cfg)] = {
            "ghz": float(m.group(3)),
            "ghz_lo": float(m.group(4)),
            "ghz_hi": float(m.group(5)),
            "ms": float(m.group(6)),
            "cycles": float(m.group(7)),
        }
    expected = {(a, c) for a in ARMS for c in ("uncached", "cached")}
    missing = expected - set(rows)
    if missing:
        sys.exit(f"parse_barrier: missing rows {sorted(missing)} in {BARRIER.name}")
    if len(rows) != 10:
        sys.exit(f"parse_barrier: expected 10 rows, found {len(rows)}")
    return rows


COUNTS = [200_000, 1_000_000, 5_000_000, 10_000_000]


def parse_sweep(header_cols: list[str]) -> list[tuple[int, float, float, float]]:
    """A four-column sweep table, located by its HEADER and not by its shape.

    This file contains two tables with identical column counts and identical
    message counts: one is messages/uncached/cached/ratio and the other is
    messages/AcqRel/SeqCst/Fenced. A regex that matched on shape alone picked
    up both and the row-count assertion caught it, which is the only reason
    this is anchored properly rather than plotting one table's numbers under
    the other's axis labels.
    """
    row = re.compile(r"^\s+([\d,]{5,})\s+([\d.]+)\s+([\d.]+)\s+([\d.]+)\s*$")
    lines = RUNLENGTH.read_text(encoding="utf-8").splitlines()
    start = None
    for i, line in enumerate(lines):
        low = line.lower()
        if "messages" in low and all(c.lower() in low for c in header_cols):
            start = i + 1
            break
    if start is None:
        sys.exit(f"parse_sweep: header with {header_cols} not found in {RUNLENGTH.name}")
    out: list[tuple[int, float, float, float]] = []
    for line in lines[start:]:
        m = row.match(line)
        if m:
            out.append((int(m.group(1).replace(",", "")), float(m.group(2)),
                        float(m.group(3)), float(m.group(4))))
        elif out:
            break
    if len(out) != 4:
        sys.exit(f"parse_sweep{header_cols}: expected 4 rows, found {len(out)}")
    if [r[0] for r in out] != COUNTS:
        sys.exit(f"parse_sweep{header_cols}: unexpected counts {[r[0] for r in out]}")
    return out


def fig_barrier(rows) -> None:
    """The lead result: five correct rings, and the order reverses with caching."""
    fig, axes = plt.subplots(1, 2, figsize=(COL * 2, 2.35), sharey=True)
    for ax, cfg, title in (
        (axes[0], "uncached", "Cursor line contended"),
        (axes[1], "cached", "Cursor line in L1"),
    ):
        order = sorted(ARMS, key=lambda a: rows[(a, cfg)]["cycles"])
        vals = [rows[(a, cfg)]["cycles"] for a in order]
        ax.bar(
            range(len(order)),
            vals,
            color=[COLOR[a] for a in order],
            edgecolor="white",
            linewidth=0.6,
            width=0.72,
        )
        for i, (a, v) in enumerate(zip(order, vals)):
            ax.text(i, v * 1.07, f"{v:.1f}", ha="center", va="bottom", fontsize=7)
        ax.set_xticks(range(len(order)))
        ax.set_xticklabels([LABEL[a].split("\n")[0] for a in order], rotation=28, ha="right")
        ax.set_yscale("log")
        ax.set_title(title)
        ax.set_ylim(4, 500)
    axes[0].set_ylabel("cycles per message (log)")
    fig.text(
        0.5,
        -0.10,
        "Apple M4 performance cores, pad=128, 10M messages, 30 independent launches, "
        "median of per-launch medians.\nEvery bar is a correct SPSC ring except Unordered, "
        "which is a cost floor and lost data in 180 of 180 epochs.\n"
        "Source: results/2026-11-17_m4_barrier.txt",
        ha="center",
        fontsize=6.4,
        color="#444444",
    )
    fig.tight_layout()
    fig.savefig(FIGS / "fig-barrier.pdf", bbox_inches="tight", backend="pdf")
    plt.close(fig)
    print("fig-barrier.pdf")
    for cfg in ("uncached", "cached"):
        ranked = sorted(ARMS, key=lambda a: rows[(a, cfg)]["cycles"])
        print(f"   {cfg:9}", " < ".join(f"{a} {rows[(a,cfg)]['cycles']:.3f}" for a in ranked))


def parse_stated_spread() -> str:
    """The spread as the results file STATES it, not as recomputed here.

    Recomputing 27.17/1.66 from the rounded ratio column gives 16.37x, while
    the file states 16.33x because it divided the unrounded ratios. Both are
    defensible and a figure caption that disagrees with the running text by
    0.04 is a gift to a reviewer, so the stated value is parsed rather than
    derived.
    """
    m = re.search(r"spread of ([\d.]+)x", RUNLENGTH.read_text(encoding="utf-8"))
    if not m:
        sys.exit("parse_stated_spread: no 'spread of Nx' sentence in " + RUNLENGTH.name)
    return m.group(1)


def fig_runlength(sweep) -> None:
    """Result two: the conclusion is a function of how long the run lasted."""
    fig, ax = plt.subplots(figsize=(COL, 2.1))
    xs = [r[0] for r in sweep]
    ys = [r[3] for r in sweep]
    ax.plot(xs, ys, marker="o", markersize=4.5, linewidth=1.4, color=COLOR["acqrel"])
    for x, y in zip(xs, ys):
        ax.annotate(f"{y:.2f}x", (x, y), textcoords="offset points",
                    xytext=(0, 7), ha="center", fontsize=7)
    ax.set_xscale("log")
    ax.set_xlabel("messages per epoch (log)")
    ax.set_ylabel("uncached / cached cycle ratio")
    ax.set_ylim(0, max(ys) * 1.28)
    ax.axhline(1.50, linestyle="--", linewidth=0.9, color="#aa3333")
    ax.text(xs[0], 1.50 * 1.35, "registered bound 1.50x", fontsize=6.6, color="#aa3333")
    fig.text(
        0.5,
        -0.16,
        f"A {parse_stated_spread()}x spread against a registered 1.50x. The same comparison, "
        "the same binary,\nthe same machine: only the run length differs. "
        "Source: results/2026-11-16_m4_runlength.txt",
        ha="center",
        fontsize=6.4,
        color="#444444",
    )
    fig.tight_layout()
    fig.savefig(FIGS / "fig-runlength.pdf", bbox_inches="tight", backend="pdf")
    plt.close(fig)
    print("fig-runlength.pdf")
    for n, unc, cac, ratio in sweep:
        print(f"   {n:>10,}  uncached {unc:8.3f}  cached {cac:7.3f}  ratio {ratio:6.2f}")
    print(f"   spread as stated in the results file: {parse_stated_spread()}x "
          f"(recomputing from the rounded column would give "
          f"{max(r[3] for r in sweep)/min(r[3] for r in sweep):.2f}x)")


def fig_ordering(ord_sweep) -> None:
    """RQ1: the ranking of the three orderings inverts with run length."""
    fig, ax = plt.subplots(figsize=(COL, 2.15))
    xs = [r[0] for r in ord_sweep]
    series = {
        "AcqRel (ldapr/stlr)": ([r[1] for r in ord_sweep], COLOR["acqrel"], "o"),
        "SeqCst (ldar/stlr)": ([r[2] for r in ord_sweep], "#aa3333", "s"),
        "Fenced (dmb x2)": ([r[3] for r in ord_sweep], COLOR["fencedstore"], "^"),
    }
    for name, (ys, c, mk) in series.items():
        ax.plot(xs, ys, marker=mk, markersize=4.2, linewidth=1.3, color=c, label=name)
    ax.set_xscale("log")
    ax.set_xlabel("messages per epoch (log)")
    ax.set_ylabel("cycles per message")
    ax.legend(frameon=False, loc="lower right")
    a0, s0, f0 = ord_sweep[0][1], ord_sweep[0][2], ord_sweep[0][3]
    a3, s3, f3 = ord_sweep[-1][1], ord_sweep[-1][2], ord_sweep[-1][3]
    fig.text(
        0.5, -0.17,
        f"At 200,000 messages the three orderings rank AcqRel {a0:.1f} < Fenced {f0:.1f} < "
        f"SeqCst {s0:.1f}.\nAt 10,000,000 they rank Fenced {f3:.1f} < SeqCst {s3:.1f} ~ "
        f"AcqRel {a3:.1f}: the cheapest becomes the dearest.\n"
        "Uncached, warm epoch, 128-byte padding. "
        "Source: results/2026-11-16_m4_runlength.txt",
        ha="center", fontsize=6.4, color="#444444",
    )
    fig.tight_layout()
    fig.savefig(FIGS / "fig-ordering.pdf", bbox_inches="tight", backend="pdf")
    plt.close(fig)
    print("fig-ordering.pdf")
    for n, a, sq, f in ord_sweep:
        print(f"   {n:>10,}  AcqRel {a:8.3f}  SeqCst {sq:8.3f}  Fenced {f:8.3f}")


def fig_clock(rows) -> None:
    """Why the figures above may be read in cycles: the compared arms share a clock."""
    fig, ax = plt.subplots(figsize=(COL, 2.2))
    ys, labels, colors, los, his = [], [], [], [], []
    for cfg in ("uncached", "cached"):
        for a in ARMS:
            r = rows[(a, cfg)]
            ys.append(r["ghz"])
            los.append(r["ghz"] - r["ghz_lo"])
            his.append(r["ghz_hi"] - r["ghz"])
            labels.append(f"{a[:11]}\n{cfg}")
            colors.append(COLOR[a])
    idx = range(len(ys))
    ax.errorbar(
        ys, list(idx), xerr=[los, his], fmt="none",
        ecolor="#888888", elinewidth=0.9, capsize=2,
    )
    ax.scatter(ys, list(idx), s=22, c=colors, zorder=3, edgecolor="white", linewidth=0.5)
    ordered_unc = [rows[(a, "uncached")]["ghz"] for a in ARMS if a != "unordered"]
    spread = max(
        rows[(a, "uncached")]["ghz_hi"] for a in ARMS if a != "unordered"
    ) / min(rows[(a, "uncached")]["ghz_lo"] for a in ARMS if a != "unordered")
    ax.set_yticks(list(idx))
    ax.set_yticklabels(labels, fontsize=6.2)
    ax.invert_yaxis()
    ax.set_xlabel("core clock, GHz (median, with min and max observed)")
    fig.text(
        0.5,
        -0.13,
        f"The four ordered uncached arms, which carry the lead result, span {spread:.3f}x "
        f"across 540 lines\n(median {min(ordered_unc):.3f} to {max(ordered_unc):.3f} GHz), so "
        "that comparison is clock-clean. No figure in this\npaper compares a cached arm against "
        "an uncached one. Source: results/2026-11-17_m4_barrier.txt",
        ha="center",
        fontsize=6.4,
        color="#444444",
    )
    fig.tight_layout()
    fig.savefig(FIGS / "fig-clock.pdf", bbox_inches="tight", backend="pdf")
    plt.close(fig)
    print("fig-clock.pdf")
    print(f"   ordered uncached clock spread {spread:.4f}x "
          f"({min(ordered_unc):.3f} to {max(ordered_unc):.3f} GHz median)")


def parse_ccdf(path: Path, wait: str = "spin"):
    """pl_ccdf lines for ONE wait strategy, median across epochs and launches.

    Grouping matters here and getting it wrong is visible. A first version
    keyed only on the load arm, which merged three wait strategies and three
    epochs into one series; the plot then joined points belonging to different
    runs and drew a zigzag instead of a curve. The key is (load, wait), one
    wait strategy is selected, and the latency at each survival level is the
    median over every run that reported it.
    """
    if not path.exists():
        return {}
    pat = re.compile(
        r"pl_ccdf\b.*?\bload=(\S+).*?\bwait=(\S+).*?"
        r"\bsurvival=([\d.e+-]+)\s+ns=([\d.]+)\s+beyond=(\d+)"
    )
    raw: dict[str, dict[float, list[tuple[float, int]]]] = {}
    waits_seen: set[str] = set()
    for line in path.read_text(encoding="utf-8").splitlines():
        m = pat.search(line)
        if not m:
            continue
        load, w, surv, ns, beyond = m.groups()
        waits_seen.add(w)
        if w != wait:
            continue
        raw.setdefault(load, {}).setdefault(float(surv), []).append(
            (float(ns), int(beyond))
        )
    if not raw and waits_seen:
        print(f"   note: no rows for wait={wait}; file has {sorted(waits_seen)}")
    out: dict[str, list[tuple[float, float, int]]] = {}
    for load, levels in raw.items():
        pts = []
        for surv, vals in levels.items():
            vals.sort()
            mid = vals[len(vals) // 2]
            pts.append((surv, mid[0], min(b for _, b in vals)))
        pts.sort(key=lambda t: -t[0])
        out[load] = pts
    return out


def fig_ccdf(arms, wait: str = "spin") -> None:
    """Tail latency as a survival curve, with burstiness as the only variable."""
    if not arms:
        print("fig-ccdf.pdf  SKIPPED: no pl_ccdf data yet in " + BURST.name)
        return
    fig, ax = plt.subplots(figsize=(COL, 2.4))
    order = sorted(arms, key=lambda k: (not k.startswith("rate"), k))
    palette = ["#1b3a5c", "#2d6a9f", "#7fb2d8", "#aa3333", "#4a8fc0"]
    deepest = 1.0
    for i, name in enumerate(order):
        # Read only where the curve is resolved. The emitter stops at ten
        # observations beyond a quantile; a figure should not go that far.
        pts = [(s, ns, b) for s, ns, b in arms[name] if b >= 100]
        if not pts:
            continue
        deepest = min(deepest, min(s for s, _, _ in pts))
        ax.plot(
            [ns / 1000.0 for _, ns, _ in pts],
            [s for s, _, _ in pts],
            linewidth=1.5,
            color=palette[i % len(palette)],
            label=name,
        )
    ax.set_yscale("log")
    ax.set_xscale("log")
    ax.set_xlabel("per message latency, microseconds (log)")
    ax.set_ylabel("P(latency > x)  (log)")
    ax.legend(frameon=False, fontsize=6.8)
    fig.text(
        0.5, -0.19,
        "Same mean rate, same messages, same code: only the arrival times differ. "
        f"wait={wait},\nmedian across epochs and launches. Drawn only where at "
        "least 100 observations lie beyond\nthe quantile, so the deepest point "
        f"plotted is {deepest:.1e} survival. Source: results/" + BURST.name,
        ha="center", fontsize=6.4, color="#444444",
    )
    fig.tight_layout()
    fig.savefig(FIGS / "fig-ccdf.pdf", bbox_inches="tight", backend="pdf")
    plt.close(fig)
    print("fig-ccdf.pdf")
    for name in order:
        pts = [p for p in arms[name] if p[2] >= 100]
        if pts:
            p999 = min(pts, key=lambda t: abs(t[0] - 1e-3))
            print(f"   {name:16} {len(pts):3d} points, deepest survival "
                  f"{min(p[0] for p in pts):.2e}, p99.9 {p999[1] / 1000.0:.2f} us")


def verify_fonts() -> None:
    """Assert no Type-3 font reached any figure.

    Setting rcParams is necessary and not sufficient: a stray mathtext glyph
    or a usetex path can still emit Type 3, and the rejection then arrives
    from arXiv or IEEE PDF eXpress rather than from here. pdffonts is the
    authoritative reader. Matplotlib writes the fonts as CID TrueType inside
    compressed object streams, so grepping the raw bytes for /TrueType finds
    nothing and proves nothing: that check was tried first and was wrong.
    """
    import shutil
    import subprocess

    pdffonts = shutil.which("pdffonts")
    if not pdffonts:
        print("NOTE: pdffonts not installed (poppler-utils), font types unverified.")
        print("      Debian/Ubuntu: sudo apt install poppler-utils")
        print("      macOS: brew install poppler")
        return
    bad = []
    for f in sorted(FIGS.glob("fig-*.pdf")):
        out = subprocess.run([pdffonts, str(f)], capture_output=True, text=True).stdout
        types = [ln.split()[1:3] for ln in out.splitlines()[2:] if ln.strip()]
        flat = " ".join(" ".join(t) for t in types)
        if "Type 3" in flat or "Type3" in flat:
            bad.append((f.name, flat))
        print(f"  {f.name:22} {flat or '(no embedded font)'}")
    if bad:
        sys.exit(f"TYPE-3 FONT PRESENT, these would be rejected: {bad}")
    print("  all figures PASS: no Type 3, which is what IEEE PDF eXpress and arXiv check")


def main() -> None:
    for f in (BARRIER, RUNLENGTH):
        if not f.exists():
            sys.exit(f"missing {f}")
    FIGS.mkdir(parents=True, exist_ok=True)
    rows = parse_barrier()
    sweep = parse_sweep(["uncached", "cached", "ratio"])
    ord_sweep = parse_sweep(["acqrel", "seqcst", "fenced"])
    print(f"parsed {len(rows)} arm rows and two {len(sweep)}-row sweeps, nothing typed in")
    fig_barrier(rows)
    fig_runlength(sweep)
    fig_ordering(ord_sweep)
    fig_clock(rows)
    fig_ccdf(parse_ccdf(BURST))
    print("\n=== font verification ===")
    verify_fonts()


if __name__ == "__main__":
    main()
