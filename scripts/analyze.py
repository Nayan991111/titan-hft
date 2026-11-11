#!/usr/bin/env python3
"""Summarise titan-pingpong ("pp") and titan-stream ("st") output.

Reads one or more result files, groups epochs by configuration and reports,
per configuration, the median of the per-epoch values with a bootstrap 95%
confidence interval. Predefined contrasts are tested with a two-sided
Mann-Whitney U test (exact when there are no ties, normal approximation with
tie correction otherwise) and Cliff's delta as effect size.

The unit of analysis is the epoch: each epoch contributes one value per
configuration, so the number of epochs, not the number of round trips,
determines statistical power. With 3 epochs per group the smallest attainable
two-sided p-value is 0.1; claims of significance need at least 8 per group.

Standard library only.  Usage: scripts/analyze.py FILE [FILE ...]
"""

import math
import random
import re
import sys
from collections import defaultdict
from functools import lru_cache

KEY_FIELDS = ("impl", "order", "pad", "cached", "wait", "qos")
METRIC = {"pp": "batch_rtt_ns", "st": "ns_per_msg"}
EXTRA = {
    "pp": ("rtt_cycles", "rtt_instructions", "rtt_l1d_miss", "core_ghz", "invol_switches"),
    "st": ("msg_cycles", "msg_instructions", "msg_l1d_miss", "core_ghz", "invol_switches"),
}
PAIR = re.compile(r"(\w+)=(\S+)")


def load(paths):
    groups = defaultdict(list)
    for path in paths:
        with open(path) as fh:
            for line in fh:
                kind = line.split(" ", 1)[0]
                if kind not in METRIC:
                    continue
                rec = dict(PAIR.findall(line))
                rec.setdefault("qos", "ui")
                key = (kind,) + tuple(rec.get(f, "na") for f in KEY_FIELDS)
                groups[key].append(rec)
    return groups


