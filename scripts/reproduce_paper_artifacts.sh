#!/bin/bash
# Reproduce the artifacts behind the paper, from a clean clone, in one command.
#
#   scripts/reproduce_paper_artifacts.sh [--quick] [--pmu] [--full]
#
#   --quick   small message counts and few epochs, a few minutes, for a
#             reviewer who wants to see that everything builds and runs
#   --full    the protocol the paper used: 30 independent launches on wall
#             power, tens of minutes
#   --pmu     also read the hardware performance counters, which needs root
#             on macOS and therefore asks for a password
#
# With neither --quick nor --full it runs --quick, because a first run should
# finish.
#
# What this script does NOT do. It does not download market data. The NASDAQ
# TotalView-ITCH sample is not redistributed with this repository and is not
# committed to it. Without it the real-feed arms are skipped and the synthetic
# arms still run; scripts/fetch_itch_sample.sh fetches a day from NASDAQ's own
# public server if you want the real-feed numbers.
#
# Hardware counters are read through macOS kperf and exist only on Apple
# silicon. On any other platform the counter arms are skipped and the
# wall-clock arms still run, so the suite is runnable anywhere while only the
# Apple silicon numbers are reproducible on Apple silicon.
set -u

MODE=quick
PMU=""
for a in "$@"; do
  case "$a" in
    --quick) MODE=quick ;;
    --full)  MODE=full ;;
    --pmu)   PMU=pmu ;;
    -h|--help) sed -n '2,28p' "$0"; exit 0 ;;
    *) echo "unknown argument: $a"; sed -n '2,28p' "$0"; exit 2 ;;
  esac
done

cd "$(dirname "$0")/.."
OUT="artifacts/$(date +%Y%m%d_%H%M%S)"
mkdir -p "$OUT"
RAW="$OUT/raw.txt"
: > "$RAW"
h() { echo; echo "===== $1"; }
have() { command -v "$1" >/dev/null 2>&1; }

h "environment"
{
  echo "date:      $(date -u +%Y-%m-%dT%H:%M:%SZ)"
  echo "uname:     $(uname -smr)"
  echo "rustc:     $(rustc -V 2>/dev/null || echo absent)"
  echo "cargo:     $(cargo -V 2>/dev/null || echo absent)"
  echo "commit:    $(git rev-parse HEAD 2>/dev/null || echo 'not a git checkout')"
  echo "dirty:     $(test -n "$(git status --porcelain 2>/dev/null)" && echo yes || echo no)"
  echo "mode:      $MODE"
  echo "pmu:       ${PMU:-off}"
} | tee "$OUT/environment.txt"

have cargo || { echo "cargo not found. Install Rust from https://rustup.rs and run this again."; exit 1; }

h "build"
cargo build --release 2>&1 | tail -3

h "tests"
cargo test --release -- --test-threads=1 > "$OUT/tests.txt" 2>&1
TESTS=$(grep -cE '^test .* ok$' "$OUT/tests.txt" || true)
FAILED=$(grep -cE '^test result: FAILED' "$OUT/tests.txt" || true)
echo "passed: $TESTS   failed suites: $FAILED   full log: $OUT/tests.txt"
[ "${FAILED:-0}" = "0" ] || { echo "tests failed; stopping"; exit 1; }

h "lint"
if cargo clippy --release --workspace --all-targets > "$OUT/clippy.txt" 2>&1; then
  echo "clippy warnings: $(grep -cE '^(warning|error)' "$OUT/clippy.txt" || true)"
else
  echo "clippy unavailable or failed; see $OUT/clippy.txt"
fi

h "instruction census: which aarch64 atomics the compiler actually emitted"
# The paper's claim about memory ordering rests on AcqRel lowering to ldapr,
# SeqCst to ldar, and Fenced to dmb ish. That holds only when the build
# targets armv8.3-a or later, so it is checked and not assumed.
if scripts/census.sh > "$OUT/census.txt" 2>&1; then
  grep -oE "(ldapr|ldar|stlr|dmb|isb)=[0-9]+" "$OUT/census.txt" | sort | uniq -c | sort -rn | head -12
  if grep -q "ldapr" "$OUT/census.txt"; then
    echo "ldapr present: the RCpc acquire form is in the build"
  else
    echo "WARNING: no ldapr in the census. On this build AcqRel and SeqCst may both"
    echo "         be lowering to ldar, which would collapse the ordering comparison."
    echo "         Check that the target is armv8.3-a or later (target-cpu=native on"
    echo "         Apple silicon selects one)."
  fi
else
  echo "census skipped (not an aarch64 build host); see $OUT/census.txt"
fi

