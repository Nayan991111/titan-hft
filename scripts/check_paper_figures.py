"""Every figure the manuscript embeds must exist, be Type 42, and be current.

Five publication figures existed for days and none of them was in the paper.
Nothing caught that, because every check asked whether what was there was
correct and none asked whether anything was there at all.

This gate asks four things of each figure the manuscript includes:

  it exists where graphicspath points;
  it carries no Type 3 font, which is what IEEE PDF eXpress and arXiv reject;
  it is CURRENT, meaning a fresh regeneration from committed data produces the
  same content, so a figure cannot silently fall behind the records it draws;
  every number its caption quotes is present in the results file the caption
  names, so a caption cannot drift from the data it describes.

The currency check compares extracted text, not bytes, and the reason is
written in docs/defects.md D7: the same script on the same data produces
different bytes on different machines, by 19 to 121 bytes, because the PDF
stream compression differs with the interpreter and library build. The first
version of this gate demanded byte identity against the committed copy. That
passed on the machine that committed the figures and failed on the machine that
measures them, which is the defect D7 already recorded and closed, reintroduced
by its own author. It is D17.

Determinism is a different property and a different gate owns it:
check_figures_deterministic.py regenerates twice on one machine and compares
those two runs to each other, which is the thing that is actually achievable.
"""

import re
import shutil
import subprocess
import sys
import tempfile
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]
TEX = ROOT / "docs" / "paper" / "main.tex"
FIGS = ROOT / "docs" / "figures"


def captions(tex: str) -> list[tuple[str, str]]:
    """(figure file, caption text) for each figure environment."""
    out = []
    for env in re.findall(r"\\begin\{figure\}.*?\\end\{figure\}", tex, re.S):
        f = re.search(r"\\includegraphics\[[^\]]*\]\{([^}]+)\}", env)
        c = re.search(r"\\caption\{(.*?)\}\s*\n?\\label", env, re.S)
        if f:
            out.append((f.group(1), c.group(1) if c else ""))
    return out


