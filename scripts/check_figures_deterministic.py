"""A figure must be a function of its data, not of when or where it was made.

Two different properties get confused here and only one of them is
achievable.

Same-machine determinism: regenerating a figure twice on one machine from the
same committed data gives the same bytes. This is the property that stops a
figure drifting away from the data it claims to show, it is the one that
matters, and it is what this file checks.

Cross-machine byte identity: a figure regenerated on any machine matches the
committed bytes. This is NOT achievable. Matplotlib embeds a subset of
whatever font the host resolves and the host's FreeType writes that subset,
so the same data produced files 19 to 121 bytes apart between macOS and Linux
during the 2026-11-20 audit, with identical content. A check that demanded it
would fail on every machine except the one that made the commit. See
docs/defects.md D7.

The committed figures are the ones produced on the measurement machine.

Usage:
    python3 -I scripts/check_figures_deterministic.py
"""

import hashlib
import shutil
import subprocess
import sys
import tempfile
from pathlib import Path

ROOT = Path(__file__).resolve().parent.parent
FIGS = ROOT / "docs" / "figures"
GENERATORS = ["figures_paper.py", "figures.py", "figures_readme.py", "figures_abstract.py"]


def digests():
    return {p.name: hashlib.sha256(p.read_bytes()).hexdigest()
            for p in sorted(FIGS.iterdir()) if p.is_file()}


def regenerate():
    for g in GENERATORS:
        script = ROOT / "scripts" / g
        if not script.exists():
            continue
        r = subprocess.run([sys.executable, "-I", str(script)],
                           cwd=ROOT, capture_output=True, text=True)
        if r.returncode != 0:
            return f"{g} exited {r.returncode}: {r.stderr.strip()[:200]}"
    return None


def main():
    try:
        import matplotlib  # noqa: F401
    except ImportError:
        print("SKIP: matplotlib is not installed, so the PDF figures cannot be checked here")
        return 0

    backup = Path(tempfile.mkdtemp())
    for p in FIGS.iterdir():
        if p.is_file():
            shutil.copy2(p, backup / p.name)
    try:
        err = regenerate()
        if err:
            print("FAIL: " + err)
            return 1
        first = digests()
        err = regenerate()
        if err:
            print("FAIL: " + err)
            return 1
        second = digests()
    finally:
        for p in backup.iterdir():
            shutil.copy2(p, FIGS / p.name)
        shutil.rmtree(backup, ignore_errors=True)

    drift = sorted(k for k in set(first) | set(second) if first.get(k) != second.get(k))
    if drift:
        print(f"FAIL: {len(drift)} figure(s) differ between two regenerations on this machine:")
        for k in drift:
            print(f"  {k}")
        print("A figure that changes without its data changing cannot be trusted to show it.")
        return 1
    print(f"PASS: {len(first)} figures are identical across two regenerations on this machine")
    return 0


if __name__ == "__main__":
    sys.exit(main())
