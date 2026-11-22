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


# A file may carry another run's derived lines when the registration compares
# the two and a reader of either needs to see what it was compared against.
# H80 and H81 are such a pair: part (g) of H81 is the agreement of their
# directions. The cross reference is declared here so that it is a decision
# rather than an accident, which is the whole difference between this and D32.
CROSS = {
    ("2026-11-22_m4_h80_viol_prodside.txt", "h81"):
        "H81 part (g) compares the two runs, so each file carries both",
    ("2026-11-22_m4_h81_viol_consumer.txt", "h80"):
        "H81 part (g) compares the two runs, so each file carries both",
}


def check_derived_keys() -> list:
    """A derived record key must name the hypothesis whose file it sits in.

    The H84 verdict script was written from the H83 one and the record key was
    not renamed with everything else, so ten lines of H84 statistics were
    appended to the H84 file under the key h83_cell. Nothing read them wrongly
    because nothing had read them yet, and a script grouping by prefix would
    have merged two runs. That is D32. The rule is mechanical: if a results
    file name carries an hNN, every hNN_ key inside it must carry the same one.
    """
    import glob
    import os
    import re
    bad = []
    checked = 0
    for path in sorted(glob.glob(os.path.join(os.path.dirname(os.path.dirname(
            os.path.abspath(__file__))), "results", "*.txt"))):
        name = os.path.basename(path)
        m = re.search(r"_(h\d+)_", name)
        if not m:
            continue
        want = m.group(1)
        with open(path, encoding="utf-8", errors="replace") as fh:
            keys = {k for line in fh
                    for k in re.findall(r"^(h\d+)_[a-z_]+ ", line)}
        if want not in keys:
            bad.append(f"{name} carries no {want}_ derived key of its own")
        for k in sorted(keys):
            checked += 1
            if k == want:
                continue
            why = CROSS.get((name, k))
            if why:
                print(f"  cross reference: {name} also carries {k}_, {why}")
            else:
                bad.append(f"{name} carries derived key {k}_ but its name says {want}")
    print(f"derived record keys  : {checked} checked against their file name")
    return bad


def _main_with_keys() -> int:
    rc = main()
    bad = check_derived_keys()
    for b in bad:
        print("FAIL: " + b)
    return 1 if (rc or bad) else 0


if __name__ == "__main__":
    sys.exit(_main_with_keys())
