#!/bin/bash
# titan_baremetal_x86.sh
#
# THE x86-64 CONTROL, ON REAL HARDWARE.
#
# Run this on any bare-metal x86-64 Linux machine: an old desktop, an old
# laptop, a college lab machine, or anything booted from a Linux USB stick.
# It does not need to be fast, modern, or a server. It needs to be four things,
# and it checks all four before measuring anything:
#
#   1. not a virtual machine        (a hypervisor can pin or hide the clock)
#   2. at least 3 usable cores      (producer, consumer, and one for the OS)
#   3. a visible cpufreq interface  (so the clock can be OBSERVED, which is the
#                                    single thing the container run could not do)
#   4. readable hardware counters   (perf, for cycles, so an effective clock can
#                                    be computed independently of cpufreq)
#
# Why this matters: the paper's run-length result says a conclusion reverses
# because the core clock tracks how long the core has been busy. That was
# measured on Apple silicon. The x86 control said the effect is absent, but it
# ran in a KVM container with NO cpufreq interface at all, so "absent" and
# "invisible" could not be told apart. This script tells them apart.
#
# Usage:
#   bash titan_baremetal_x86.sh
#
# It writes one results file and prints it. Paste that file back.
# Nothing is installed system-wide except, if you allow it, rustup into $HOME.

set -u
STAMP="$(date +%Y%m%d_%H%M)"
WORK="${HOME}/titan_baremetal_${STAMP}"
OUT="${HOME}/titan_baremetal_x86_${STAMP}.txt"
REPO_URL="https://github.com/Nayan991111/titan-hft.git"

say() { printf '%s\n' "$*" | tee -a "$OUT"; }
hr()  { say "--------------------------------------------------------------------"; }

: > "$OUT"
say "TITAN x86-64 BARE-METAL CONTROL"
say "started $(date -u '+%Y-%m-%d %H:%M:%S UTC')"
hr

# ---------------------------------------------------------------- preflight
say "=== PREFLIGHT. This aborts rather than recording a run it cannot read ==="
FATAL=0

VIRT="unknown"
if command -v systemd-detect-virt >/dev/null 2>&1; then
  VIRT="$(systemd-detect-virt 2>/dev/null || echo none)"
fi
HYPER="no"
grep -qm1 '^flags.*\bhypervisor\b' /proc/cpuinfo && HYPER="yes"
say "virtualization    : ${VIRT}"
say "hypervisor cpu flag: ${HYPER}"
if [ "$HYPER" = "yes" ] || { [ "$VIRT" != "none" ] && [ "$VIRT" != "unknown" ]; }; then
  say "  FATAL: this is virtualized. That is the exact weakness this run exists"
  say "         to remove, so recording it here would be pointless. Boot a Linux"
  say "         USB stick on real hardware, or use a physical machine."
  FATAL=1
fi

NCPU="$(getconf _NPROCESSORS_ONLN 2>/dev/null || echo 0)"
say "online cpus       : ${NCPU}"
if [ "$NCPU" -lt 3 ]; then
  say "  FATAL: fewer than 3 cores. The producer and consumer each need one and"
  say "         the operating system needs somewhere else to run, or preemption"
  say "         lands directly on the measured loop."
  FATAL=1
fi

CPUFREQ_DIR="/sys/devices/system/cpu/cpu0/cpufreq"
if [ -d "$CPUFREQ_DIR" ]; then
  say "cpufreq interface : present"
  say "  scaling_driver  : $(cat "$CPUFREQ_DIR/scaling_driver" 2>/dev/null || echo unreadable)"
  say "  scaling_governor: $(cat "$CPUFREQ_DIR/scaling_governor" 2>/dev/null || echo unreadable)"
  say "  min / max kHz   : $(cat "$CPUFREQ_DIR/cpuinfo_min_freq" 2>/dev/null || echo ?) / $(cat "$CPUFREQ_DIR/cpuinfo_max_freq" 2>/dev/null || echo ?)"
  HAVE_CPUFREQ=1
else
  say "cpufreq interface : ABSENT"
  say "  FATAL: without this the clock cannot be observed, which leaves this run"
  say "         with the identical defect as the container run it replaces."
  HAVE_CPUFREQ=0
  FATAL=1
fi

HAVE_PERF=0
if command -v perf >/dev/null 2>&1; then
  if perf stat -e cycles true >/dev/null 2>&1; then
    HAVE_PERF=1
    say "perf counters     : readable"
  else
    PARANOID="$(cat /proc/sys/kernel/perf_event_paranoid 2>/dev/null || echo ?)"
    say "perf counters     : present but blocked (perf_event_paranoid=${PARANOID})"
    say "  To allow, run:  sudo sysctl -w kernel.perf_event_paranoid=1"
    say "  Not fatal: cpufreq above still gives a clock reading."
  fi
