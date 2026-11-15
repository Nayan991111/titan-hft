#!/usr/bin/env python3
"""Mean CPU power and cluster residency per benchmark arm, from powermetrics.

`powermetrics` seizes the Apple Silicon PMU, so it cannot run while a binary
is reading hardware counters through kperf. Energy is therefore a separate
run with no PMU, and this script reads what powermetrics wrote.

One powermetrics process is started per arm and stopped when that arm ends,
so each raw file holds only that arm's samples and no alignment of
timestamps is needed. The first and last sample of each file are dropped,
because the first may begin before the binary and the last may end after it.

Input is a manifest, one arm per line, whitespace separated:

    arm_name    raw_file    messages

`messages` may be 0 for the idle baseline. Energy per message is reported
against the baseline arm named `idle` when it is present, since the figure
of interest is what the benchmark adds to the machine rather than what the
machine draws at rest.

    python3 scripts/energy.py MANIFEST [--raw-dir DIR]

The parser is deliberately loud: if a field is missing it says which file
and which field, and reports no number for it, rather than substituting a
zero that would read as a measurement.
"""

import re
import sys
import os

SAMPLE_SPLIT = re.compile(r"\*\*\*\s*Sampled system activity")

FIELDS = {
    "cpu_power_mw": re.compile(r"^CPU Power:\s*([0-9.]+)\s*mW", re.M),
    "package_power_mw": re.compile(r"^(?:Package|Combined) Power[^:]*:\s*([0-9.]+)\s*mW", re.M),
    "p_residency_pct": re.compile(r"^P-Cluster HW active residency:\s*([0-9.]+)%", re.M),
    "e_residency_pct": re.compile(r"^E-Cluster HW active residency:\s*([0-9.]+)%", re.M),
    "p_freq_mhz": re.compile(r"^P-Cluster HW active frequency:\s*([0-9.]+)\s*MHz", re.M),
    "e_freq_mhz": re.compile(r"^E-Cluster HW active frequency:\s*([0-9.]+)\s*MHz", re.M),
}
ELAPSED = re.compile(r"\(([0-9.]+)ms elapsed\)")


def median(xs):
    s = sorted(xs)
    n = len(s)
    if n == 0:
        return None
    return s[n // 2] if n % 2 else (s[n // 2 - 1] + s[n // 2]) / 2


def mean(xs):
    return sum(xs) / len(xs) if xs else None


def parse_file(path):
    """Returns (per-field list of values, number of samples kept, notes)."""
    notes = []
    try:
        text = open(path, encoding="utf-8", errors="replace").read()
    except OSError as e:
        return {}, 0, [f"cannot read {path}: {e}"]
    blocks = SAMPLE_SPLIT.split(text)[1:]
    if not blocks:
        return {}, 0, [f"{os.path.basename(path)}: no sample blocks found. "
                       f"powermetrics may have written a different format; the raw file is kept so it can be parsed by hand"]
    # Drop the first and last sample: they can straddle the arm's boundaries.
    kept = blocks[1:-1] if len(blocks) >= 3 else blocks
    out = {k: [] for k in FIELDS}
    elapsed = []
    for b in kept:
        for name, rx in FIELDS.items():
            m = rx.search(b)
            if m:
                out[name].append(float(m.group(1)))
        m = ELAPSED.search(b)
        if m:
            elapsed.append(float(m.group(1)))
    out["_elapsed_ms"] = elapsed
    for name in FIELDS:
        if not out[name]:
            notes.append(f"{os.path.basename(path)}: field '{name}' never appeared in {len(kept)} samples")
    return out, len(kept), notes


def main():
    if len(sys.argv) < 2:
        print(__doc__)
        sys.exit(2)
    manifest = sys.argv[1]
    raw_dir = ""
    if "--raw-dir" in sys.argv:
        raw_dir = sys.argv[sys.argv.index("--raw-dir") + 1]

    arms = []
    for line in open(manifest, encoding="utf-8"):
        line = line.strip()
        if not line or line.startswith("#"):
            continue
        parts = line.split()
        if len(parts) < 3:
            print(f"skipping malformed manifest line: {line}")
            continue
        arms.append((parts[0], os.path.join(raw_dir, parts[1]) if raw_dir else parts[1], int(parts[2])))

    results = {}
    all_notes = []
    print("## per arm, mean over samples (first and last sample of each arm dropped)")
    for name, path, msgs in arms:
        vals, n, notes = parse_file(path)
        all_notes += notes
        if not vals:
            print(f"  {name}: no data")
            continue
        results[name] = (vals, msgs)
        secs = sum(vals.get("_elapsed_ms", [])) / 1000.0
        bits = [f"samples {n}", f"window {secs:.2f} s"]
        for f in ("cpu_power_mw", "package_power_mw", "p_residency_pct", "e_residency_pct", "p_freq_mhz"):
            m = mean(vals.get(f, []))
            if m is not None:
                unit = "mW" if f.endswith("_mw") else ("%" if f.endswith("_pct") else "MHz")
                bits.append(f"{f}={m:.1f}{unit}")
        print(f"  {name}: " + "  ".join(bits))

    base = results.get("idle")
    if base is None:
        print("\n(no arm named 'idle', so no baseline subtraction is reported)")
    else:
        bp = mean(base[0].get("cpu_power_mw", []))
        print(f"\n## above the idle baseline (idle CPU power {bp:.1f} mW)" if bp is not None else "\n## no idle CPU power parsed")
        if bp is not None:
            for name, (vals, msgs) in results.items():
                if name == "idle":
                    continue
                p = mean(vals.get("cpu_power_mw", []))
                if p is None:
                    continue
                secs = sum(vals.get("_elapsed_ms", [])) / 1000.0
                extra_mw = p - bp
                line = f"  {name}: CPU power +{extra_mw:.1f} mW over idle"
                if msgs > 0 and secs > 0:
                    # Energy the arm added, divided by the messages it processed.
                    nj = extra_mw * 1e-3 * secs / msgs * 1e9
                    line += f", {nj:.1f} nJ per message over {msgs} messages in {secs:.2f} s"
                print(line)

    # Ratios between wait strategies at the same load, which is the question.
    print("\n## wait strategies at the same load, mean CPU power above idle")
    if base is not None and mean(base[0].get("cpu_power_mw", [])) is not None:
        bp = mean(base[0].get("cpu_power_mw", []))
        by_load = {}
        for name, (vals, msgs) in results.items():
            if name == "idle" or "_" not in name:
                continue
            load, wait = name.rsplit("_", 1)
            p = mean(vals.get("cpu_power_mw", []))
            if p is not None:
                by_load.setdefault(load, {})[wait] = p - bp
        for load, waits in sorted(by_load.items()):
            if len(waits) < 2:
                continue
            lo = min(waits, key=lambda w: waits[w])
            for w in sorted(waits):
                r = waits[w] / waits[lo] if waits[lo] > 0 else float("nan")
                print(f"  {load}: {w} = {waits[w]:.1f} mW above idle ({r:.3f}x the cheapest, {lo})")

    if all_notes:
        print("\n## notes, read these before trusting anything above")
        for n in dict.fromkeys(all_notes):
            print("  " + n)


if __name__ == "__main__":
    main()
