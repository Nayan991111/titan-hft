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

import re
import sys
import random
from collections import defaultdict

# WARNING before changing this tuple: it is unpacked positionally, as
# `order, pad, cached, wait = key`, in two places inside report_padding. Adding
# a field here raises there and the whole streaming section vanishes without an
# error anyone reads. An attempt to append "messages" so that a run-length
# sweep could be paired inside one file did exactly that: nineteen lines of
# statistics became zero, caught only by running the analyzer on the same input
# before and after. A sweep over message counts is done across separate raw
# files instead, which needs no change here.
RING_KEY = ("order", "pad", "cached", "wait")

# The tick-to-trade harness: one row per stage of the trading path.
# Deliberately a SEPARATE constant from RING_KEY even though the field names
# coincide today. RING_KEY is unpacked positionally inside report_padding and
# carries the warning above; nothing here is unpacked positionally, so the two
# must be free to diverge without one silently breaking the other.
BD_KEY = ("order", "pad", "cached", "wait")

T2T_KEY = ("feed", "stage")
PAIR_KEY = ("order", "cached", "wait")
# The cross-core harness adds the QoS class, which selects performance or
# efficiency cores, so it is part of what identifies a case.
PP_KEY = ("order", "pad", "cached", "wait", "qos")

GREEKS_KEY = ("arm", "ladder", "qos")

PATHS_KEY = ("arm", "dist", "qos")


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


def launch_key(x):
    """Sort order for a launch label.

    Labels are not always bare numbers: a run that sweeps two QoS classes
    tags them `ui1` and `bg1` so the two sets cannot collide. Sorting those
    with `int` raises, which is how one run's entire statistics section was
    lost after the labels changed and this function did not exist.
    """
    m = re.match(r"^([A-Za-z_]*)(\d+)$", str(x))
    if m:
        return (m.group(1), int(m.group(2)))
    return (str(x), 0)


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
        elif kind == "bd":
            key = tuple(d.get(k, "") for k in BD_KEY)
        elif kind == "pp":
            key = tuple(d.get(k, "") for k in PP_KEY)
        elif kind == "t2t":
            key = tuple(d.get(k, "") for k in T2T_KEY)
        elif kind == "paths_cost":
            key = tuple(d.get(k, "") for k in PATHS_KEY)
        elif kind == "greeks_cost":
            key = tuple(d.get(k, "") for k in GREEKS_KEY)
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
    for launch in sorted(data[kind], key=launch_key):
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
    for launch in sorted(data["st"], key=launch_key):
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
    launches = sorted(data["pl"], key=launch_key)
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
    for launch in sorted(data[kind], key=launch_key):
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


def report_threshold(data, kind, keynames, field, want, threshold, above=True):
    """Count the launches whose per-launch median crosses a fixed threshold.

    For a hypothesis stated against a number from an earlier run rather than
    against a paired arm, so there is nothing to pair with.
    """
    cases = per_launch(data, kind, field)
    idx = {n: keynames.index(n) for n in want}
    sel = [v for k, v in cases.items() if all(k[i] == want[n] for n, i in idx.items())]
    if not sel or not sel[0]:
        return
    vals = sel[0]
    wins = sum(1 for x in vals if (x > threshold if above else x < threshold))
    tag = " ".join(f"{k}={v}" for k, v in want.items())
    side = "above" if above else "below"
    lo, hi = boot_ci(vals)
    print(f"\n## {kind}: is {field} {side} {threshold} for {tag}?")
    print(f"  median {median(vals):.3f} [{lo:.3f}, {hi:.3f}], {side} {threshold} "
          f"in {wins}/{len(vals)} launches, sign p = {sign_p(wins, len(vals)):.4f}")


