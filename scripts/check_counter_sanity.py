"""No performance counter may be dead across a whole results file.

A counter that is not actually programmed reads zero, and a zero is
indistinguishable from a real measurement of nothing unless something checks.
H78 shipped 900 records in which the retired store miss field held exactly one
distinct value, 0.0000, and the verdict script divided by it. That is D19 and
D21.

The rule here is weaker than "no zeros" and stronger than nothing: within one
results file, a counter derived field that never varies across hundreds of
records over tens of launches is reporting a constant, and a hardware counter
over varying work does not do that. One distinct value is the signature of a
counter that was never programmed.

Known exceptions are listed with the defect that explains them, and are
removed when the measurement is redone, not when the gate becomes
inconvenient.
"""

import glob
import os
import re
import sys
from collections import defaultdict

ROOT = os.path.dirname(os.path.dirname(os.path.abspath(__file__)))
RES = os.path.join(ROOT, "results")

FIELD = re.compile(
    r"\b(msg_cycles|msg_instructions|msg_l1d_miss|msg_l1d_store_miss|"
    r"msg_br_mispred|msg_ldst_wait_cycles|core_ghz)=([0-9.]+)")

# (file, field): the defect that explains why this one is allowed to be dead.
EXCEPTIONS = {
    ("2026-11-21_m4_h78_storeprobe.txt", "msg_l1d_store_miss"):
        "D19 and D21: the event read zero on every arm, part (a) is recorded "
        "as unevaluable rather than passed, and the run is to be redone once "
        "the cause is established",
}

# A constant is weak evidence over a handful of records and strong evidence
# over dozens. Ten is the floor at which a dead counter is still unambiguous.
# The files below it are named in the output rather than passed over in
# silence, because a gate whose coverage is invisible is a gate nobody can
# audit.
MIN_RECORDS = 10


def main() -> int:
    files = sorted(glob.glob(os.path.join(RES, "*.txt")))
    dead, allowed, checked, thin, covered = [], [], 0, [], []
    for path in files:
        name = os.path.basename(path)
        vals = defaultdict(set)
        rows = 0
        with open(path, encoding="utf-8", errors="replace") as fh:
            for line in fh:
                found = FIELD.findall(line)
                if not found:
                    continue
                rows += 1
                for k, v in found:
                    vals[k].add(v)
        # The first version of this gate read only lines beginning with "bd ",
        # and so scanned 10 counter fields when 13 results files carry them.
        # A record prefix is a convention of one harness, not of the data.
        if not vals:
            continue
        if rows < MIN_RECORDS:
            thin.append((name, rows))
            continue
        covered.append(name)
        for k, s in sorted(vals.items()):
            checked += 1
            if len(s) > 1:
                continue
            if (name, k) in EXCEPTIONS:
                allowed.append((name, k))
            else:
                dead.append((name, k, s.pop()))

    print(f"results files scanned : {len(files)}")
    print(f"files with counters   : {len(covered) + len(thin)}, of which "
          f"{len(covered)} checked and {len(thin)} below the {MIN_RECORDS} "
          f"record floor")
    for name, rows in thin:
        print(f"  thin: {name}, {rows} record(s)")
    print(f"counter fields checked: {checked}")
    for name, k in allowed:
        print(f"  allowed: {name} {k} is constant, {EXCEPTIONS[(name, k)]}")
    if dead:
        print()
        for name, k, v in dead:
            print(f"FAIL: {name} field {k} holds one value, {v}, across the whole "
                  f"file; a counter over varying work does not do that")
        return 1
    print("PASS: every counter field varies, or is a listed exception with its defect")
    return 0


if __name__ == "__main__":
    sys.exit(main())
