#!/bin/bash
# Instruction census of the measured loops of titan-fanout (write_pass_* and
# replay_pass_* for designs none, copy and snap) and titan-agg (measured_*
# and contend_* for designs local, faa, cas and shard; one named function
# per design, so the label names the design): instructions, barriers,
# ordered accesses and atomic read-modify-writes per loop function, with the
# entry address of each.
# The release profile strips symbols, so unstripped copies (same code) are
# built in target/census. Usage: scripts/fanout_census.sh
set -eu
cd "$(dirname "$0")/.."
CARGO_PROFILE_RELEASE_STRIP=false CARGO_TARGET_DIR=target/census \
  cargo build --release --locked -q -p titan-itch --bin titan-fanout --bin titan-agg
# Disassembly as "symbol:" lines followed by "address<TAB>mnemonic<TAB>operands".
dis() {
  if command -v otool >/dev/null 2>&1; then
    otool -tvV "$1"
  else
    objdump -d --no-show-raw-insn "$1" | awk '
      /^[0-9a-f]+ <.*>:$/ { s = $2; gsub(/[<>:]/, "", s); print s ":"; next }
      /^ *[0-9a-f]+:\t/ { sub(/^ */, ""); split($0, a, "\t"); split(a[2], b, " "); sub(":", "", a[1]); print a[1] "\t" b[1] "\t" b[2] }'
  fi
}
census() {
  echo "$1, loops matching /$2/:"
  dis "target/census/release/$1" | awk -v pat="$2" '
    /^[_A-Za-z].*:$/ {
      keep = ($0 ~ pat)
      if (keep) { k++; label[k] = $0; first[k] = "" }
      next
    }
    keep {
      if (first[k] == "") first[k] = $1
      m = $2; n[k]++
      if (m ~ /^dmb/) dmb[k]++
      if (m == "isb") isb[k]++
      if (m ~ /^stlr/) stlr[k]++
      if (m ~ /^ldar/) ldar[k]++
      if (m ~ /^ldapr/) ldapr[k]++
      if (m ~ /^(ldadd|stadd|ldclr|ldset|ldeor|swp)/) lse[k]++
      if (m ~ /^cas/) cas[k]++
      if (m ~ /^(ldxr|ldaxr|stxr|stlxr)/) llsc[k]++
      if (m ~ /^lock/ || m ~ /^xadd/ || m ~ /^cmpxchg/) x86rmw[k]++
    }
    END {
      for (i = 1; i <= k; i++)
        printf "  loop %d: entry=%s instructions=%d dmb=%d isb=%d stlr=%d ldar=%d ldapr=%d lse_rmw=%d cas=%d ldxr_stxr=%d x86_rmw=%d  %s\n",
          i, first[i], n[i], dmb[i] + 0, isb[i] + 0, stlr[i] + 0, ldar[i] + 0, ldapr[i] + 0, lse[i] + 0, cas[i] + 0, llsc[i] + 0, x86rmw[i] + 0, label[i]
    }'
  echo "  barrier and atomic mnemonics with operands:"
  dis "target/census/release/$1" | awk -v pat="$2" '/^[_A-Za-z].*:$/ { keep = ($0 ~ pat); next }
    keep && $2 ~ /^(dmb|ldadd|stadd|swp|cas|ldxr|ldaxr|stxr|stlxr)/ { print "    " $2, $3 }' | sort | uniq -c
}
census titan-fanout 'titan_fanout[0-9]+(write|replay)_pass_(none|copy|snap)'
census titan-agg 'titan_agg[0-9]+(measured|contend)_(local|faa|cas|shard)'