def pair_keys(data, kind, keynames, field, want_a, want_b):
    """One (a, b) pair per launch for two fully specified cases.

    `report_pair` varies one field and holds the rest equal, which cannot
    compare two cases that differ in more than one field. This can.
    """
    ia = tuple(keynames.index(n) for n in want_a)
    ib = tuple(keynames.index(n) for n in want_b)
    out = []
    for launch in sorted(data[kind], key=launch_key):
        a = b = None
        for key, fields in data[kind][launch].items():
            if field not in fields or not fields[field]:
                continue
            if all(key[i] == v for i, v in zip(ia, want_a.values())):
                a = median(fields[field])
            if all(key[i] == v for i, v in zip(ib, want_b.values())):
                b = median(fields[field])
        if a is not None and b is not None:
            out.append((a, b))
    return out


def report_cross(data, kind, keynames, field, want_a, want_b, favours):
    """Sign test and ratio for two fully specified cases, paired per launch."""
    d = pair_keys(data, kind, keynames, field, want_a, want_b)
    if not d:
        return
    ta = " ".join(f"{k}={v}" for k, v in want_a.items())
    tb = " ".join(f"{k}={v}" for k, v in want_b.items())
    diffs = [x - y for x, y in d]
    ratios = [x / y for x, y in d if y > 0]
    wins = sum(1 for x in diffs if x > 0)
    lo, hi = boot_ci(diffs)
    print(f"\n## {kind}: [{ta}] against [{tb}] on {field}, paired inside each launch")
    print(f"   positive difference and ratio above 1 favour {favours}")
    print(f"  median diff {median(diffs):+.4f} [{lo:+.4f}, {hi:+.4f}], "
          f"median of per-launch ratios {median(ratios):.4f}, "
          f"first larger in {wins}/{len(d)} launches, sign p = {sign_p(wins, len(d)):.4f}")


def barrier_shares(data, field, cached):
    """Per-launch store-side share, load-side share and ordering fraction.

    H69 asks which half of the acquire-release pair carries the measured
    inversion, so the quantity of interest is a ratio of differences:

        S = (A - FS) / (A - F)    store side's share of the saving
        L = (A - FL) / (A - F)    load side's share
        O = (A - U)  / A          ordering as a fraction of the loop

    where A, FL, FS, F, U are the per-launch median cycles (or nanoseconds)
    per message for acqrel, fencedload, fencedstore, fenced and unordered.

    A launch is dropped from S and L when |A - F| is small, because the share
    is then a ratio by a near-zero denominator and carries no information. The
    count of dropped launches is returned so it can be reported rather than
    hidden: if many are dropped the inversion did not reproduce in this run
    and the shares must not be read at all.
    """
    want = ("acqrel", "fencedload", "fencedstore", "fenced", "unordered")
    io = BD_KEY.index("order")
    ic = BD_KEY.index("cached")
    shares, floors, dropped = [], [], 0
    for launch in sorted(data["bd"], key=launch_key):
        got = {}
        for key, fields in data["bd"][launch].items():
            if key[ic] != cached or field not in fields or not fields[field]:
                continue
            if key[io] in want:
                got[key[io]] = median(fields[field])
        if not all(w in got for w in want):
            continue
        a, fl, fs, f, u = (got[w] for w in want)
        if a > 0:
            floors.append((a - u) / a)
        # 2 percent of A is the smallest denominator worth dividing by: the
        # measured noise floor on this protocol is 1.364x at p99, so a gap
        # below a few percent is not distinguishable from noise.
        if abs(a - f) < 0.02 * abs(a):
            dropped += 1
            continue
        shares.append(((a - fs) / (a - f), (a - fl) / (a - f)))
    return shares, floors, dropped


