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

    # A bibliography entry nothing cites does not appear in the rendered
    # document under IEEEtran, so it is invisible dead weight that a reader of
    # the source will wonder about, and a citation key the manuscript uses and
    # the file lacks renders as a question mark. Both directions are checked.
    bib_path = ROOT / "docs" / "paper" / "refs.bib"
    if bib_path.exists():
        bib = bib_path.read_text(encoding="utf-8")
        all_keys = re.findall(r"@\w+\{(\w+),", bib)
        keys = set(all_keys)
        # A duplicate key is resolved silently by bibtex, which keeps one entry
        # and discards the other, so a citation can render details from an entry
        # nobody is looking at. refs.bib carried two Mytkowicz entries for a
        # while and the count of entries disagreed with the count of keys by
        # one, which is the only way it showed. See D38.
        if len(all_keys) != len(keys):
            from collections import Counter
            dups = sorted(k for k, n in Counter(all_keys).items() if n > 1)
            bad.append(f"refs.bib defines {dups} more than once; bibtex keeps one "
                       f"entry and discards the other without saying which")
        cited = {k.strip() for grp in re.findall(r"\\cite\{([^}]+)\}", tex)
                 for k in grp.split(",")}
        dangling = sorted(cited - keys)
        uncited = sorted(keys - cited)
        if dangling:
            bad.append(f"the manuscript cites {dangling}, absent from refs.bib")
        if uncited:
            bad.append(f"refs.bib carries {uncited}, which nothing cites, so they "
                       f"will not appear in the rendered bibliography")
        if not dangling and not uncited:
            print(f"bibliography        : {len(keys)} entries, every one cited, "
                  f"every citation present")

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

    # The README is the front page and nothing checked a single number in it
    # against the repository it describes. It claimed a register of 44 defects
    # while the register held 49, and it described the paper's second
    # contribution as seven thresholds while the paper's own title said eight,
    # because D46 raised the taxonomy and the front page was not revisited.
    # A reader met the wrong count before reaching the paper, and a third party
    # quoted the stale 44 back to us. D50. Both counts are now derived from the
    # files that own them and the README must agree.
    readme = (ROOT / "README.md").read_text(encoding="utf-8")
    closed = reg.count("**CLOSED")
    stated = re.findall(r"register of\s+([\w-]+)\s+defects", readme)
    if not stated:
        bad.append("README no longer states the size of the defect register; "
                   "this check exists to keep that number true")
    for s in stated:
        if s != str(closed):
            bad.append(f"README says 'register of {s} defects' but "
                       f"docs/defects.md holds {closed} closed entries")

    # The README told a reader to expect 74 tests, 73 on macOS. The tree holds
    # 102 test attributes and a run on the target machine reported 101 passed
    # and 1 ignored, which is the same 102. The figure was written once and the
    # suite grew by a third underneath it. Nothing checked it because checking
    # it looked like it needed a cargo run; it does not, because the attributes
    # are countable in the source. D54.
    tests = 0
    for rs in list(ROOT.glob("crates/**/*.rs")) + list(ROOT.glob("tests/**/*.rs")):
        tests += len(re.findall(r"^\s*#\[test\]", rs.read_text(encoding="utf-8",
                                                                errors="replace"), re.M))
    claimed = re.findall(r"#\s*(\d+)\s+tests", readme)
    if not claimed:
        bad.append("README no longer states how many tests there are")
    for c in claimed:
        if int(c) != tests:
            bad.append(f"README says {c} tests; the tree holds {tests} #[test] attributes")

    # The conclusion of both manuscripts said five pre-registered thresholds
    # while the title and the discussion said eight. The taxonomy grew from
    # five to seven to eight over the study and the closing paragraph was never
    # revisited, so the last thing a reviewer reads contradicted the title of
    # the paper, in a document already archived under a DOI. The README check
    # added at D50 compared the README to the title and never compared a
    # manuscript to itself. D58.
    for texf in sorted((ROOT / "docs" / "paper").glob("*.tex")):
        body = re.sub(r"\s+", " ", texf.read_text(encoding="utf-8", errors="replace"))
        tm = re.search(r"and (\w+) Pre-Registered Thresholds", body)
        if not tm:
            continue
        want = tm.group(1).lower()
        for n in re.findall(r"(\w+)\s+(?:pre-registered|registered)\s+thresholds", body, re.I):
            if n.lower() != want:
                bad.append(f"{texf.name} says '{n} thresholds' in its body but "
                           f"'{tm.group(1)}' in its title")

    # Only main.tex was ever checked. While compressing the ISPASS variant a
    # citation key was written from memory, hoefler2015scientific, which does
    # not exist in refs.bib and never did: the paper has no Hoefler entry. It
    # reached a compiled PDF and was caught by a LaTeX warning rather than by
    # anything here, because ispass.tex is not a gated file. A fabricated
    # citation is the one error this project cannot afford, so every .tex in
    # docs/paper is now checked, not the one that happens to be the main
    # document. D56.
    bibkeys = set(re.findall(r"^@\w+\{([^,]+),",
                             (ROOT / "docs" / "paper" / "refs.bib").read_text(encoding="utf-8"),
                             re.M))
    for texf in sorted((ROOT / "docs" / "paper").glob("*.tex")):
        used = set()
        for m in re.findall(r"\\cite[tp]?\{([^}]*)\}",
                            texf.read_text(encoding="utf-8", errors="replace")):
            used |= {c.strip() for c in m.split(",")}
        for k in sorted(used - bibkeys):
            bad.append(f"{texf.name} cites {k}, which is not in refs.bib")

    # Eight binaries existed that the README named nowhere, titan-barrier among
    # them. That is the binary the headline result comes from, so the Reproduce
    # section carried no command for the paper's central claim: a reader could
    # not rerun it from the documentation, in a project whose argument is that
    # its results can be rerun. D52. Every binary in the tree must now appear in
    # the README, so a new one cannot be added and left undocumented.
    bins = set()
    for cargo in ROOT.glob("crates/*/Cargo.toml"):
        for m in re.findall(r'^\s*name\s*=\s*"(titan-[a-z0-9]+)"',
                            cargo.read_text(encoding="utf-8"), re.M):
            bins.add(m)
    for b in ROOT.glob("crates/*/src/bin/titan-*.rs"):
        bins.add(b.stem)
    missing = sorted(b for b in bins if b not in readme)
    if missing:
        bad.append("binaries that exist but the README never names: "
                   + ", ".join(missing))

    # D50 gated the two phrasings in the body and not the paper's title quoted
    # three lines above them, so the README went on announcing the paper under
    # a title the paper does not have, through the commit that was supposed to
    # fix exactly this. The title is the first line of the README a reader
    # takes seriously and it was the one line still wrong. D51. The whole
    # title is now compared, not a count inside it, so no future edit to it
    # can drift the front page without this failing.
    tt = re.search(r"\\title\{(.+?)\}\s*\n\s*\n", raw, re.S)
    if not tt:
        bad.append("main.tex has no \\title this check can read")
    else:
        want = re.sub(r"\s+", " ", tt.group(1).replace("\\\\", " ")).strip()
        got = re.sub(r"\s+", " ", readme)
        if want not in got:
            bad.append(f"README does not quote the paper's title verbatim; "
                       f"main.tex says: {want}")

    title = re.search(r"and (\w+) Pre-Registered Thresholds", tex)
    if not title:
        bad.append("main.tex title no longer names the threshold count")
    else:
        want = title.group(1).lower()
        for phrase in (r"(\w+) of the thresholds", r"The (\w+) instances"):
            for s in re.findall(phrase, readme):
                if s.lower() != want:
                    bad.append(
                        f"README says '{s}' where the paper's title says "
                        f"'{title.group(1)}' thresholds; the two must agree")

    print(f"table: {h} hypothesis rows, {e} exploratory rows")
    if bad:
        for b in bad:
            print("FAIL: " + b)
        return 1
    print("PASS: every semantic check holds")
    return 0


if __name__ == "__main__":
    sys.exit(main())