else
  say "perf counters     : perf not installed"
  say "  Optional. Debian/Ubuntu: sudo apt install linux-tools-\$(uname -r)"
  say "  Fedora: sudo dnf install perf. Not fatal."
fi

say "cpu model         : $(grep -m1 '^model name' /proc/cpuinfo | cut -d: -f2- | sed 's/^ //')"
say "kernel            : $(uname -srm)"
say "distro            : $(. /etc/os-release 2>/dev/null && echo "$PRETTY_NAME" || echo unknown)"
say "L3 cache          : $(lscpu 2>/dev/null | awk -F: '/L3 cache/{gsub(/^ +/,"",$2); print $2}' || echo unknown)"
hr

if [ "$FATAL" = "1" ]; then
  say "ABORTED before measuring. Fix the FATAL items above and re-run."
  say "report: $OUT"
  exit 1
fi
say "preflight ok"
hr

# ---------------------------------------------------------------- toolchain
say "=== TOOLCHAIN ==="
if ! command -v cargo >/dev/null 2>&1; then
  [ -f "$HOME/.cargo/env" ] && . "$HOME/.cargo/env"
fi
if ! command -v cargo >/dev/null 2>&1; then
  say "cargo not found. Rust is required."
  say "Install it into your home directory only, nothing system-wide:"
  say "  curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh -s -- -y"
  say "  . \$HOME/.cargo/env"
  say "Then re-run this script."
  say "report: $OUT"
  exit 1
fi
say "rustc  : $(rustc --version)"
say "cargo  : $(cargo --version)"
command -v git >/dev/null 2>&1 || { say "FATAL: git not installed."; exit 1; }
say "git    : $(git --version)"
hr

# ---------------------------------------------------------------- build
say "=== FETCH AND BUILD ==="
mkdir -p "$WORK" && cd "$WORK" || { say "FATAL: cannot use $WORK"; exit 1; }
if ! git clone -q --depth 1 "$REPO_URL" repo 2>&1 | tee -a "$OUT"; then
  say "FATAL: clone failed. Check network."
  exit 1
fi
cd repo || exit 1
say "commit : $(git rev-parse HEAD)"
say "tree   : $(git rev-parse HEAD^{tree})"
say "building with target-cpu=native, which is what the paper's M4 runs use"
if ! RUSTFLAGS="-C target-cpu=native" cargo build --release 2>&1 | tail -3 | tee -a "$OUT"; then
  say "FATAL: build failed."
  exit 1
fi
for b in titan-barrier titan-stream; do
  [ -x "target/release/$b" ] || { say "FATAL: $b did not build"; exit 1; }
done
say "binaries: titan-barrier, titan-stream"
hr

# ------------------------------------------------- clock sampling helper
# Highest scaling_cur_freq across all cpus, in MHz. Sampled rather than
# averaged: the question is whether the clock MOVES, so the peak during a
# busy arm against the idle floor is the signal.
peak_mhz() {
  local m=0 v
  for f in /sys/devices/system/cpu/cpu*/cpufreq/scaling_cur_freq; do
    [ -r "$f" ] || continue
    v="$(cat "$f" 2>/dev/null || echo 0)"
    [ "$v" -gt "$m" ] 2>/dev/null && m="$v"
  done
  echo $(( m / 1000 ))
}

say "=== IDLE CLOCK BASELINE, 5 samples over 5 seconds ==="
IDLE=""
for i in 1 2 3 4 5; do
  IDLE="$IDLE $(peak_mhz)"
  sleep 1
done
say "idle peak MHz     :$IDLE"
hr