def report_barrier(data):
    """H69: decompose the Fenced inversion into its load and store sides."""
    if not data.get("bd"):
        return
    for field in ("msg_cycles", "ns_per_msg"):
        for arm in ("fencedload", "fencedstore", "fenced", "unordered"):
            report_pair(data, "bd", BD_KEY, field, "order", "acqrel", arm,
                        f"{arm} being cheaper than acqrel")
    for field in ("msg_cycles", "ns_per_msg"):
        for cached in ("0", "1"):
            shares, floors, dropped = barrier_shares(data, field, cached)
            tag = "cached" if cached == "1" else "uncached"
            print(f"\n## bd: barrier-side shares, {tag}, on {field}")
            if not shares and not floors:
                print("  no launch carried all five arms")
                continue
            if floors:
                lo, hi = boot_ci(floors)
                m = median(floors)
                # H69 (f) as revised: the hypothesis is the SIGN, not a size.
                # A negative value means the unordered arm is slower than the
                # arms it is meant to bound, so it is not a floor and no
                # fraction-of-loop figure may be taken from it.
                verdict = ("usable as a floor" if m > 0 else
                           "NOT A FLOOR: unordered is slower, no ordering "
                           "fraction may be quoted from this arm")
                print(f"  H69 (f) (A-U)/A: median {m:+.4f} [{lo:+.4f}, {hi:+.4f}] "
                      f"over {len(floors)} launches, positive in "
                      f"{sum(1 for x in floors if x > 0)}/{len(floors)}, {verdict}")
            if dropped:
                print(f"  {dropped} launch(es) dropped from the shares: "
                      f"|A-F| below 2 percent of A, so the share has no denominator")
            if not shares:
                print("  no launch retained a usable denominator, shares not reported")
                continue
            s = [x for x, _ in shares]
            ll = [y for _, y in shares]
            sums = [x + y for x, y in shares]
            for name, vals, note in (("S store-side", s, "H69 (a) at or above 0.70"),
                                     ("L load-side", ll, "H69 (b) at or above 0.70")):
                lo, hi = boot_ci(vals)
                print(f"  {name}: median {median(vals):+.4f} [{lo:+.4f}, {hi:+.4f}], "
                      f"{note} in {sum(1 for x in vals if x >= 0.70)}/{len(vals)} launches")
            lo, hi = boot_ci(sums)
            print(f"  S+L: median {median(sums):.4f} [{lo:.4f}, {hi:.4f}], "
                  f"H69 (c) within 0.15 of 1 in "
                  f"{sum(1 for x in sums if abs(x - 1.0) <= 0.15)}/{len(sums)} launches")
    # H69 (g) as revised: the threshold binds on the UNCACHED arms, where the
    # result lives. The cached arms are roughly 20x faster per message by
    # construction and always run short at a fixed message count, so a global
    # threshold across all arms fails on a correct run. That is how H68 (d)
    # failed, and this is the fourth version of this check.
    cases = per_launch(data, "bd", "epoch_ms")
    if cases:
        ic = BD_KEY.index("cached")
        print("\n## bd: H69 (g) epoch duration, threshold on the uncached arms only")
        for cached, label, binds in (("0", "uncached", True), ("1", "cached", False)):
            sel = {k: v for k, v in cases.items() if k[ic] == cached and v}
            if not sel:
                continue
            allv = [x for v in sel.values() for x in v]
            worst = min((min(v), k) for k, v in sel.items())
            if binds:
                print(f"  {label}: shortest epoch {worst[0]:.3f} ms in {worst[1]}, "
                      f"{sum(1 for x in allv if x > 200.0)}/{len(allv)} arm-launches "
                      f"above 200 ms, THRESHOLD BINDS")
            else:
                print(f"  {label}: shortest {worst[0]:.3f} ms, median "
                      f"{median(allv):.3f} ms, recorded without a pass or fail "
                      f"(these arms run short by construction)")


def report_t2t(data):
    """The trading path's stages, each under the independent-launch protocol.

    The figure of 135 cycles per message that H65's headline comparison rests
    on came from three epochs in one process. This reports the same stages
    over independent launches so that comparison can be made against a number
    measured the same way as the one it is compared with.
    """
    if not data.get("t2t"):
        return
    for field in ("msg_cycles", "ns_per_msg"):
        cases = per_launch(data, "t2t", field)
        rows = sorted((k, v) for k, v in cases.items() if v)
        if not rows:
            continue
        print(f"\n## t2t: {field} per stage, median across launches")
        for key, vals in rows:
            lo, hi = boot_ci(vals)
            print(f"  feed={key[0]} stage={key[1]:9} median {median(vals):9.3f} "
                  f"[{lo:9.3f}, {hi:9.3f}] over {len(vals)} launches")
    # The ring's share of the path, paired inside each launch.
    for field in ("msg_cycles", "ns_per_msg"):
        report_pair(data, "t2t", T2T_KEY, field, "stage", "full", "direct", "full")


