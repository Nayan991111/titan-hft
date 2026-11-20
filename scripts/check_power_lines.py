"""Every results file must state its power condition, including when there is none.

Twenty-five files carried no POWER field at all. The run script began capturing
the power state on 2026-11-17, and every file before that date is silent on it.
Three further files are legitimately exempt: two measured virtualized x86-64
hosts with no battery, one is pure computation on arrival schedules.

A blank field is indistinguishable, to a reader, from an omission on a
battery-powered laptop measurement, and on a fanless machine the power state
bounds the sustained clock. So the field is now present in every file and says
either a measured state, or "not applicable" with a reason, or "not recorded"
with a reason. None of them is guessed: a run whose power state was not
captured says so, because inventing one would be worse than admitting the gap.
"""
import re
import sys
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]
RES = ROOT / "results"

def main() -> int:
    missing, na, nr, ac, batt, other = [], [], [], [], [], []
    for f in sorted(RES.glob("*.txt")):
        head = "".join(f.open(encoding="utf-8", errors="replace").readlines()[:40])
        m = re.search(r"^POWER\s*:\s*(.+)$", head, re.M)
        if not m:
            missing.append(f.name)
            continue
        v = m.group(1).strip()
        if v.lower().startswith("not applicable"):
            if len(v) < len("not applicable") + 8:
                other.append((f.name, "says not applicable without a reason"))
            else:
                na.append(f.name)
        elif v.lower().startswith("not recorded"):
            if len(v) < len("not recorded") + 8:
                other.append((f.name, "says not recorded without a reason"))
            else:
                nr.append(f.name)
        elif re.search(r"\bAC\b", v):
            ac.append(f.name)
        elif "Battery" in v:
            batt.append(f.name)
        else:
            other.append((f.name, v[:60]))

    print(f"files checked        : {len(list(RES.glob('*.txt')))}")
    print(f"wall power           : {len(ac)}")
    print(f"battery              : {len(batt)}")
    print(f"not applicable       : {len(na)}")
    for n in na:
        print(f"    {n}")
    print(f"not recorded         : {len(nr)}")
    if batt:
        print("  on battery:")
        for n in batt:
            print(f"    {n}")

    bad = [f"{n} has no POWER line" for n in missing] + [f"{n}: {w}" for n, w in other]
    if bad:
        print()
        for b in bad:
            print("FAIL: " + b)
        return 1
    print("PASS: every results file states its power condition")
    return 0

if __name__ == "__main__":
    sys.exit(main())