def median(xs):
    s = sorted(xs)
    n = len(s)
    return s[n // 2] if n % 2 else 0.5 * (s[n // 2 - 1] + s[n // 2])


def bootstrap_ci(xs, reps=10000, seed=1):
    if len(xs) < 2:
        return (float("nan"), float("nan"))
    rng = random.Random(seed)
    meds = sorted(median([rng.choice(xs) for _ in xs]) for _ in range(reps))
    return (meds[int(0.025 * reps)], meds[int(0.975 * reps) - 1])


@lru_cache(maxsize=None)
def u_count(n1, n2, u):
    """Number of arrangements of n1 and n2 items giving Mann-Whitney U == u."""
    if u < 0:
        return 0
    if n1 == 0 or n2 == 0:
        return 1 if u == 0 else 0
    return u_count(n1 - 1, n2, u - n2) + u_count(n1, n2 - 1, u)


def mann_whitney(a, b):
    n1, n2 = len(a), len(b)
    u = sum(1.0 if x > y else 0.5 if x == y else 0.0 for x in a for y in b)
    ties = len(set(a) & set(b)) > 0 or len(set(a + b)) < n1 + n2
    if not ties and n1 + n2 <= 40:
        total = math.comb(n1 + n2, n1)
        ui = int(round(u))
        lo = sum(u_count(n1, n2, k) for k in range(0, ui + 1)) / total
        hi = sum(u_count(n1, n2, k) for k in range(ui, n1 * n2 + 1)) / total
        return u, min(1.0, 2 * min(lo, hi)), "exact"
    allv = sorted(a + b)
    n = n1 + n2
    counts = defaultdict(int)
    for v in allv:
        counts[v] += 1
    tie_term = sum(t ** 3 - t for t in counts.values()) / (n * (n - 1))
    sd = math.sqrt(n1 * n2 / 12.0 * ((n + 1) - tie_term))
    if sd == 0:
        return u, 1.0, "normal"
    z = (u - n1 * n2 / 2.0) / sd
    return u, math.erfc(abs(z) / math.sqrt(2)), "normal"


def cliffs_delta(a, b):
    gt = sum(1 for x in a for y in b if x > y)
    lt = sum(1 for x in a for y in b if x < y)
    return (gt - lt) / (len(a) * len(b))


def values(groups, key, field):
    out = []
    for r in groups.get(key, []):
        if field in r:
            try:
                out.append(float(r[field]))
            except ValueError:
                pass
    return out


def summary(groups):
    for kind in ("pp", "st"):
        keys = sorted(k for k in groups if k[0] == kind)
        if not keys:
            continue
        metric = METRIC[kind]
        print(f"\n## {kind}: {metric}, median over epochs [bootstrap 95% CI]")
        rows = []
        for k in keys:
            xs = values(groups, k, metric)
            if not xs:
                continue
            lo, hi = bootstrap_ci(xs)
            extra = []
            for f in EXTRA[kind]:
                v = values(groups, k, f)
                if v:
                    extra.append(f"{f}={median(v):.3g}")
            rows.append((median(xs), k, len(xs), lo, hi, extra))
        for med, k, n, lo, hi, extra in sorted(rows):
            label = " ".join(f"{f}={v}" for f, v in zip(KEY_FIELDS, k[1:]))
            print(f"{label:62s} n={n:<3d} {med:9.2f} [{lo:8.2f}, {hi:8.2f}] {' '.join(extra)}")


CONTRASTS = [
    ("pp", "AcqRel vs SeqCst (pad128, cached, isb)", ("ring", "acqrel", "128", "1", "isb", "ui"), ("ring", "seqcst", "128", "1", "isb", "ui")),
    ("pp", "AcqRel vs Fenced (pad128, cached, isb)", ("ring", "acqrel", "128", "1", "isb", "ui"), ("ring", "fenced", "128", "1", "isb", "ui")),
    ("pp", "cached vs uncached (acqrel, pad128, isb)", ("ring", "acqrel", "128", "1", "isb", "ui"), ("ring", "acqrel", "128", "0", "isb", "ui")),
    ("pp", "pad128 vs pad64 (acqrel, uncached, isb)", ("ring", "acqrel", "128", "0", "isb", "ui"), ("ring", "acqrel", "64", "0", "isb", "ui")),
    ("pp", "wfe vs isb (acqrel, pad128, cached)", ("ring", "acqrel", "128", "1", "wfe", "ui"), ("ring", "acqrel", "128", "1", "isb", "ui")),
    ("pp", "spin vs isb (acqrel, pad128, cached)", ("ring", "acqrel", "128", "1", "spin", "ui"), ("ring", "acqrel", "128", "1", "isb", "ui")),
    ("pp", "P-core QoS vs E-core QoS (acqrel, pad128, cached, isb)", ("ring", "acqrel", "128", "1", "isb", "ui"), ("ring", "acqrel", "128", "1", "isb", "bg")),
    ("st", "AcqRel vs SeqCst (pad128, cached)", ("ring", "acqrel", "128", "1", "isb", "ui"), ("ring", "seqcst", "128", "1", "isb", "ui")),
    ("st", "AcqRel vs SeqCst (pad128, uncached)", ("ring", "acqrel", "128", "0", "isb", "ui"), ("ring", "seqcst", "128", "0", "isb", "ui")),
    ("st", "pad128 vs pad64 (acqrel, cached)", ("ring", "acqrel", "128", "1", "isb", "ui"), ("ring", "acqrel", "64", "1", "isb", "ui")),
    ("st", "pad128 vs pad64 (acqrel, uncached)", ("ring", "acqrel", "128", "0", "isb", "ui"), ("ring", "acqrel", "64", "0", "isb", "ui")),
    ("st", "AcqRel vs Fenced (pad128, cached)", ("ring", "acqrel", "128", "1", "isb", "ui"), ("ring", "fenced", "128", "1", "isb", "ui")),
    ("st", "wfe vs isb (acqrel, pad128, cached)", ("ring", "acqrel", "128", "1", "wfe", "ui"), ("ring", "acqrel", "128", "1", "isb", "ui")),
]


def contrasts(groups):
    print("\n## contrasts: A vs B on the primary metric (lower is better)")
    for kind, name, ka, kb in CONTRASTS:
        a = values(groups, (kind,) + ka, METRIC[kind])
        b = values(groups, (kind,) + kb, METRIC[kind])
        if not a or not b:
            continue
        ma, mb = median(a), median(b)
        u, p, how = mann_whitney(a, b)
        d = cliffs_delta(a, b)
        print(f"{kind} {name:56s} A={ma:9.2f} B={mb:9.2f} A/B={ma / mb:6.3f} nA={len(a)} nB={len(b)} U={u:.1f} p={p:.4f} ({how}) cliffs_delta={d:+.2f}")


def main(argv):
    if len(argv) < 2:
        print(__doc__)
        return 1
    groups = load(argv[1:])
    if not groups:
        print("no pp/st records found")
        return 1
    summary(groups)
    contrasts(groups)
    return 0


if __name__ == "__main__":
    sys.exit(main(sys.argv))
