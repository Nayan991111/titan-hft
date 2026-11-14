#!/usr/bin/env python3
"""Summarise the output of titan-pingpong ("pp"), titan-stream ("st"),
titan-replay ("rp"), titan-t2t ("t2t"), titan-pipeline ("pl"),
titan-fanout ("fo"), titan-agg ("agg"), titan-wire ("wire") and titan-arb ("arb").

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

RING_KEY = ("impl", "order", "pad", "cached", "wait", "qos")
KEY_FIELDS = {"pp": RING_KEY, "st": RING_KEY, "rp": ("impl", "mix"), "t2t": ("stage", "feed", "build"),
              "pl": ("load", "wait", "path", "feed"), "pl_gap": ("load", "wait", "path", "feed", "gap"),
              "pl_split": ("load", "wait", "path", "feed", "part"), "fo": ("work", "design", "readers", "feed"),
              "agg": ("design", "strategies"), "wire": ("stage", "feed"), "arb": ("cond", "feed")}
METRIC = {"pp": "batch_rtt_ns", "st": "ns_per_msg", "rp": "ns_per_msg", "t2t": "ns_per_msg", "pl": "p99_ns", "pl_gap": "p50_ns", "pl_split": "p99_ns", "fo": "msg_cycles", "agg": "msg_cycles", "wire": "msg_cycles", "arb": "msg_cycles"}
EXTRA = {
    "pp": ("rtt_cycles", "rtt_instructions", "rtt_l1d_miss", "core_ghz", "invol_switches"),
    "st": ("msg_cycles", "msg_instructions", "msg_l1d_miss", "core_ghz", "invol_switches"),
    "rp": ("msg_cycles", "msg_instructions", "msg_l1d_miss", "msg_br_mispred", "core_ghz", "allocations"),
    "t2t": ("msg_cycles", "msg_instructions", "msg_l1d_miss", "msg_br_mispred", "ipc", "core_ghz", "batch_p99_ns", "intent_rate", "allocations"),
    "pl_split": ("p50_ns", "p90_ns", "p999_ns", "max_ns", "warm_units_per_msg", "saturated"),
    "pl_gap": ("messages", "p90_ns", "p99_ns", "p999_ns", "max_ns", "saturated"),
    "fo": ("writer_ns_per_msg", "msg_instructions", "quote_rate", "full_waits", "conflated_frac", "stale_p50_ns", "stale_p99_ns", "worst_reader_p99_ns", "core_ghz", "invol_switches"),
    "agg": ("ns_per_order", "msg_instructions", "msg_l1d_miss", "cas_retries_s0", "cas_retries_per_order", "min_other_orders", "core_ghz", "invol_switches"),
    "wire": ("ns_per_update", "msg_instructions", "msg_l1d_miss", "msg_br_mispred", "allocations_per_update", "core_ghz"),
    "arb": ("ns_per_msg", "msg_instructions", "msg_l1d_miss", "msg_br_mispred", "allocations_per_msg", "core_ghz"),
    "pl": ("p50_ns", "p999_ns", "p9999_ns", "max_ns", "achieved_mps", "saturated", "feed_late_over_1us", "invol_switches", "allocations"),
}
PAIR = re.compile(r"(\w+)=(\S+)")
# agg epochs excluded as starved, per (design, strategies)
EXCLUDED = defaultdict(int)


def load(paths):
    groups = defaultdict(list)
    for path in paths:
        with open(path) as fh:
            for line in fh:
                kind = line.split(" ", 1)[0]
                if kind not in METRIC:
                    continue
                rec = dict(PAIR.findall(line))
                if kind == "pl_gap" and rec.get("messages") == "0":
                    continue
                if kind == "pl" and rec.get("udp_dropped", "0") != "0":
                    continue  # gap-detection check runs, not measurements
                if kind == "agg" and rec.get("starved") == "1":
                    # another strategy completed under ops/100 orders: excluded, as pre-registered
                    EXCLUDED[(rec.get("design", "na"), rec.get("strategies", "na"))] += 1
                    continue
                if kind in ("pp", "st"):
                    rec.setdefault("qos", "ui")
                key = (kind,) + tuple(rec.get(f, "na") for f in KEY_FIELDS[kind])
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
    for kind in ("pp", "st", "rp", "t2t", "pl", "pl_gap", "pl_split", "fo", "agg", "wire", "arb"):
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
            label = " ".join(f"{f}={v}" for f, v in zip(KEY_FIELDS[kind], k[1:]))
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
    ("rp", "L3 book vs reference book (default mix)", ("l3", "default"), ("ref", "default")),
    ("rp", "L3 book vs reference book (add-heavy mix)", ("l3", "add_heavy"), ("ref", "add_heavy")),
    ("rp", "L3 book vs reference book (replace-heavy mix)", ("l3", "replace_heavy"), ("ref", "replace_heavy")),
    ("rp", "decode only vs decode + L3 book (default mix)", ("decode", "default"), ("l3", "default")),
]


def contrasts(groups):
    print("\n## contrasts: A vs B on the primary metric (lower is better)")
    extra = []
    for mix in sorted({k[2] for k in groups if k[0] == "rp"} - {"default", "add_heavy", "replace_heavy"}):
        extra.append(("rp", f"L3 book vs reference book ({mix})", ("l3", mix), ("ref", mix)))
        extra.append(("rp", f"decode only vs decode + L3 book ({mix})", ("decode", mix), ("l3", mix)))
    for feed, b in sorted({(k[2], k[3]) for k in groups if k[0] == "t2t"}):
        tag = feed if b == "na" else f"{feed}, build {b}"
        extra.append(("t2t", f"full vs direct, ring cost ({tag})", ("full", feed, b), ("direct", feed, b)))
        extra.append(("t2t", f"book vs full, ablation ({tag})", ("book", feed, b), ("full", feed, b)))
        extra.append(("t2t", f"decode vs book ({tag})", ("decode", feed, b), ("book", feed, b)))
        extra.append(("t2t", f"risk vs full, pre-trade checks ({tag})", ("risk", feed, b), ("full", feed, b)))
        extra.append(("t2t", f"checks vs replay, checks alone per order ({tag})", ("checks", feed, b), ("replay", feed, b)))
        extra.append(("t2t", f"fix vs ouch, order encoding ({tag})", ("fix", feed, b), ("ouch", feed, b)))
        extra.append(("t2t", f"ouch vs replay, binary encoding per order ({tag})", ("ouch", feed, b), ("replay", feed, b)))
        extra.append(("t2t", f"fixfull vs fixsel, FIX parse ({tag})", ("fixfull", feed, b), ("fixsel", feed, b)))
        extra.append(("t2t", f"fixfull vs fixneon, NEON delimiters ({tag})", ("fixfull", feed, b), ("fixneon", feed, b)))
        extra.append(("t2t", f"auditbin vs checks, binary audit record ({tag})", ("auditbin", feed, b), ("checks", feed, b)))
        extra.append(("t2t", f"audittext vs checks, text audit record ({tag})", ("audittext", feed, b), ("checks", feed, b)))
        extra.append(("t2t", f"vpin vs vpinfloor, VPIN update ({tag})", ("vpin", feed, b), ("vpinfloor", feed, b)))
        if b != "na" and b != "base":
            for stage in ("decode", "book", "full"):
                extra.append(("t2t", f"build {b} vs base, {stage} ({feed})", (stage, feed, b), (stage, feed, "base")))
    if any(k[0] == "agg" for k in groups):
        def ag(d, n):
            return (d, str(n))
        extra.append(("agg", "faa1 vs local1, one uncontended atomic add", ag("faa", 1), ag("local", 1)))
        extra.append(("agg", "faa3 vs faa1, contention on one shared total", ag("faa", 3), ag("faa", 1)))
        extra.append(("agg", "cas3 vs faa3, exact limit vs two-step check", ag("cas", 3), ag("faa", 3)))
        extra.append(("agg", "shard3 vs faa3, per-strategy totals vs one shared total", ag("shard", 3), ag("faa", 3)))
        extra.append(("agg", "shard3 vs shard1, cost of the other strategies writing their totals", ag("shard", 3), ag("shard", 1)))
    for feed in sorted({k[2] for k in groups if k[0] == "arb"}):
        extra.append(("arb", f"two0 vs one0, second lossless line ({feed})", ("two0", feed), ("one0", feed)))
        extra.append(("arb", f"two_a vs two0, loss on A filled by B ({feed})", ("two_a", feed), ("two0", feed)))
        extra.append(("arb", f"two_ab vs two0, loss on both lines ({feed})", ("two_ab", feed), ("two0", feed)))
    for feed in sorted({k[2] for k in groups if k[0] == "wire"}):
        extra.append(("wire", f"fix vs bin, decode ({feed})", ("fix", feed), ("bin", feed)))
        extra.append(("wire", f"json_serde vs json_fixed, JSON decode ({feed})", ("json_serde", feed), ("json_fixed", feed)))
        extra.append(("wire", f"json_fixed vs fix ({feed})", ("json_fixed", feed), ("fix", feed)))
    for work, feed in sorted({(k[1], k[4]) for k in groups if k[0] == "fo"}):
        def fo(d, n):
            return (work, d, str(n), feed)
        extra.append(("fo", f"copy3 vs copy1, writer ({work}, {feed})", fo("copy", 3), fo("copy", 1)))
        extra.append(("fo", f"snap1 vs snap0, writer ({work}, {feed})", fo("snap", 1), fo("snap", 0)))
        extra.append(("fo", f"snap3 vs snap1, writer ({work}, {feed})", fo("snap", 3), fo("snap", 1)))
        extra.append(("fo", f"snap3 vs copy3, writer ({work}, {feed})", fo("snap", 3), fo("copy", 3)))
        extra.append(("fo", f"snap0 vs none, writer ({work}, {feed})", fo("snap", 0), fo("none", 0)))
        for n in (1, 2, 3):
            extra.append(("fo", f"snap{n} vs copy{n}, staleness ({work}, {feed})", fo("snap", n), fo("copy", n)))
    for load, w, path, feed in sorted({k[1:5] for k in groups if k[0] == "pl_gap"}):
        for g in ("lt100ns", "gt100us"):
            if ("pl_gap", load, w, path, feed, g) not in groups:
                continue
            extra.append(("pl_gap", f"{g} vs 1us_10us, {load} {w} ({feed})", (load, w, path, feed, g), (load, w, path, feed, "1us_10us")))
    for load, path, feed in sorted({(k[1], k[3], k[4]) for k in groups if k[0] == "pl"}):
        for w in ("isb", "wfe"):
            extra.append(("pl", f"{w} vs spin, {load} {path} ({feed})", (load, w, path, feed), (load, "spin", path, feed)))
        extra.append(("pl", f"udppoll vs isb, {load} {path} ({feed})", (load, "udppoll", path, feed), (load, "isb", path, feed)))
        extra.append(("pl", f"udpblock vs udppoll, {load} {path} ({feed})", (load, "udpblock", path, feed), (load, "udppoll", path, feed)))
    for kind, name, ka, kb in CONTRASTS + extra:
        metrics = [METRIC[kind]] + {"t2t": ["msg_cycles"], "pl": ["p50_ns"], "pl_gap": ["p99_ns"]}.get(kind, [])
        if kind == "fo" and "staleness" in name:
            metrics = ["stale_p50_ns", "stale_p99_ns"]
        if kind == "pl" and any(r.get("saturated") == "1" for k in (ka, kb) for r in groups.get((kind,) + k, [])):
            continue  # percentiles of a saturated run grow with run length
        for metric in metrics:
            a = values(groups, (kind,) + ka, metric)
            b = values(groups, (kind,) + kb, metric)
            if not a or not b:
                continue
            ma, mb = median(a), median(b)
            u, p, how = mann_whitney(a, b)
            d = cliffs_delta(a, b)
            tag = name if metric == METRIC[kind] else f"{name} [{metric}]"
            print(f"{kind} {tag:56s} A={ma:9.2f} B={mb:9.2f} A/B={ma / mb:6.3f} nA={len(a)} nB={len(b)} U={u:.1f} p={p:.4f} ({how}) cliffs_delta={d:+.2f}")


def gap_epochs(groups):
    """H26 and H27 decision count: epochs in which a gap class has the higher median."""
    keys = sorted({k[1:5] for k in groups if k[0] == "pl_gap"})
    if not keys:
        return
    print("\n## gap classes: epochs where class A median > 1us_10us median")
    for load, w, path, feed in keys:
        def by_epoch(g):
            return {r["epoch"]: float(r["p50_ns"]) for r in groups.get(("pl_gap", load, w, path, feed, g), []) if int(r["messages"]) > 0}
        ref = by_epoch("1us_10us")
        for g in ("lt100ns", "gt100us"):
            a = by_epoch(g)
            common = sorted(set(a) & set(ref))
            wins = sum(a[e] > ref[e] for e in common)
            print(f"pl_gap {load} {w} {path} ({feed}) {g}: {wins} of {len(common)} epochs")


def fo_derived(groups, reps=2000, seed=1):
    """Writer cost per published update above the `none` floor, X(d, N).

    X = (median msg_cycles of the config - median msg_cycles of none) / q,
    q = published updates per unit (from the integer fields quotes and
    messages; 1 in work=replay, where msg_cycles is per update already).
    Bootstrap 95% intervals resample the epochs of each config and of none.
    A ratio X(a) / X(b) is printed only when the interval of X(b) lies above
    zero; otherwise it is undefined."""
    keys = sorted({(k[1], k[4]) for k in groups if k[0] == "fo"})
    if not keys:
        return
    print("\n## fo: writer cycles per published update above none, X [bootstrap 95% CI]")
    order = (("snap", 0), ("copy", 1), ("snap", 1), ("copy", 2), ("snap", 2), ("copy", 3), ("snap", 3))
    for work, feed in keys:
        nk = ("fo", work, "none", "0", feed)
        base = values(groups, nk, "msg_cycles")
        rates = [float(r["quotes"]) / float(r["messages"]) for r in groups.get(nk, []) if "quotes" in r and "messages" in r]
        if not base or not rates:
            continue
        q = median(rates)
        cyc = {dn: values(groups, ("fo", work, dn[0], str(dn[1]), feed), "msg_cycles") for dn in order}
        cyc = {dn: v for dn, v in cyc.items() if v}
        rng = random.Random(seed)
        boot = {dn: [] for dn in cyc}
        for _ in range(reps):
            b = median([rng.choice(base) for _ in base])
            for dn, v in cyc.items():
                boot[dn].append((median([rng.choice(v) for _ in v]) - b) / q)
        x = {dn: (median(v) - median(base)) / q for dn, v in cyc.items()}
        ci = {}
        for dn, bs in boot.items():
            bs = sorted(bs)
            ci[dn] = (bs[int(0.025 * reps)], bs[int(0.975 * reps) - 1])
        print(f"fo {work} {feed}: none={median(base):.3f} cycles per unit, updates per unit q={q:.7f}")
        for dn in cyc:
            print(f"fo {work} {feed}: X({dn[0]}{dn[1]}) = {x[dn]:.2f} [{ci[dn][0]:.2f}, {ci[dn][1]:.2f}]")
        for a, c in ((("copy", 3), ("copy", 1)), (("snap", 3), ("snap", 1)), (("snap", 1), ("snap", 0)), (("snap", 3), ("copy", 3))):
            if a not in x or c not in x:
                continue
            if ci[c][0] <= 0:
                print(f"fo {work} {feed}: X({a[0]}{a[1]}) / X({c[0]}{c[1]}) undefined (interval of X({c[0]}{c[1]}) reaches 0 or below)")
                continue
            rs = sorted(ba / bc for ba, bc in zip(boot[a], boot[c]))
            print(f"fo {work} {feed}: X({a[0]}{a[1]}) / X({c[0]}{c[1]}) = {x[a] / x[c]:.3f} [{rs[int(0.025 * reps)]:.3f}, {rs[int(0.975 * reps) - 1]:.3f}]")


def wire_derived(groups, reps=2000, seed=1):
    """Decode cost above the floor of the same format, X(stage), and the H50 ratios."""
    feeds = sorted({k[2] for k in groups if k[0] == "wire"})
    if not feeds:
        return
    floor_of = {"bin": "floor_bin", "fix": "floor_fix", "json_fixed": "floor_json", "json_serde": "floor_json"}
    print("\n## wire: decode cycles per update above the floor of its format, X [bootstrap 95% CI]")
    for feed in feeds:
        v = {st: values(groups, ("wire", st, feed), "msg_cycles") for st in list(floor_of) + sorted(set(floor_of.values()))}
        if any(not x for x in v.values()):
            continue
        rng = random.Random(seed)
        boot = {st: [] for st in floor_of}
        for _ in range(reps):
            fl = {f: median([rng.choice(v[f]) for _ in v[f]]) for f in set(floor_of.values())}
            for st, f in floor_of.items():
                boot[st].append(median([rng.choice(v[st]) for _ in v[st]]) - fl[f])
        x = {st: median(v[st]) - median(v[f]) for st, f in floor_of.items()}
        ci = {}
        for st, bs in boot.items():
            bs = sorted(bs)
            ci[st] = (bs[int(0.025 * reps)], bs[int(0.975 * reps) - 1])
            print(f"wire {feed}: X({st}) = {x[st]:.2f} [{ci[st][0]:.2f}, {ci[st][1]:.2f}]")
        for a, b in (("fix", "bin"), ("json_serde", "json_fixed"), ("json_fixed", "fix")):
            if ci[b][0] <= 0:
                print(f"wire {feed}: X({a}) / X({b}) undefined (interval of X({b}) reaches 0 or below)")
                continue
            rs = sorted(ba / bb for ba, bb in zip(boot[a], boot[b]))
            print(f"wire {feed}: X({a}) / X({b}) = {x[a] / x[b]:.3f} [{rs[int(0.025 * reps)]:.3f}, {rs[int(0.975 * reps) - 1]:.3f}]")


def arb_derived(groups, reps=2000, seed=1):
    """Arbitration cycles per delivered message above the floor, and the H51 ratios."""
    feeds = sorted({k[2] for k in groups if k[0] == "arb"})
    if not feeds:
        return
    conds = ("one0", "two0", "two_a", "two_ab")
    print("\n## arb: cycles per delivered message above the floor, X [bootstrap 95% CI]")
    for feed in feeds:
        v = {c: values(groups, ("arb", c, feed), "msg_cycles") for c in conds + ("floor",)}
        if any(not x for x in v.values()):
            continue
        rng = random.Random(seed)
        boot = {c: [] for c in conds}
        for _ in range(reps):
            fl = median([rng.choice(v["floor"]) for _ in v["floor"]])
            for c in conds:
                boot[c].append(median([rng.choice(v[c]) for _ in v[c]]) - fl)
        x = {c: median(v[c]) - median(v["floor"]) for c in conds}
        ci = {}
        for c in conds:
            bs = sorted(boot[c])
            ci[c] = (bs[int(0.025 * reps)], bs[int(0.975 * reps) - 1])
            print(f"arb {feed}: X({c}) = {x[c]:.2f} [{ci[c][0]:.2f}, {ci[c][1]:.2f}]")
        for a, b in (("two0", "one0"), ("two_ab", "two0")):
            if ci[b][0] <= 0:
                print(f"arb {feed}: X({a}) / X({b}) undefined (interval of X({b}) reaches 0 or below)")
                continue
            rs = sorted(ba / bb for ba, bb in zip(boot[a], boot[b]))
            print(f"arb {feed}: X({a}) / X({b}) = {x[a] / x[b]:.3f} [{rs[int(0.025 * reps)]:.3f}, {rs[int(0.975 * reps) - 1]:.3f}]")


def main(argv):
    if len(argv) < 2:
        print(__doc__)
        return 1
    groups = load(argv[1:])
    builds = {k[3] for k in groups if k[0] == "t2t"}
    if "na" in builds and len(builds) > 1:
        print("WARNING: t2t lines with and without build= are mixed; unlabelled lines are grouped as build=na")
    if not groups:
        print("no pp/st/rp/t2t/pl/fo/agg/wire/arb records found")
        return 1
    summary(groups)
    contrasts(groups)
    gap_epochs(groups)
    fo_derived(groups)
    wire_derived(groups)
    arb_derived(groups)
    if any(k[0] == "agg" for k in groups) or EXCLUDED:
        total = sum(EXCLUDED.values())
        print(f"\n## agg: epochs excluded as starved (another strategy under ops/100 orders): {total}")
        for (d, n), c in sorted(EXCLUDED.items()):
            print(f"agg excluded design={d} strategies={n} epochs={c}")
    return 0


if __name__ == "__main__":
    sys.exit(main(sys.argv))
