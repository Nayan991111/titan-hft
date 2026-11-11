#!/bin/bash
# Download one public NASDAQ BX TotalView-ITCH 5.0 sample day from
# emi.nasdaq.com, verify it against the published md5, and decompress it into
# data/ (git-ignored; the data is never committed or redistributed).
# Resumable: rerun after an interrupted download.
# Usage: scripts/fetch_itch_sample.sh [YYYYMMDD]   (default 20190730)
set -eu
cd "$(dirname "$0")/.."
DAY=${1:-20190730}
URL="${TITAN_ITCH_URL:-https://emi.nasdaq.com/ITCH/Nasdaq%20BX%20ITCH}"
F="$DAY.BX_ITCH_50"
mkdir -p data
cd data
if [ -f "$F" ]; then
  echo "data/$F already present ($(wc -c < "$F" | tr -d ' ') bytes)"
  exit 0
fi
md5_of() { if command -v md5 >/dev/null 2>&1; then md5 -q "$1"; else md5sum "$1" | cut -d' ' -f1; fi; }
curl -fsSL --retry 5 --retry-delay 5 -o "$F.gz.md5sum" "$URL/$F.gz.md5sum"
want=$(grep -oE '[0-9a-fA-F]{32}' "$F.gz.md5sum" | head -1 | tr 'A-F' 'a-f')
[ -n "$want" ] || { echo "no md5 in $F.gz.md5sum"; exit 1; }
complete=1
if [ ! -f "$F.gz" ] || [ "$(md5_of "$F.gz")" != "$want" ]; then
  curl -fL --retry 5 --retry-delay 5 -C - -o "$F.gz" "$URL/$F.gz" || complete=0
fi
got=$(md5_of "$F.gz")
if [ "$got" != "$want" ]; then
  if [ "$complete" = "1" ]; then
    # Transfer finished but content is wrong: discard so the next run starts clean.
    rm -f "$F.gz"
    echo "md5 mismatch on a completed transfer (expected $want, got $got); file discarded, rerun to download again"
  else
    echo "download interrupted; partial file kept, rerun to resume"
  fi
  exit 1
fi
echo "md5 verified: $got"
gunzip "$F.gz"
rm -f "$F.gz.md5sum"
echo "data/$F ready ($(wc -c < "$F" | tr -d ' ') bytes)"
