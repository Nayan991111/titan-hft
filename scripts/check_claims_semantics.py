"""Semantic checks on the manuscript: is each number the RIGHT number.

Every other gate in this repository is mechanical. They ask whether a value
reproduces: whether a figure regenerates byte for byte, whether a second
implementation agrees, whether a file has the expected record count. All of
them passed while the manuscript used a count statistic where queueing theory
requires a gap statistic, and while its lead figure rested on an arm that
fails its own correctness check without saying so.

This file asks the other question. Each check below corresponds to a defect in
docs/defects.md and exists so that defect cannot return silently.

Usage:
    python3 -I scripts/check_claims_semantics.py
"""

import re
import sys
from pathlib import Path

ROOT = Path(__file__).resolve().parent.parent
TEX = ROOT / "docs" / "paper" / "main.tex"
DOC = ROOT / "docs" / "experiments.md"
DEFECTS = ROOT / "docs" / "defects.md"


def main():
    raw = TEX.read_text(encoding="utf-8")
    # LaTeX wraps prose at the column, so a phrase this file searches for can
    # be split across a newline. Collapsing whitespace before matching is the
    # difference between checking the manuscript and checking its line breaks.
    tex = re.sub(r"\s+", " ", raw)
    doc = DOC.read_text(encoding="utf-8")
    bad = []

    # D1. The hypothesis count in the manuscript must be derived from the table.
    rows = re.findall(r"^\| ([HE]\d+[a-c]?) \|", doc, re.M)
    h = sum(1 for r in rows if r.startswith("H"))
    e = sum(1 for r in rows if r.startswith("E"))
    claimed = re.search(r"pre-registered protocol, (\d+) hypothesis rows and (\w+) exploratory", tex)
    if not claimed:
        bad.append("D1: the abstract no longer states the protocol size in the expected form")
    else:
        words = {"one": 1, "two": 2, "three": 3, "four": 4, "five": 5, "six": 6}
        got_h = int(claimed.group(1))
        got_e = words.get(claimed.group(2).lower(), -1)
        if got_h != h:
            bad.append(f"D1: abstract says {got_h} hypothesis rows, the table has {h}")
        if got_e != e:
            bad.append(f"D1: abstract says {claimed.group(2)} exploratory rows, the table has {e}")

    # D2. The cost-floor figure may never appear without saying the arm is not
    # a correct ring. Checked within the sentence block that carries it.
    for m in re.finditer(r"95\.3", tex):
        window = tex[max(0, m.start() - 400): m.start() + 900]
        if "cost floor" not in window or "not a correct ring" not in window:
            bad.append("D2: a 95.3 percent claim appears without the cost-floor disclosure beside it")
            break

    # D8. Fano is a count statistic. Wherever the manuscript uses it, the
    # distinction from the interarrival coefficient of variation must be drawn,
    # because the approximation consumes the latter.
    if "Fano" in tex and "coefficient of variation" not in tex:
        bad.append("D8: the manuscript uses the Fano factor without naming the coefficient of variation")
    for m in re.finditer(r"Fano", tex):
        window = tex[max(0, m.start() - 700): m.start() + 700]
        if "coefficient of variation" not in window:
            bad.append("D8: a Fano factor mention has no coefficient of variation within its paragraph")
            break

    # D5. The manuscript must not claim unconditional one-command reproduction
    # while the artifact script re-measures on the host it runs on.
    for phrase in ("reproduces every figure and table with one command",
                   "artifact reproduces every"):
        if phrase in tex:
            bad.append(f"D5: the manuscript still claims '{phrase}'")

    # The row count is written twice in the manuscript, once as a digit in the
    # abstract and once as a word in the pre-registration subsection. The check
    # read the digit and not the word, so the word went stale at 'Eighty' while
    # the table held 81 and every gate passed. Both forms are checked now.
    WORDS = {
        70: "Seventy", 71: "Seventy-one", 72: "Seventy-two", 73: "Seventy-three",
        74: "Seventy-four", 75: "Seventy-five", 76: "Seventy-six",
        77: "Seventy-seven", 78: "Seventy-eight", 79: "Seventy-nine",
        80: "Eighty", 81: "Eighty-one", 82: "Eighty-two", 83: "Eighty-three",
        84: "Eighty-four", 85: "Eighty-five", 86: "Eighty-six",
        87: "Eighty-seven", 88: "Eighty-eight", 89: "Eighty-nine", 90: "Ninety",
    }
    for word in set(WORDS.values()):
        if f"{word} hypothesis rows" in tex and word != WORDS.get(h):
            bad.append(f"the manuscript says '{word} hypothesis rows' in prose but the "
                       f"table holds {h}")
    if h in WORDS and f"{WORDS[h]} hypothesis rows" not in tex:
        bad.append(f"the manuscript never states the row count in words; the "
                   f"pre-registration subsection is expected to say "
                   f"'{WORDS[h]} hypothesis rows'")

    # A committed document that contradicts the manuscript is worse than no
    # document. docs/paper.md is the frozen Gate 4 planning note; it must say so.
    pm = ROOT / "docs" / "paper.md"
    if pm.exists() and "SUPERSEDED PLANNING DOCUMENT" not in pm.read_text(encoding="utf-8"):
        bad.append("docs/paper.md is a superseded planning document and no longer "
                   "carries the banner saying so, so a reader would take its stale "
                   "claims for current ones")

    # D10. The protocol is not uniform: the run-length sweep used 20
    # independent launches where everything else used 30, and the abstract
    # claimed 30 for all of it.
    #
    # The first version of this check took the lowest launch count mentioned
    # anywhere under results/ and compared it with the abstract. That is too
    # clever: those files also mention pilot runs in prose, so it reported a
    # failure against a number no comparison in the paper uses. A results file
    # alone cannot say which protocol produced which number; the row in
    # docs/experiments.md says that, and no regular expression reads it
    # reliably.
    #
    # So the rule here is narrow and enforceable: the manuscript may state the
    # launch count only as a floor, never as a flat uniform claim. A flat
    # claim is the defect, whatever number it carries.
    flat = [n for n in re.findall(r"\b(\d+) independent launches per comparison", tex)
            if f"at least {n} independent launches per comparison" not in tex]
    if flat:
        bad.append(f"the manuscript claims a uniform '{flat[0]} independent launches per "
                   f"comparison'; the protocol is not uniform, so it must be stated as a floor")

    # D8 again, in the other direction. The 13.503 result is only meaningful
    # against the statistic it is a ratio of. Stated without the coefficient of
    # variation beside it, a reader would take it for a burstiness effect in
    # general, which is the error D8 records: the Fano factor of counts predicts
    # 1.040 for this same comparison.
    for frag in ("13.503", "13.5030"):
        if frag in tex:
            i = tex.index(frag)
            near = tex[max(0, i - 1200):i + 1200]
            if "coefficient of variation" not in near:
                bad.append(f"'{frag}' appears without 'coefficient of variation' nearby; "
                           "the ratio is meaningless without the statistic it is taken over")
            if "Fano" not in near:
                bad.append(f"'{frag}' appears without naming the Fano factor nearby; "
                           "the result is a comparison between two predictors, not one number")

    # arXiv's submission form caps the abstract field at 1920 characters and
    # silently truncates or rejects beyond it. The abstract reached 909 words
    # and about 5800 characters by accretion, every addition individually
    # justified, and the paper could not have been submitted anywhere. A limit
    # that lives in a web form is invisible until the day it blocks you, so it
    # lives here instead. See D33.
    ARXIV_ABSTRACT_CHARS = 1920
    ab = re.search(r"\\begin\{abstract\}(.*?)\\end\{abstract\}", tex, re.S)
    if not ab:
        bad.append("the manuscript has no abstract")
    else:
        plain = re.sub(r"\\[a-zA-Z]+(\{[^}]*\})?", " ", ab.group(1)).strip()
        plain = re.sub(r"\s+", " ", plain)
        n = len(plain)
        if n > ARXIV_ABSTRACT_CHARS:
            bad.append(f"the abstract is {n} characters and arXiv accepts "
                       f"{ARXIV_ABSTRACT_CHARS}; it cannot be submitted as it stands")
        else:
            print(f"abstract             : {n} characters, {len(plain.split())} words, "
                  f"{ARXIV_ABSTRACT_CHARS - n} under the arXiv limit")

    # The register's status line must agree with its own markers. It did not,
    # the first time the two were written in the same commit.
    # The map stopped at ten and the register reached eleven, so the check
    # reported a parse failure (-1) as though the count disagreed. A word list
    # that runs out is a check that breaks on success, which is worse than no
    # check: it cries wolf exactly when the work is going well.
    words = {"none": 0, "zero": 0, "one": 1, "two": 2, "three": 3, "four": 4,
             "five": 5, "six": 6, "seven": 7, "eight": 8, "nine": 9, "ten": 10,
             "eleven": 11, "twelve": 12, "thirteen": 13, "fourteen": 14,
             "fifteen": 15, "sixteen": 16, "seventeen": 17, "eighteen": 18,
             "nineteen": 19, "twenty": 20, "thirty": 30, "forty": 40,
             "fifty": 50}

    def as_count(tok: str) -> int:
        """Read a digit or a number word, compound forms included.

        The list stopped at a single word and the register reached twenty one,
        so the check failed on a correct status line for the second time. The
        lesson from the first time was written three lines above and still
        applied to only the cases already seen. Summing the parts reads every
        form the register can take without the list needing to be extended
        again.
        """
        tok = tok.strip().lower()
        if tok.isdigit():
            return int(tok)
        parts = [w for w in tok.replace("-", " ").split() if w]
        if not parts or any(w not in words for w in parts):
            return -1
        return sum(words[w] for w in parts)
    reg_raw = DEFECTS.read_text(encoding="utf-8") if DEFECTS.exists() else ""
    status = re.search(
        r"\*\*Status at [\d-]+: ([\w\- ]+?) closed, ([\w\- ]+?) partly closed, "
        r"([\w\- ]+?) open", reg_raw)
    if not status:
        bad.append("the register has no status line in the expected form")
    else:
        said = [as_count(status.group(i)) for i in (1, 2, 3)]
        if -1 in said:
            bad.append("the register's status line uses a number word this check "
                       "cannot read; write it as a digit or extend the word list")
        counted = [reg_raw.count("**CLOSED"), reg_raw.count("**PARTLY"), reg_raw.count("**OPEN.")]
        if said != counted:
            bad.append(f"the register's status line says {said} closed/partly/open "
                       f"but its markers count {counted}")

    # Every defect that has an identifier must be in the register.
    reg = DEFECTS.read_text(encoding="utf-8") if DEFECTS.exists() else ""
    for d in ("D1", "D2", "D3", "D4", "D5", "D8"):
        if f"## {d}." not in reg:
            bad.append(f"{d} is enforced here but absent from docs/defects.md")

    print(f"table: {h} hypothesis rows, {e} exploratory rows")
    if bad:
        for b in bad:
            print("FAIL: " + b)
        return 1
    print("PASS: every semantic check holds")
    return 0


if __name__ == "__main__":
    sys.exit(main())
