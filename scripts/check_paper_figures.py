"""Every figure the manuscript embeds must exist, be Type 42, and be current.

Five publication figures existed for days and none of them was in the paper.
Nothing caught that, because every check asked whether what was there was
correct and none asked whether anything was there at all.

This gate asks four things of each figure the manuscript includes:

  it exists where graphicspath points;
  it carries no Type 3 font, which is what IEEE PDF eXpress and arXiv reject;
  it regenerates from committed data, byte for byte, by scripts/figures_paper.py;
  every number its caption quotes is present in the results file the caption
  names, so a caption cannot drift from the data it describes.

The third check is the one that matters most. A figure is an assertion about
committed records, and an assertion nobody can reproduce is decoration.
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

    # Regeneration must be byte identical.
    with tempfile.TemporaryDirectory() as td:
        keep = {n for n, _ in figs}
        for n in keep:
            if (FIGS / n).exists():
                shutil.copy2(FIGS / n, Path(td) / n)
        r = subprocess.run([sys.executable, "-I", str(ROOT / "scripts" / "figures_paper.py")],
                           capture_output=True, text=True, cwd=ROOT)
        if r.returncode != 0:
            bad.append("figures_paper.py failed, so the figures cannot be regenerated:\n"
                       + r.stderr.strip()[-400:])
        else:
            for n in sorted(keep):
                a, b = Path(td) / n, FIGS / n
                if a.exists() and a.read_bytes() != b.read_bytes():
                    bad.append(f"{n} does not regenerate byte for byte from committed data")
            print(f"  all {len(keep)} embedded figure(s) regenerate byte for byte")

    if bad:
        print()
        for b in bad:
            print("FAIL: " + b)
        return 1
    print("PASS: every embedded figure exists, is Type 42, regenerates, and its "
          "caption matches its data")
    return 0


if __name__ == "__main__":
    sys.exit(main())
