"""Every line in a result file that looks like a machine record must be one.

Result files deliberately mix three things: prose written for a human, the
output of an analysis script, and the raw records the run emitted. Parsers in
this project select records by their first token, so a prose or analysis line
that happens to begin with a record prefix is silently counted as data.

That is not hypothetical. `scripts/capacity_envelope.py` printed a summary
line reading "pl lines: 1080" into two result files. Both files then contained
1081 lines beginning with "pl ", of which 1080 were records, and the
verification step of the run script that wrote them failed on the difference
after the data had already been pushed. The analysis was correct and the data
was intact; the file was shaped so that counting it was wrong.

This gate makes that class of defect impossible to commit. For every file in
results/, every line whose first token is a known record prefix must carry at
least one key=value pair, which is what every real record has and what no
sentence of prose has.

Usage:
    python3 -I scripts/check_record_lines.py
"""

import glob
import os
import re
import sys

PREFIXES = {
    "pl", "pl_ccdf", "pl_config", "pl_split", "pl_gap",
    "st", "bd", "pp", "t2t", "paths_cost", "greeks_cost",
}

KV = re.compile(r"[A-Za-z_][A-Za-z0-9_]*=[^\s]")

# Measurement records written on 11 and 12 November carry human readable
# summary tables whose rows begin with a record prefix. They are allowed here,
# by exact count so that appending another one still fails, because a result
# file is a record of a run and this project does not rewrite one to satisfy a
# gate added afterwards. Nothing reads them as data: every committed parser
# requires a key=value pair before it keeps a line, which was verified by
# feeding one of these lines to scripts/analyze.py on its own. Its key is
# entirely "na", no real record produces that key, and the group is dropped at
# report time because it holds no metric value.
LEGACY = {
    ("2026-11-11_m4_spsc.txt", "pp"): 7,
    ("2026-11-12_m4_ac.txt", "pl_gap"): 6,
    ("2026-11-12_m4_ac2.txt", "pl_gap"): 6,
}


def main():
    root = os.path.join(os.path.dirname(os.path.dirname(os.path.abspath(__file__))), "results")
    files = sorted(glob.glob(os.path.join(root, "*.txt")))
    if not files:
        print("FAIL: no result files found")
        return 1

    bad = []
    seen = {}
    checked = 0
    records = 0
    for path in files:
        with open(path, encoding="utf-8", errors="replace") as fh:
            for n, line in enumerate(fh, 1):
                checked += 1
                token = line.split(" ", 1)[0]
                if token not in PREFIXES:
                    continue
                if KV.search(line):
                    records += 1
                    continue
                seen[(os.path.basename(path), token)] = seen.get(
                    (os.path.basename(path), token), 0) + 1
                bad.append((os.path.basename(path), token, n, line.rstrip()[:62]))

    unexpected = [b for b in bad if seen.get((b[0], b[1]), 0) != LEGACY.get((b[0], b[1]))]
    for key, count in LEGACY.items():
        if seen.get(key, 0) != count:
            unexpected.append((key[0], key[1], 0,
                               f"allowlisted count changed: expected {count}, found {seen.get(key, 0)}"))

    if unexpected:
        print(f"FAIL: {len(unexpected)} line(s) begin with a record prefix and carry no key=value pair.")
        print("Prefix based tooling counts such a line as data.")
        for name, token, n, text in unexpected[:20]:
            print(f"  {name}:{n}  [{token}]  {text}")
        if len(unexpected) > 20:
            print(f"  ... and {len(unexpected) - 20} more")
        print("Fix the writer so its prose does not begin with a record prefix.")
        return 1

    allowed = sum(LEGACY.values())
    print(f"checked {len(files)} result file(s), {checked} line(s), {records} record(s), "
          f"0 new prose lines wearing a record prefix ({allowed} allowlisted from 11 and 12 November)")
    return 0


if __name__ == "__main__":
    sys.exit(main())
