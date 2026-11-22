#!/bin/bash
# Build the manuscript and refuse to produce a PDF containing a dash.
#
#   bash docs/paper/build.sh
#
# Two dash sources have to be dealt with and neither is in the prose:
#
#   IEEEtran.cls  puts a triple-hyphen ligature after "Abstract" and after
#                 "Index Terms". Handled in the preamble of main.tex with
#                 etoolbox \patchcmd, which errors out if a future IEEEtran
#                 stops matching rather than letting a dash through.
#
#   IEEEtran.bst  turns page ranges into the double-hyphen ligature and emits
#                 a six-hyphen ligature in place of a repeated author name.
#                 Those are generated into main.bbl every time bibtex runs,
#                 so they are rewritten here, after bibtex and before the
#                 final pdflatex passes.
#
# The class and style files are never edited. A modified IEEEtran.cls would be
# a format non-conformance that a reviewer or an automated format check would
# be right to flag.
#
# main.bbl is committed on purpose: arXiv runs neither bibtex nor biber on an
# uploaded .bib, so the tarball has to carry a prebuilt .bbl, and the one it
# carries must be the rewritten one.
set -u
cd "$(dirname "$0")" || exit 1

command -v pdflatex >/dev/null 2>&1 || { echo "FAIL: pdflatex not installed"; exit 1; }
command -v bibtex   >/dev/null 2>&1 || { echo "FAIL: bibtex not installed"; exit 1; }
kpsewhich IEEEtran.cls >/dev/null 2>&1 || {
  echo "FAIL: IEEEtran.cls not found."
  echo "  Debian or Ubuntu: sudo apt install texlive-publishers"
  echo "  macOS, full distribution: brew install --cask mactex-no-gui"
  echo "  macOS, BasicTeX: sudo tlmgr install ieeetran"
  exit 1
}

# IEEEtran sets \texttt in Courier and its headings in Helvetica, and BasicTeX
# carries Times alone. Without those metrics pass 1 dies on a missing TFM, which
# reads as a corrupt document rather than a missing font. Checked here so the
# message names the fix. Found by installing BasicTeX on the measuring machine.
for TFM in pcrb7t pcrr7t phvb7t; do
  kpsewhich "$TFM.tfm" >/dev/null 2>&1 || {
    echo "FAIL: $TFM.tfm not found, so the Courier or Helvetica metrics that"
    echo "  IEEEtran needs are missing. BasicTeX ships Times and neither of"
    echo "  the other two."
    echo "  macOS: sudo tlmgr install collection-fontsrecommended"
    echo "  Debian or Ubuntu: sudo apt install texlive-fonts-recommended"
    exit 1
  }
done

echo "=== pass 1 ==="
pdflatex -interaction=nonstopmode -halt-on-error main.tex > build1.log 2>&1 || {
  echo "FAIL: pass 1 did not compile"; grep -E "^! " build1.log | head -5; exit 1; }

echo "=== bibtex ==="
bibtex main > buildbib.log 2>&1
if grep -qiE "^I couldn't open|error message" buildbib.log; then
  echo "FAIL: bibtex"; cat buildbib.log; exit 1
fi

echo "=== rewriting the dashes IEEEtran.bst generated into main.bbl ==="
BEFORE_PAGES="$(grep -c -- '--' main.bbl || true)"
# Page ranges: the double-hyphen ligature renders as an en dash.
#
# Written without -i on purpose. BSD sed, which is what macOS ships, requires an
# argument to -i, so `sed -i 's|a|b|' file` is read as a backup suffix of
# 's|a|b|' followed by a script of `file`, and dies with "invalid command code".
# GNU sed accepts both forms, so a Linux build cannot show the fault. The
# substitution below ran on macOS only because its suffix happened to be
# attached; the one after it did not run at all until this was fixed. See
# docs/defects.md D15.
sed 's/\([0-9]\)--\([0-9]\)/\1-\2/g' main.bbl > main.bbl.new && mv main.bbl.new main.bbl
# Repeated author: IEEEtran.bst writes a six-hyphen ligature, which renders as
# a pair of em dashes, in place of an author list identical to the one above.
# The names are restored instead. This is a bibliography of four entries, so
# the one case is handled explicitly rather than by a rule that could silently
# match something else.
# Currently a no-op, because the \IEEEtranBSTCTL entry in refs.bib sets
# CTLdash_repeated_names to "no" and the ligature is never generated. Kept, and
# kept working, so that removing the control entry does not silently reintroduce
# a dash. The guard below would catch it either way.
sed 's|^------, ``Analyzing the memory ordering models|L.~Wrenger, D.~T{\\"o}llner, and D.~Lohmann, ``Analyzing the memory ordering models|' main.bbl > main.bbl.new && mv main.bbl.new main.bbl
rm -f main.bbl.orig
AFTER_PAGES="$(grep -c -- '--' main.bbl || true)"
echo "  double-hyphen ligatures in main.bbl: ${BEFORE_PAGES} before, ${AFTER_PAGES} after"
if [ "${AFTER_PAGES}" != "0" ]; then
  echo "FAIL: a double-hyphen ligature survived in main.bbl:"
  grep -n -- '--' main.bbl
  exit 1
fi

echo "=== passes 2 and 3 ==="
pdflatex -interaction=nonstopmode main.tex > build2.log 2>&1
pdflatex -interaction=nonstopmode main.tex > build3.log 2>&1

echo "=== checks ==="
echo "  undefined citations : $(grep -c 'Citation.*undefined' build3.log)"
echo "  undefined references: $(grep -c 'Reference.*undefined' build3.log)"
echo "  overfull boxes      : $(grep -c '^Overfull' build3.log)"
if [ -f main.pdf ]; then
  echo "  pages               : $(pdfinfo main.pdf 2>/dev/null | awk '/^Pages/{print $2}')"
else
  echo "FAIL: no main.pdf produced"; exit 1
fi

echo "=== the check this script exists for: dashes in the RENDERED pdf ==="
if ! command -v pdftotext >/dev/null 2>&1; then
  echo "  pdftotext not installed, so the rendered output could not be checked."
  echo "  Debian or Ubuntu: sudo apt install poppler-utils"
  echo "  macOS: brew install poppler"
  echo "  NOT a pass. Verify before submitting."
  exit 0
fi
COUNT="$(pdftotext main.pdf - 2>/dev/null | python3 -c "
import sys
s = sys.stdin.read()
print(s.count('—') + s.count('–'))
")"
echo "  em dash plus en dash in the rendered pdf: ${COUNT}"
if [ "${COUNT}" != "0" ]; then
  echo "FAIL: the pdf still renders a dash. Lines:"
  pdftotext main.pdf - 2>/dev/null | python3 -c "
import sys
for ln in sys.stdin.read().split('\n'):
    if '—' in ln or '–' in ln:
        print('   ', ln[:120])
"
  exit 1
fi
echo "  PASS: zero dashes rendered"


echo "=== fonts in the rendered pdf: Type 3 is rejected by arXiv and PDF eXpress ==="
if command -v pdffonts >/dev/null 2>&1; then
  pdffonts main.pdf | sed 's/^/  /'
  if pdffonts main.pdf | awk 'NR>2' | grep -q 'Type 3'; then
    echo "  FAIL: a Type 3 font is embedded, which arXiv and IEEE PDF eXpress reject"
    exit 1
  fi
  echo "  PASS: no Type 3 font in the document"
else
  echo "  pdffonts is absent, so the font type cannot be verified here"
fi

rm -f build1.log build2.log build3.log buildbib.log
