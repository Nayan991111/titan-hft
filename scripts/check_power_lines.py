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

POWER_WORDS = {
    "on wall power": ("AC", "wall power"),
    "on ac power": ("AC", "wall power"),
    "on battery": ("Battery", "battery"),
}


def check_register_claims(bad: list) -> None:
    """A power claim in a register row must match a run that row cites.

    The H79 row said the run was on wall power. Nobody had read the file. The
    operator then reported running without the charger, so a measurement
    condition that was never checked was one commit from the paper.

    Two things keep this from being an over-strict check, which is the kind
    that gets switched off. Only table rows are examined, because the
    register's prose discusses power states as methodology and history rather
    than as claims about a particular run. And a row that cites several runs
    passes when any one of them matches, because the claim attaches to the run
    it describes, and a row often cites both a preliminary run and the one
    that replaced it.

    The limit of that second rule is stated rather than hidden: a row citing
    several runs passes when any of them records the claimed state, not when
    the one the sentence is about does. That is why the four claims this check
    first caught were corrected in every row they appeared in, including the
    two rows it did not flag, by reading the file the sentence names instead
    of trusting the check. See D26.
    """
    exp = ROOT / "docs" / "experiments.md"
    if not exp.exists():
        return
    checked, skipped = 0, []
    for line in exp.read_text(encoding="utf-8").split("\n"):
        if not line.startswith("|"):
            continue
        low = line.lower()
        claims = [w for w in POWER_WORDS if w in low]
        if not claims:
            continue
        files = re.findall(r"results/([\w.\-]+\.txt)", line)
        if not files:
            continue
        row = line.split("|")[1].strip()[:8]
        for claim in claims:
            token = POWER_WORDS[claim][0]
            seen, matched, absent = [], False, []
            for fn in files:
                f = RES / fn
                if not f.exists():
                    absent.append(fn)
                    continue
                head = "".join(f.open(encoding="utf-8", errors="replace").readlines()[:40])
                for v in re.findall(r"^POWER\s*:\s*(.+)$", head, re.M):
                    seen.append(f"{fn}={v.strip()[:40]!r}")
                    if token in v:
                        matched = True
            if matched:
                checked += 1
            elif absent and not seen:
                # A file missing from this checkout is not a false claim. This
                # happens when a run was committed from the machine that
                # produced it and the claim is read somewhere that has not
                # fetched it. It is reported, loudly, and not counted as
                # verified, because an unread file verifies nothing.
                skipped.append((row, POWER_WORDS[claim][1], absent))
            else:
                bad.append(("docs/experiments.md",
                            f"row {row} claims {POWER_WORDS[claim][1]} and no run it "
                            f"cites records that; the cited runs say "
                            f"{'; '.join(seen) if seen else 'nothing'}"))
    print(f"register power claims : {checked} verified against a cited run")
    for row, word, absent in skipped:
        print(f"  NOT VERIFIED HERE: row {row} claims {word}; "
              f"{', '.join(absent)} is not in this checkout")


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
    check_register_claims(other if isinstance(other, list) else [])
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
