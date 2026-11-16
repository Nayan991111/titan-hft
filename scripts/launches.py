#!/usr/bin/env python3
"""Statistics for the independent-launch protocol.

Epochs inside one process share a code layout, an allocator state and a
thermal state, so they are not independent samples of anything that depends
on placement. The SPSC padding question depends on exactly that, which is
why three epochs in one process on battery left H3b and H14 unsettled. This
script reads a run where every line carries `launch=N` and treats the launch
as the unit of observation: it reduces each launch to one value per case,
then reports the median and a bootstrap interval across launches.

For the padding question it also forms the paired difference within each
launch. Two cases measured in the same process share whatever that process
got from the loader and the allocator, so pairing inside the launch removes
launch-to-launch variation instead of averaging over it. The sign test over
the paired differences needs no distributional assumption at all.

    python3 scripts/launches.py RAWFILE
"""

import sys
import random
from collections import defaultdict

RING_KEY = ("order", "pad", "cached", "wait")
PAIR_KEY = ("order", "cached", "wait")
# The cross-core harness adds the QoS class, which selects performance or
# efficiency cores, so it is part of what identifies a case.
PP_KEY = ("order", "pad", "cached", "wait", "qos")


def median(xs):
    s = sorted(xs)
    n = len(s)
    if n == 0:
        return float("nan")
    return s[n // 2] if n % 2 else (s[n // 2 - 1] + s[n // 2]) / 2


def comb(n, k):
    num, den = 1, 1
    for i in range(k):
        num *= n - i
        den *= i + 1
    return num // den


def sign_p(wins, n):
    """Two-sided exact binomial test against a fair coin."""
    if n == 0:
        return float("nan")
    k = max(wins, n - wins)
    tail = sum(comb(n, i) for i in range(k, n + 1)) / float(2 ** n)
    return min(1.0, 2 * tail)


def boot_ci(xs, reps=4000, seed=1):
    if len(xs) < 2:
        return (float("nan"), float("nan"))
    rng = random.Random(seed)
    ms = sorted(median([rng.choice(xs) for _ in xs]) for _ in range(reps))
    return (ms[int(0.025 * reps)], ms[int(0.975 * reps) - 1])


def parse(path):
    """kind -> launch -> case tuple -> field -> list of values."""
    out = defaultdict(lambda: defaultdict(lambda: defaultdict(lambda: defaultdict(list))))
    for line in open(path, encoding="utf-8", errors="replace"):
        parts = line.split()
        if not parts:
            continue
        kind = parts[0]
        d = {}
        for p in parts[1:]:
            if "=" in p:
                k, v = p.split("=", 1)
                d[k] = v
        if "launch" not in d:
            continue
        launch = d["launch"]
        if kind == "st":
            key = tuple(d.get(k, "") for k in RING_KEY)
        elif kind == "pp":
            key = tuple(d.get(k, "") for k in PP_KEY)
        elif kind == "pl":
            key = (d.get("load", ""), d.get("wait", ""), d.get("path", ""))
        else:
            continue
        for k, v in d.items():
            try:
                out[kind][launch][key][k].append(float(v))
            except ValueError:
                pass
    return out


def per_launch(data, kind, field):
    """case tuple -> list of one value per launch, launches sorted."""
    res = defaultdict(list)
    for launch in sorted(data[kind], key=lambda x: int(x)):
        for key, fields in data[kind][launch].items():
            if field in fields and fields[field]:
                res[key].append(median(fields[field]))
    return res


def report_padding(data, field):
    cases = per_launch(data, "st", field)
    if not cases:
        return
    launches = len({l for l in data["st"]})
    print(f"\n## streaming, {field}, one value per launch, {launches} launches")
    for key in sorted(cases):
        xs = cases[key]
        lo, hi = boot_ci(xs)
        order, pad, cached, wait = key
        print(f"  order={order} pad={pad} cached={cached} wait={wait}: median {median(xs):.3f} [{lo:.3f}, {hi:.3f}] over {len(xs)} launches")

    # Paired within launch: positive means 64-byte padding is cheaper.
    print(f"\n## paired within launch, {field}: pad128 minus pad64 (positive favours pad64)")
    pairs = defaultdict(list)
    for launch in sorted(data["st"], key=lambda x: int(x)):
        by_pair = defaultdict(dict)
        for key, fields in data["st"][launch].items():
            order, pad, cached, wait = key
            if field in fields and fields[field]:
                by_pair[(order, cached, wait)][pad] = median(fields[field])
        for pk, v in by_pair.items():
            if "64" in v and "128" in v:
                pairs[pk].append(v["128"] - v["64"])
    for pk in sorted(pairs):
        d = pairs[pk]
        wins = sum(1 for x in d if x > 0)
        lo, hi = boot_ci(d)
        order, cached, wait = pk
        print(
            f"  order={order} cached={cached} wait={wait}: median diff {median(d):+.3f} "
            f"[{lo:+.3f}, {hi:+.3f}], pad64 cheaper in {wins}/{len(d)} launches, sign p = {sign_p(wins, len(d)):.4f}"
        )


def report_pipeline(data):
    cases = per_launch(data, "pl", "p99_ns")
    if not cases:
        return
    launches = sorted(data["pl"], key=lambda x: int(x))
    print(f"\n## pipeline p99, one value per launch, {len(launches)} launches")
    for key in sorted(cases):
        xs = cases[key]
        lo, hi = boot_ci(xs)
        spread = max(xs) / min(xs) if min(xs) > 0 else float("nan")
        load, wait, path = key
        print(
            f"  load={load} wait={wait} path={path}: median {median(xs):.0f} ns [{lo:.0f}, {hi:.0f}], "
            f"launch min {min(xs):.0f} max {max(xs):.0f}, max/min {spread:.3f}, {len(xs)} launches"
        )
    # Ratio of the real-timestamp arm to each constant-rate arm, same wait.
    print("\n## real timestamps against constant rate, same wait, median over launches")
    byload = defaultdict(dict)
    for (load, wait, path), xs in cases.items():
        byload[(wait, path)][load] = median(xs)
    for (wait, path), v in sorted(byload.items()):
        real = next((m for ld, m in v.items() if ld.startswith("speed_")), None)
        for ld, m in sorted(v.items()):
            if ld.startswith("rate_") and real is not None and m > 0:
                print(f"  wait={wait} path={path}: speed vs {ld} = {real / m:.3f}x ({real:.0f} vs {m:.0f} ns)")


def pair_on(data, kind, keynames, field, vary, a, b):
    """Pair cases that differ only in `vary`, inside each launch.

    Two cases measured in one process share that process's code layout and
    allocator state, so taking the difference inside a launch removes
    launch-to-launch variation rather than averaging over it. Returns a map
    from the held-fixed attributes to the list of per-launch (a, b) pairs.
    """
    idx = keynames.index(vary)
    pairs = {}
    for launch in sorted(data[kind], key=lambda x: int(x)):
        bucket = {}
        for key, fields in data[kind][launch].items():
            if field not in fields or not fields[field]:
                continue
            rest = key[:idx] + key[idx + 1:]
            bucket.setdefault(rest, {})[key[idx]] = median(fields[field])
        for rest, v in bucket.items():
            if a in v and b in v:
                pairs.setdefault(rest, []).append((v[a], v[b]))
    return pairs


def report_pair(data, kind, keynames, field, vary, a, b, favours):
    """Sign test and ratio for every pairing of `a` against `b`."""
    pairs = pair_on(data, kind, keynames, field, vary, a, b)
    if not pairs:
        return
    held = [k for k in keynames if k != vary]
    print(f"\n## {kind}: {vary}={a} against {vary}={b} on {field}, paired inside each launch")
    print(f"   positive difference and ratio above 1 favour {favours}")
    for rest in sorted(pairs):
        d = pairs[rest]
        diffs = [x - y for x, y in d]          # a minus b
        ratios = [x / y for x, y in d if y > 0]
        wins = sum(1 for x in diffs if x > 0)
        lo, hi = boot_ci(diffs)
        tag = " ".join(f"{k}={v}" for k, v in zip(held, rest))
        print(f"  {tag}: median diff {median(diffs):+.3f} [{lo:+.3f}, {hi:+.3f}], "
              f"median of per-launch ratios {median(ratios):.3f}, "
              f"{a} larger in {wins}/{len(d)} launches, sign p = {sign_p(wins, len(d)):.4f}")


def report_pingpong(data):
    if "pp" not in data or not data["pp"]:
        return
    launches = len(data["pp"])
    for field in ("rtt_cycles", "batch_rtt_ns", "per_rt_p99_ns", "core_ghz", "invol_switches"):
        cases = per_launch(data, "pp", field)
        if not cases:
            continue
        print(f"\n## cross core, {field}, one value per launch, {launches} launches")
        for key in sorted(cases):
            xs = cases[key]
            lo, hi = boot_ci(xs)
            tag = " ".join(f"{k}={v}" for k, v in zip(PP_KEY, key))
            print(f"  {tag}: median {median(xs):.3f} [{lo:.3f}, {hi:.3f}] over {len(xs)} launches")
    # The four questions this harness answers, each as a within-launch pairing.
    for field in ("rtt_cycles", "batch_rtt_ns"):
        report_pair(data, "pp", PP_KEY, field, "order", "seqcst", "acqrel", "acqrel being cheaper")
        report_pair(data, "pp", PP_KEY, field, "order", "fenced", "acqrel", "acqrel being cheaper")
        report_pair(data, "pp", PP_KEY, field, "pad", "128", "64", "pad=64 being cheaper")
        report_pair(data, "pp", PP_KEY, field, "wait", "isb", "wfe", "wfe being cheaper")
        # The binary labels these "bg" (BACKGROUND, efficiency cores) and
        # "ui" (USER_INTERACTIVE, performance cores).
        report_pair(data, "pp", PP_KEY, field, "qos", "bg", "ui", "the performance cores being faster")


def main():
    if len(sys.argv) < 2:
        print(__doc__)
        sys.exit(2)
    data = parse(sys.argv[1])
    if not data:
        print("no lines carrying launch= were found")
        sys.exit(1)
    for field in ("msg_cycles", "ns_per_msg", "msg_l1d_miss"):
        report_padding(data, field)
    report_pipeline(data)
    report_pingpong(data)


if __name__ == "__main__":
    main()