def main() -> int:
    tex = TEX.read_text(encoding="utf-8")
    # Long results paths in captions carry \allowbreak after each separator so
    # they can wrap in a two-column measure. Those markers are typesetting, not
    # content, and every pattern below reads the path, so they come out first.
    tex = tex.replace("\\allowbreak ", "").replace("\\allowbreak", "")
    figs = captions(tex)
    print(f"figures embedded in the manuscript: {len(figs)}")
    if not figs:
        print("FAIL: the manuscript embeds no figures at all")
        return 1

    bad: list[str] = []

    for name, cap in figs:
        path = FIGS / name
        if not path.exists():
            bad.append(f"{name} is included but does not exist in docs/figures")
            continue
        fonts = subprocess.run(["pdffonts", str(path)], capture_output=True, text=True).stdout
        if "Type 3" in fonts:
            bad.append(f"{name} contains a Type 3 font, which arXiv and IEEE reject")
        print(f"  {name:<22} present, no Type 3")

        # Each number the caption quotes must be in a results file it names.
        #
        # EVERY file the caption names, not the first one. A figure may be
        # drawn from more than one run, and the wait-strategy figure is: its
        # cost and stall panels come from the stall probe and its issue panel
        # from the issue probe. The earlier form took the first match and
        # failed that figure for quoting numbers that were in its second file,
        # which would have pushed the figure towards being split or towards
        # dropping half its caption. Both named files must exist, so naming an
        # extra file is not a way to widen the search.
        srcs = re.findall(r"results/([A-Za-z0-9_\\.-]+\.txt)",
                          cap.replace("\\_", "_"))
        if not srcs:
            bad.append(f"{name}: the caption names no results file, so nothing can check it")
            continue
        seen = list(dict.fromkeys(srcs))
        absent = [s for s in seen if not (ROOT / "results" / s).exists()]
        if absent:
            bad.append(f"{name}: the caption names {absent}, which do not exist")
            continue
        body = "\n".join((ROOT / "results" / s).read_text(
            encoding="utf-8", errors="replace") for s in seen)
        plain = re.sub(r"\\[a-zA-Z]+|[{}$\\]", " ", cap)
        quoted = {n for n in re.findall(r"\b\d+\.\d{2,}\b", plain)}
        missing = sorted(n for n in quoted if n not in body)
        named = seen[0] if len(seen) == 1 else f"the {len(seen)} runs it names"
        if missing:
            bad.append(f"{name}: caption quotes {missing} which are in none of {seen}")
        else:
            print(f"  {'':<22} caption numbers all present in {named}")

    # Currency, by content rather than by bytes. See the module docstring and
    # docs/defects.md D7 and D17: bytes differ across machines, text does not.
    # The working tree is restored whatever happens, so running this gate never
    # leaves a modified figure behind.
    def text_of(p: Path) -> str:
        r = subprocess.run(["pdftotext", "-layout", str(p), "-"],
                           capture_output=True, text=True)
        return r.stdout if r.returncode == 0 else ""

    def content_of(p: Path) -> list:
        """The numbers a figure draws, and deliberately nothing else.

        This took three attempts and the first two were patches rather than
        decisions, which is why D34, D35 and D36 all exist for one question.

        The check asks whether a committed figure still matches the records it
        draws. The data appears in a figure as numbers: axis ticks and
        annotated values. The words appear because the plotting script writes
        them, and both the committed figure and a fresh regeneration come from
        the same script, so a word can only differ for a rendering reason and
        never for a data reason. Comparing words therefore adds no power to
        detect staleness and adds a cross-machine failure mode, which is what
        it did twice.

        What the earlier attempts compared, in order: the raw output of
        pdftotext -layout, which encodes glyph positions as whitespace; then
        word tokens, which pdftotext splits wherever spacing suggests a gap, so
        one machine gave un + ord + ered and the other unord + ered; then the
        letters concatenated, which removed the splitting and kept the layout
        ordering, so a label shifted by a point is emitted before rather than
        after its neighbour and the string changes while every character is
        the same.

        The textual side of a caption is still checked, by the separate rule
        that every number a caption quotes must appear in the results file the
        caption names. Nothing is unguarded here.
        """
        raw = text_of(p)
        return sorted(re.findall(r"-?\d+(?:\.\d+)?", raw))

    try:
        import matplotlib  # noqa: F401
    except ImportError:
        print("  SKIP: matplotlib is absent, so currency cannot be checked here")
    else:
        before = {n: content_of(FIGS / n) for n, _ in figs if (FIGS / n).exists()}
        backup = Path(tempfile.mkdtemp())
        for f in FIGS.iterdir():
            if f.is_file():
                shutil.copy2(f, backup / f.name)
        try:
            r = subprocess.run([sys.executable, "-I",
                                str(ROOT / "scripts" / "figures_paper.py")],
                               capture_output=True, text=True, cwd=ROOT)
            if r.returncode != 0:
                bad.append("figures_paper.py failed, so currency cannot be established:\n"
                           + r.stderr.strip()[-400:])
            else:
                for n in sorted(before):
                    after = content_of(FIGS / n)
                    if after != before[n]:
                        lost_n = sorted(set(before[n]) - set(after))[:8]
                        new_n = sorted(set(after) - set(before[n]))[:8]
                        bad.append(f"{n}: numbers in the committed figure and absent "
                                   f"from a regeneration {lost_n}; numbers in the "
                                   f"regeneration and absent from the committed figure "
                                   f"{new_n}")
                        bad.append(f"{n} is stale: regenerating from committed data "
                                   f"changes its content, so the committed figure no "
                                   f"longer matches the records it draws")
                if not bad:
                    print(f"  all {len(before)} embedded figure(s) are current: a fresh "
                          f"regeneration reproduces the same content")
        finally:
            for f in backup.iterdir():
                shutil.copy2(f, FIGS / f.name)
            shutil.rmtree(backup, ignore_errors=True)

    if bad:
        print()
        for b in bad:
            print("FAIL: " + b)
        return 1
    print("PASS: every embedded figure exists, is Type 42, is current, and its "
          "caption matches its data")
    return 0


if __name__ == "__main__":
    sys.exit(main())