def report_paths(data):
    """The six parts of H66, each on the arms, distance, metric it names."""
    if not data.get("paths_cost"):
        return
    k = PATHS_KEY
    # (a) geometric against arithmetic averaging: one exponential per step
    report_pair(data, "paths_cost", k, "cyc_step", "arm", "asian_geom", "asian_arith", "asian_geom")
    # (b) the barrier monitored in log space against price space
    report_pair(data, "paths_cost", k, "cyc_step", "arm", "barrier_log", "barrier_price", "barrier_log")
    # (c) branchless extremes against branchy ones
    report_pair(data, "paths_cost", k, "cyc_step", "arm", "lookback_free", "lookback_branchy", "lookback_free")
    # (d) stopping at the first touch, per path because the step count differs
    report_pair(data, "paths_cost", k, "cyc_path", "arm", "barrier_log_exit", "barrier_log", "barrier_log_exit")
    # (e) a stored path against rolling scalars, and the misses that would explain it
    report_pair(data, "paths_cost", k, "cyc_step", "arm", "stored", "rolling", "stored")
    for arm in ("stored", "rolling"):
        report_threshold(data, "paths_cost", k, "l1d_step",
                         {"arm": arm, "dist": "near", "qos": "ui"}, 0.05, above=False)
    # (f) every statistic against bare path generation; the two differ in
    # both arm and distance, so this needs the cross pairing
    for qos in ("ui", "bg"):
        report_cross(data, "paths_cost", k, "cyc_step",
                     {"arm": "rolling", "dist": "near", "qos": qos},
                     {"arm": "terminal", "dist": "na", "qos": qos}, "rolling")
    # Reported alongside, not registered: the branch the barrier distance
    # controls, and the same three savings in wall time.
    for arm in ("barrier_log", "barrier_price", "rolling"):
        report_pair(data, "paths_cost", k, "brmis_step", "dist", "near", "far", "near")
        break
    for a, b in (("asian_geom", "asian_arith"), ("barrier_log", "barrier_price")):
        report_pair(data, "paths_cost", k, "ns_step", "arm", a, b, a)


def report_greeks(data):
    """The six parts of H65, each on the arm, ladder, QoS and metric it names."""
    if not data.get("greeks_cost"):
        return
    k = GREEKS_KEY
    # (a) the six separate functions against the fused kernel
    report_pair(data, "greeks_cost", k, "cycles", "arm", "separate", "fused", "separate")
    # (b) the cumulative normal's share of the fused kernel
    report_pair(data, "greeks_cost", k, "cycles", "arm", "cdf", "fused", "cdf")
    # (c) single precision against double
    report_pair(data, "greeks_cost", k, "cycles", "arm", "f32", "fused", "f32")
    # (d) the hand-hoisted ladder against the per-contract loop
    report_pair(data, "greeks_cost", k, "cycles", "arm", "chain", "loop", "chain")
    # (e) against the 135 cycles per message of the whole trading path
    report_threshold(data, "greeks_cost", k, "cycles",
                     {"arm": "fused", "ladder": "straddle", "qos": "ui"}, 135.0)
    # (f) the sign branch in the cumulative normal
    report_pair(data, "greeks_cost", k, "br_mispredict", "ladder", "straddle", "otm", "straddle")
    # Reported alongside, not registered: the same four pairings in wall time,
    # because H64 showed cycles and nanoseconds disagree on efficiency cores.
    for arm_a, arm_b in (("separate", "fused"), ("f32", "fused"), ("chain", "loop")):
        report_pair(data, "greeks_cost", k, "ns", "arm", arm_a, arm_b, arm_a)


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
    report_barrier(data)
    report_greeks(data)
    report_paths(data)
    report_t2t(data)


if __name__ == "__main__":
    main()
