"""Instructions the manuscript names must be ones the compiler emits.

The manuscript said the acquire-release arm uses `stlr`, twice, and named no
acquire-side instruction at all. The census over the compiled library says the
producer path uses `stlr` and the consumer path uses `stlur`, and that the
acquire side is `ldapur` and `ldapr` with `ldar` on neither. The scaled and
unscaled encodings are different instructions, and the paper's central claim is
about the store side, so calling both `stlr` was wrong rather than shorthand.
That is D18.

The census script existed the whole time and was never read against the
manuscript. This gate reads it, against a census captured on the measuring
machine and committed, so the two cannot drift apart again.
"""

import re
import sys
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]
TEX = ROOT / "docs" / "paper" / "main.tex"
CENSUS = ROOT / "results" / "census-aarch64.txt"

# The census covers the titan-spsc ring and nothing else, so the gate polices
# only the mnemonics that ring can emit. The acquire and release forms are the
# strict set: D18 was a claim about those, and a manuscript naming one the
# census does not contain is wrong about codegen.
#
# Everything else the paper mentions is reported but never used to fail. `isb
# sy` is the clearest reason why: it is real, it is named correctly, and it
# lives in the counter bracketing in titan-platform, which this census does not
# cover. Failing on it would be the gate asserting beyond its evidence, which
# is the fault it exists to prevent. Caught in rehearsal, before it stopped a
# run on the measuring machine.
WATCHED_STRICT = {"ldar", "ldapr", "ldapur", "ldaxr", "stlr", "stlur", "stlxr"}
WATCHED_SOFT = {"dmb", "dsb", "isb", "wfe", "sevl", "ldxr"}
WATCHED = WATCHED_STRICT | WATCHED_SOFT


def main() -> int:
    if not CENSUS.exists():
        print(f"SKIP: {CENSUS.name} is not committed yet, so manuscript "
              f"instruction claims cannot be checked against a census")
        return 0
    census = CENSUS.read_text(encoding="utf-8")

    # A census taken on a non-AArch64 host carries x86 mnemonics, and comparing
    # the manuscript against it would accuse the paper of naming instructions
    # that "do not exist" when the truth is that the census is from the wrong
    # machine. That is the D11 and D15 shape, a tool verified where it does not
    # run, and it was caught here in rehearsal rather than on the M4. The gate
    # states which it is rather than guessing.
    AARCH64_MARKERS = ("ldp", "stp", "b.ne", "cbnz", "ret", "ldr", "str")
    X86_MARKERS = ("movq", "retq", "jne", "cmpq", "leaq")
    arm = sum(1 for m in AARCH64_MARKERS if re.search(rf"\b{m}\b", census))
    x86 = sum(1 for m in X86_MARKERS if re.search(rf"\b{m}\b", census))
    if x86 > arm:
        print(f"SKIP: {CENSUS.name} is not from an AArch64 build "
              f"({x86} x86 markers against {arm} AArch64), so the manuscript's "
              f"instruction claims cannot be checked against it here")
        return 0

    emitted = {w for w in WATCHED if re.search(rf"\b{w}\b", census)}
    tex = TEX.read_text(encoding="utf-8")
    named = {w for w in WATCHED
             if re.search(rf"\\texttt\{{{w}(\s+ish)?\}}", tex) or
             re.search(rf"\\texttt\{{{w}\}}", tex)}

    print(f"census    : {CENSUS.name}, {len(emitted)} watched mnemonic(s) emitted")
    print(f"manuscript: {len(named)} watched mnemonic(s) named")
    bad = []
    for w in sorted(named):
        if w not in emitted:
            if w in WATCHED_STRICT:
                bad.append(f"the manuscript names {w} but this census shows the "
                           f"compiler never emits it in the ring")
            else:
                print(f"    note     {w} named but outside this census's scope, "
                      f"not treated as a fault")
    for w in sorted(emitted):
        print(f"    emitted  {w}{'   (named in the paper)' if w in named else ''}")

    # The store side is the claim, so wherever the paper names one release form
    # it must name the other nearby. Requiring only that both appear somewhere
    # in the document is too weak: the first version of this check passed a
    # manuscript whose abstract said "stlr on both paths" purely because the
    # Method still mentioned stlur twenty paragraphs away. Proximity is what
    # makes the attribution correct, so proximity is what is checked.
    if "stlr" in emitted and "stlur" in emitted:
        flat = re.sub(r"\s+", " ", tex)
        WINDOW = 400
        for m in re.finditer(r"\\texttt\{stlr\}", flat):
            lo = max(0, m.start() - WINDOW)
            near = flat[lo:m.end() + WINDOW]
            if "stlur" not in near:
                ctx = flat[max(0, m.start() - 70):m.start() + 70]
                bad.append("the manuscript names stlr without naming stlur within "
                           f"{WINDOW} characters, and the census emits both, so the "
                           f"store side is attributed to one form where there are "
                           f"two: ...{ctx.strip()}...")

    if bad:
        print()
        for b in bad:
            print("FAIL: " + b)
        return 1
    print("PASS: every instruction the manuscript names is one the compiler emits")
    return 0


if __name__ == "__main__":
    sys.exit(main())
