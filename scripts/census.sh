#!/bin/bash
# Instruction census of the titan-spsc ring operations.
# Emits the library assembly and counts mnemonics per census symbol, then
# lists each function. Usage: scripts/census.sh [--list]
# Honours RUSTFLAGS and CARGO_TARGET_DIR, so a build with different code
# layout flags can be censused without disturbing the default target.
set -eu
cd "$(dirname "$0")/.."
T="${CARGO_TARGET_DIR:-target}"
rm -f "$T"/release/deps/titan_spsc-*.s
# Cargo skips rustc when nothing changed, and then no assembly is written.
touch crates/titan-spsc/src/census.rs
cargo rustc --release -q -p titan-spsc --lib -- --emit asm
S=$(ls -t "$T"/release/deps/titan_spsc-*.s 2>/dev/null | head -1 || true)
[ -n "$S" ] || { echo "census: no assembly produced" >&2; exit 1; }
# A function ends at .cfi_endproc or at the next global label. Local labels
# (L... on Mach-O, .L... on ELF) do not end it; Mach-O output built with
# panic=abort carries no CFI directives.
AWK='
/^(_|[A-KM-Za-z])[A-Za-z0-9_$.]*:/ { f = ""; if ($1 ~ /^_?titan_census_/) { f = $1; sub(":", "", f); sub("^_", "", f) } next }
f && /\.cfi_endproc/ { f = ""; next }
f && /^[ \t]+[a-z]/ { if (mode == "list") print f "\t" $0; else c[f " " $1]++ }
END { if (mode != "list") for (k in c) print k, c[k] }'
awk -v mode=count "$AWK" "$S" | sort | awk '{a[$1]=a[$1]" "$2"="$3} END {for (k in a) print k":"a[k]}' | sort
if [ "${1:-}" = "--list" ]; then
  awk -v mode=list "$AWK" "$S"
fi