# ---------------------------------------------------------------- the runs
# Two message counts, because the whole point is run length. 200k is the short
# run where the M4 conclusion reverses; 10M is the long run.
run_set() {
  local bin="$1" n="$2" epochs="$3" tag="$4" launches="$5"
  local raw="$WORK/raw_${tag}.txt"
  : > "$raw"
  say "--- ${bin} n=${n} epochs=${epochs} launches=${launches} (${tag}) ---"
  local pre post cyc secs
  for i in $(seq 1 "$launches"); do
    pre="$(peak_mhz)"
    if [ "$HAVE_PERF" = "1" ]; then
      # cycles from the hardware counter give an effective clock that does not
      # depend on cpufreq being honest.
      perf stat -x, -e cycles -o "$WORK/perf_$$.txt" -- \
        ./target/release/"$bin" "$n" "$epochs" 2>/dev/null \
        | sed "s/^\(st\|bd\) /\1 launch=$i /" >> "$raw"
      cyc="$(awk -F, '/cycles/{print $1}' "$WORK/perf_$$.txt" 2>/dev/null | head -1)"
      secs="$(awk -F, '/seconds time elapsed/{print $1}' "$WORK/perf_$$.txt" 2>/dev/null | head -1)"
      rm -f "$WORK/perf_$$.txt"
    else
      ./target/release/"$bin" "$n" "$epochs" 2>/dev/null \
        | sed "s/^\(st\|bd\) /\1 launch=$i /" >> "$raw"
      cyc=""; secs=""
    fi
    post="$(peak_mhz)"
    printf 'clk tag=%s launch=%s pre_mhz=%s post_mhz=%s perf_cycles=%s perf_secs=%s\n' \
      "$tag" "$i" "$pre" "$post" "${cyc:-NA}" "${secs:-NA}" >> "$raw"
    printf '%s ' "$i"
  done
  echo
  say "data lines        : $(grep -cE '^(st|bd) launch=' "$raw")"
  say "clock lines       : $(grep -c '^clk ' "$raw")"
  say "peak MHz observed : $(awk '/^clk /{for(i=1;i<=NF;i++){split($i,a,"=");if(a[1]=="pre_mhz"||a[1]=="post_mhz"){if(a[2]+0>m)m=a[2]+0}}}END{print m+0}' "$raw")"
  say "min MHz observed  : $(awk '/^clk /{for(i=1;i<=NF;i++){split($i,a,"=");if(a[1]=="pre_mhz"||a[1]=="post_mhz"){if(n==0||a[2]+0<m){m=a[2]+0};n++}}}END{print m+0}' "$raw")"
  if [ "$HAVE_PERF" = "1" ]; then
    say "effective GHz from the cycle counter, per launch, median first and last:"
    awk '/^clk /{c="";s="";for(i=1;i<=NF;i++){split($i,a,"=");if(a[1]=="perf_cycles")c=a[2];if(a[1]=="perf_secs")s=a[2]}
         if(c!="NA"&&s!="NA"&&s+0>0){printf "  launch %s  %.3f GHz\n",NR,(c+0)/(s+0)/1e9}}' "$raw" | sed -n '1p;$p' | tee -a "$OUT"
  fi
  cat "$raw" >> "$WORK/raw_all.txt"
}

: > "$WORK/raw_all.txt"
say "=== MEASUREMENT ==="
say "Run length is the independent variable, so both counts are measured with"
say "everything else held fixed. 30 launches is the registered number."
hr
run_set titan-stream  200000   3 "stream_200k" 30
hr
run_set titan-stream  10000000 3 "stream_10M"  30
hr
run_set titan-barrier 10000000 3 "barrier_10M" 30
hr

# ---------------------------------------------------------------- analysis
say "=== ANALYSIS, by the repository's own analyzer, ONE RUN LENGTH AT A TIME ==="
say "Each run length is analysed from its own raw file and never from the pooled"
say "one. The analyzer keys a streaming case on (order, pad, cached, wait) and"
say "NOT on the message count, so feeding it both run lengths together would"
say "silently average 200k into 10M and destroy the only comparison this run"
say "exists to make. The launch numbers restart at 1 per run length as well, so"
say "pooling would also pair launch 1 of one against launch 1 of the other."
if command -v python3 >/dev/null 2>&1; then
  for tag in stream_200k stream_10M barrier_10M; do
    raw="$WORK/raw_${tag}.txt"
    [ -s "$raw" ] || continue
    hr
    say ">>> ANALYSIS: ${tag}"
    python3 scripts/launches.py "$raw" > "$WORK/analysis_${tag}.txt" 2>&1 || true
    say "    sections: $(grep -c '^## ' "$WORK/analysis_${tag}.txt") lines: $(wc -l < "$WORK/analysis_${tag}.txt")"
    cat "$WORK/analysis_${tag}.txt" >> "$OUT"
  done
else
  say "python3 not installed, so the raw data is reported without analysis."
  say "Paste the raw files back and they will be analysed here."
fi
hr

say "=== THE QUESTION THIS RUN EXISTS TO ANSWER ==="
say "Did the clock move? Compare the idle baseline against the peak observed"
say "during the runs, and compare the short run against the long run."
say "  If the clock is FLAT across all of it, the absence of a run-length"
say "  effect on x86 is a real property of this machine, and the Apple result"
say "  stands as specific to Apple silicon."
say "  If the clock MOVES here and the ordering conclusion still does not"
say "  reverse, then frequency scaling alone does not explain the Apple result"
say "  and the paper's mechanism claim is weakened and must be requalified."
say "Either outcome is publishable. The container run could produce neither,"
say "because it had no cpufreq interface at all."
hr
say "raw data : $WORK/raw_all.txt"
say "report   : $OUT"
say "Paste the report back. Keep the raw file."
say "finished $(date -u '+%Y-%m-%d %H:%M:%S UTC')"