case "$MODE" in
  quick) PP_N=20000;  PP_E=2;  ST_N=100000;  ST_E=2;  LAUNCHES=3;  BOOK_LIMIT=200000 ;;
  full)  PP_N=200000; PP_E=3;  ST_N=200000;  ST_E=3;  LAUNCHES=30; BOOK_LIMIT=0 ;;
esac

F=data/20190730.BX_ITCH_50
if [ -f "$F" ]; then
  echo
  echo "real feed present: $F"
  REAL=1
else
  echo
  echo "real feed absent. The real-feed arms are skipped. To fetch one day from"
  echo "NASDAQ's public server:  scripts/fetch_itch_sample.sh 20190730 bx"
  REAL=0
fi

h "cross-core handoff and streaming ring, $LAUNCHES independent launches"
# The launch is the unit of observation: epochs inside one process share a
# code layout and an allocator state, and the padding question depends on
# exactly that.
for i in $(seq 1 "$LAUNCHES"); do
  for q in ui bg; do
    ./target/release/titan-pingpong "$PP_N" "$PP_E" "$q" $PMU 2>&1 | sed "s/^pp /pp launch=$i /" >> "$RAW"
    # The pricing kernel (H65) and the path engine (H66). Both are pure
    # compute and cost a few milliseconds a launch, so they ride the same
    # loop rather than needing one of their own. The QoS class is part of
    # each case's key, so a plain launch number is enough to pair within.
    ./target/release/titan-greeks "$PP_E" "$q" $PMU 2>&1 | sed "s/\$/ launch=$i/" >> "$RAW"
    ./target/release/titan-paths  "$PP_E" "$q" $PMU 2>&1 | sed "s/\$/ launch=$i/" >> "$RAW"
  done
  ./target/release/titan-stream "$ST_N" "$ST_E" $PMU 2>&1 | sed "s/^st /st launch=$i /" >> "$RAW"
  # H69, the barrier-placement cross. It is the same ring at the same message
  # count as titan-stream, so it belongs in the same loop: a separate loop
  # would compare arms measured under a different set of launches, which is
  # the one thing the independent-launch protocol exists to prevent.
  ./target/release/titan-barrier "$ST_N" "$ST_E" $PMU 2>&1 | sed "s/^bd /bd launch=$i /" >> "$RAW"
  printf "%s " "$i"
done
echo

if [ "$REAL" = "1" ]; then
  h "order book on the real feed"
  LIM=""
  [ "$BOOK_LIMIT" != "0" ] && LIM="limit=$BOOK_LIMIT"
  ./target/release/titan-heap  file=$F $LIM epochs="$PP_E" $PMU 2>&1 | tee -a "$RAW" | grep -E "^heap_(config|life) " || true
  ./target/release/titan-queue file=$F $LIM epochs="$PP_E" $PMU 2>&1 | tee -a "$RAW" | grep -E "^queue_(config|cancel|chain) " || true
  ./target/release/titan-walk  file=$F $LIM epochs="$PP_E" $PMU 2>&1 | tee -a "$RAW" | grep -E "^walk_(config|cost|cond|depth) " || true
fi

h "statistics"
python3 scripts/launches.py "$RAW" > "$OUT/launches.txt" 2>&1 || echo "(launches.py reported a problem; see the file)"
python3 scripts/analyze.py  "$RAW" > "$OUT/analyze.txt"  2>&1 || echo "(analyze.py reported a problem; see the file)"
sed -n '1,40p' "$OUT/launches.txt"

h "figures"
if python3 scripts/figures.py > "$OUT/figures.txt" 2>&1; then
  echo "wrote $(ls docs/figures/*.svg 2>/dev/null | wc -l | tr -d ' ') SVG figures to docs/figures/"
else
  echo "figures skipped; see $OUT/figures.txt"
fi

h "done"
echo "raw lines:    $RAW"
echo "statistics:   $OUT/launches.txt and $OUT/analyze.txt"
echo "environment:  $OUT/environment.txt"
echo
echo "The verdicts these numbers are judged against are pre-registered in"
echo "docs/experiments.md, written before each run and never edited afterwards."
echo "Every row there cites the results file holding its data, except two that"
echo "record findings about the counter interface itself, where the row is the"
echo "record. Rows flagged PRELIMINARY PROTOCOL were measured with three to"
echo "five epochs inside one process rather than over independent launches,"
echo "and are quotable only with that protocol stated alongside them."
if [ "$MODE" = "quick" ]; then
  echo
  echo "This was --quick, so the counts are small and the numbers will not match"
  echo "the paper. Use --full --pmu on an Apple silicon machine on wall power to"
  echo "reproduce the published figures."
fi
