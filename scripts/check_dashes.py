#!/usr/bin/env python3
"""Fail on any em dash or en dash this project's authors wrote.

Six dashes appear in the compiled manuscript and none of them come from the
prose: IEEEtran.cls emits one after "Abstract" and one after "Index Terms",
and IEEEtran.bst emits a repeated-author dash and normalises page ranges into
en dashes. Those are IEEE house typography and editing the class and style
files to remove them would make the manuscript non-conformant to the format it
claims. So this check reads SOURCE, never the compiled PDF, and in LaTeX source
it also rejects the ligature sequences that render as dashes even though the
file contains no dash character.

    python3 scripts/check_dashes.py

Exit 0 when clean, 1 otherwise, listing every file and line.
"""
from __future__ import annotations

import re
import sys
from pathlib import Path

ROOT = Path(__file__).resolve().parent.parent
EM, EN = "—", "–"
PATTERNS = ("docs/**/*.md", "docs/**/*.tex", "docs/**/*.bib", "results/*.txt",
            "scripts/*.py", "scripts/*.sh", "crates/**/*.rs", "README.md")
# A line in this file, or in the manuscript's own explanation of the rule, has
# to be able to name the thing it forbids.
SELF = {"scripts/check_dashes.py", "docs/paper/README.md"}
# A line may opt out with this marker, and only a line whose purpose is to
# REMOVE a dash should ever carry it: the \patchcmd calls in the manuscript
# preamble have to name the ligature they delete. The marker is deliberately
# verbose so it cannot be added by accident and shows up in a diff.
ALLOW = "dash-check-allow-this-line-removes-a-dash"
LIGATURE = re.compile(r"(?<!-)---?(?!-)")


def main() -> int:
    bad: list[str] = []
    checked = 0
    for pat in PATTERNS:
        for f in sorted(ROOT.glob(pat)):
            rel = f.relative_to(ROOT).as_posix()
            if rel in SELF:
                continue
            try:
                text = f.read_text(encoding="utf-8")
            except (UnicodeDecodeError, OSError):
                continue
            checked += 1
            for n, line in enumerate(text.splitlines(), 1):
                if ALLOW in line:
                    continue
                if EM in line or EN in line:
                    bad.append(f"{rel}:{n}: dash character: {line.strip()[:90]}")
                # In LaTeX source the ligatures render as dashes. Comments do
                # not reach the page, so they are exempt.
                if f.suffix in (".tex", ".bib") and not line.lstrip().startswith("%"):
                    if LIGATURE.search(line):
                        bad.append(f"{rel}:{n}: renders as a dash: {line.strip()[:90]}")
    for b in bad:
        print(b)
    print(f"checked {checked} files, {len(bad)} violations")
    return 1 if bad else 0


if __name__ == "__main__":
    sys.exit(main())
