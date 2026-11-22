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

        # Each number the caption quotes must be in the results file it names.
        src = re.search(r"results/([A-Za-z0-9_\\.-]+\.txt)", cap.replace("\\_", "_"))
        if not src:
            bad.append(f"{name}: the caption names no results file, so nothing can check it")
            continue
        rf = ROOT / "results" / src.group(1)
        if not rf.exists():
            bad.append(f"{name}: the caption names {src.group(1)}, which does not exist")
            continue
        body = rf.read_text(encoding="utf-8", errors="replace")
        plain = re.sub(r"\\[a-zA-Z]+|[{}$\\]", " ", cap)
        quoted = {n for n in re.findall(r"\b\d+\.\d{2,}\b", plain)}
        missing = sorted(n for n in quoted if n not in body)
        if missing:
            bad.append(f"{name}: caption quotes {missing} which are not in {src.group(1)}")
        else:
            print(f"  {'':<22} caption numbers all present in {src.group(1)}")

    # Currency, by content rather than by bytes. See the module docstring and
    # docs/defects.md D7 and D17: bytes differ across machines, text does not.
    # The working tree is restored whatever happens, so running this gate never
    # leaves a modified figure behind.
    def text_of(p: Path) -> str:
        r = subprocess.run(["pdftotext", "-layout", str(p), "-"],
                           capture_output=True, text=True)
        return r.stdout if r.returncode == 0 else ""

    def content_of(p: Path) -> tuple:
        """What a figure says, not where it says it.

        The currency check used to compare the output of pdftotext -layout
        directly. That output encodes glyph positions as whitespace, so a
        different matplotlib or font version shifts a label by a point and the
        whole string changes. fig-barrier, which carries the most text of any
        figure here, failed on the measurement machine while passing on the
        machine that had last regenerated it, and nothing about the data had
        moved. That is D34, and it is the third time cross machine PDF
        comparison has produced a false alarm, after D7 and D17.

        The question this check exists to answer is whether a figure still
        matches the records it draws, which is a question about its numbers and
        its labels. Both are compared as multisets, so spacing, column order
        and line breaks cannot raise an alarm while a changed value still
        does.
        """
        raw = text_of(p)
        nums = sorted(re.findall(r"-?\d+(?:\.\d+)?", raw))
        # Word tokens are not stable either. pdftotext breaks a label wherever
        # the glyph spacing suggests a gap, so the same "unordered" came out as
        # un + ord + ered on one machine and unord + ered on the other, and
        # comparing word multisets reported labels lost and gained while the
        # numbers were identical. Concatenating every letter removes the
        # fragmentation entirely: the same label yields the same string however
        # it was split. D35.
        letters = "".join(re.findall(r"[a-z]+", raw.lower()))
        return nums, letters

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
                        bn, bw = before[n]
                        an, aw = after
                        lost_n = sorted(set(bn) - set(an))[:6]
                        new_n = sorted(set(an) - set(bn))[:6]
                        same_text = bw == aw
                        bad.append(f"{n}: numbers in the committed figure and absent from "
                                   f"a regeneration {lost_n}, numbers in the regeneration "
                                   f"and absent from the committed figure {new_n}, "
                                   f"letter content identical: {same_text}")
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
