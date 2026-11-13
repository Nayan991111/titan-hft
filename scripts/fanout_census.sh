#!/bin/bash
# Instruction census of the six titan-fanout writer loops (write_pass and
# replay_pass for designs none, copy and snap): instructions, barriers and
# ordered accesses per loop, and the entry address of each loop function.
# The release profile strips symbols, so an unstripped copy (same code) is
# built in target/census. Usage: scripts/fanout_census.sh
set -eu
cd "$(dirname "$0")/.."
CARGO_PROFILE_RELEASE_STRIP=false CARGO_TARGET_DIR=target/census \
  cargo build --release --locked -q -p titan-itch --bin titan-fanout
BIN=target/census/release/titan-fanout
# Disassembly as "symbol:" lines followed by "address<TAB>mnemonic<TAB>operands".
if command -v otool >/dev/null 2>&1; then
  dis() { otool -tvV "$BIN"; }
else
  dis() {
    objdump -d --no-show-raw-insn "$BIN" | awk '
      /^[0-9a-f]+ <.*>:$/ { s = $2; gsub(/[<>:]/, "", s); print s ":"; next }
      /^ *[0-9a-f]+:\t/ { sub(/^ */, ""); split($0, a, "\t"); split(a[2], b, " "); sub(":", "", a[1]); print a[1] "\t" b[1] "\t" b[2] }'
  }
fi
dis | awk '
  /^[_A-Za-z].*:$/ {
    keep = ($0 ~ /write_pass|replay_pass/)
    if (keep) { k++; label[k] = $0; first[k] = "" }
    next
  }
  keep {
    if (first[k] == "") first[k] = $1
    m = $2; n[k]++
    if (m ~ /^dmb/) dmb[k]++
    if (m == "isb") isb[k]++
    if (m == "stlr" || m == "stlrb" || m == "stlrh") stlr[k]++
    if (m == "ldar" || m == "ldarb" || m == "ldarh") ldar[k]++
    if (m == "ldapr" || m == "ldaprb" || m == "ldaprh") ldapr[k]++
  }
  END {
    for (i = 1; i <= k; i++)
      printf "loop %d: entry=%s instructions=%d dmb=%d isb=%d stlr=%d ldar=%d ldapr=%d  %s\n",
        i, first[i], n[i], dmb[i] + 0, isb[i] + 0, stlr[i] + 0, ldar[i] + 0, ldapr[i] + 0, label[i]
  }'
echo "barrier operands in the six loops:"
dis | awk '/^[_A-Za-z].*:$/ { keep = ($0 ~ /write_pass|replay_pass/); next } keep && $2 ~ /^dmb/ { print "  " $2, $3 }' | sort | uniq -c
