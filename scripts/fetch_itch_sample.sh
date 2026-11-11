#!/bin/bash
# Download one public NASDAQ BX TotalView-ITCH 5.0 sample day from
# emi.nasdaq.com, verify it against the published md5 (or the listed size when
# no md5 is published), and decompress it into data/ (git-ignored; the data is
# never committed or redistributed).
#
# File names are not hard-coded: the script reads the server's directory
# listing, finds the BX ITCH 5.0 folder and the files it holds, and uses the
# exact links found there. The requested day is used when it is listed;
# otherwise the smallest listed day is used and the choice is printed.
# Resumable: rerun after an interrupted download.
#
# Usage: scripts/fetch_itch_sample.sh [YYYYMMDD]
#   TITAN_ITCH_ROOT  listing root (default https://emi.nasdaq.com/ITCH/)
# The last line of a successful run is ITCH_FILE=data/<name>.
set -eu
cd "$(dirname "$0")/.."
DAY=${1:-20190730}
ROOT="${TITAN_ITCH_ROOT:-https://emi.nasdaq.com/ITCH/}"
case "$ROOT" in */) ;; *) ROOT="$ROOT/" ;; esac
mkdir -p data
cd data

for f in "$DAY".*ITCH_50; do
  if [ -f "$f" ]; then
    echo "data/$f already present ($(wc -c < "$f" | tr -d ' ') bytes)"
    echo "ITCH_FILE=data/$f"
    exit 0
  fi
done

UA="Mozilla/5.0 (Macintosh; Intel Mac OS X 10_15_7)"
# get URL OUT: download, print the HTTP status, fail on anything but 200.
get() {
  code=$(curl -sS -L -A "$UA" --retry 3 --retry-delay 3 -o "$2" -w '%{http_code}' "$1" || true)
  if [ "$code" != "200" ]; then
    echo "GET $1 -> HTTP ${code:-none}"
    return 1
  fi
}
# Origin (scheme://host) of a URL, used to resolve absolute-path links.
origin() { printf '%s\n' "$1" | awk -F/ '{print $1 "//" $3}'; }
# resolve BASE_DIR_URL HREF
resolve() {
  case "$2" in
    http://*|https://*) printf '%s\n' "$2" ;;
    /*) printf '%s%s\n' "$(origin "$1")" "$2" ;;
    *) printf '%s%s\n' "$1" "$2" ;;
  esac
}
# Parse an IIS-style listing into "SIZE<TAB>HREF" lines (SIZE is "-" when
# absent, e.g. for folders).
parse() {
  awk '{
    gsub(/<[bB][rR] *\/?>/, "\n")
    n = split($0, L, "\n")
    for (i = 1; i <= n; i++) {
      l = L[i]; lo = tolower(l); p = index(lo, "href=")
      if (!p) continue
      rest = substr(l, p + 5); qc = substr(rest, 1, 1)
      if (qc != "\"" && qc != "\047") continue
      rest = substr(rest, 2); q = index(rest, qc)
      if (!q) continue
      href = substr(rest, 1, q - 1)
      m = split(substr(l, 1, p - 1), W, " "); size = "-"
      for (j = m; j >= 1; j--) if (W[j] ~ /^[0-9]+$/) { size = W[j]; break }
      print size "\t" href
    }
  }' "$1"
}
base_name() { printf '%s\n' "${1##*/}" | sed 's/%20/ /g'; }

fail_help() {
  echo
  echo "Could not discover the sample files automatically."
  echo "Open ${ROOT} in a browser and report the folder and file names shown."
  exit 1
}

get "$ROOT" itch_root_listing.html || fail_help
dir=$(parse itch_root_listing.html | cut -f2 | awk '{l = tolower($0); gsub(/%20/, " ", l)} l ~ /bx/ && l ~ /itch/ && l ~ /\/$/ {print; exit}')
[ -n "$dir" ] || { echo "no BX ITCH folder in the root listing; entries:"; parse itch_root_listing.html | cut -f2; fail_help; }
DIR_URL=$(resolve "$ROOT" "$dir")
echo "listing $DIR_URL"
get "$DIR_URL" itch_listing.html || fail_help
parse itch_listing.html > itch_listing.tsv

awk -F'\t' '{l = tolower($2)} l ~ /itch_50\.gz$/ {print}' itch_listing.tsv > itch_candidates.tsv
count=$(wc -l < itch_candidates.tsv | tr -d ' ')
[ "$count" -gt 0 ] || { echo "no *ITCH_50.gz files listed; entries:"; cat itch_listing.tsv; fail_help; }
echo "$count ITCH 5.0 files listed"

pick=$(awk -F'\t' -v d="$DAY" '{n = $2; sub(/.*\//, "", n)} index(n, d) == 1 {print; exit}' itch_candidates.tsv)
if [ -z "$pick" ]; then
  pick=$(awk -F'\t' '$1 ~ /^[0-9]+$/' itch_candidates.tsv | sort -n | head -1)
  [ -n "$pick" ] || pick=$(head -1 itch_candidates.tsv)
  echo "day $DAY not listed; using the smallest listed file instead"
fi
SIZE=$(printf '%s\n' "$pick" | cut -f1)
HREF=$(printf '%s\n' "$pick" | cut -f2)
GZ_URL=$(resolve "$DIR_URL" "$HREF")
GZ=$(base_name "$HREF")
F=${GZ%.gz}
echo "selected $GZ (listed size $SIZE bytes)"

if [ -f "$F" ]; then
  echo "data/$F already present"
  echo "ITCH_FILE=data/$F"
  exit 0
fi

md5_of() { if command -v md5 >/dev/null 2>&1; then md5 -q "$1"; else md5sum "$1" | cut -d' ' -f1; fi; }
size_of() { wc -c < "$1" | tr -d ' '; }
want=""
md5_href=$(awk -F'\t' -v h="$HREF.md5sum" 'tolower($2) == tolower(h) {print $2; exit}' itch_listing.tsv)
if [ -n "$md5_href" ] && get "$(resolve "$DIR_URL" "$md5_href")" "$GZ.md5sum"; then
  want=$(grep -oE '[0-9a-fA-F]{32}' "$GZ.md5sum" | head -1 | tr 'A-F' 'a-f')
fi
if [ -n "$want" ]; then echo "published md5 $want"; else echo "no md5 published; verifying by listed size"; fi

ok() {
  [ -f "$GZ" ] || return 1
  if [ -n "$want" ]; then [ "$(md5_of "$GZ")" = "$want" ]
  elif [ "$SIZE" != "-" ]; then [ "$(size_of "$GZ")" = "$SIZE" ]
  else gzip -t "$GZ" 2>/dev/null
  fi
}

complete=1
if ! ok; then
  rc=0
  curl -fL -A "$UA" --retry 5 --retry-delay 5 -C - -o "$GZ" "$GZ_URL" || rc=$?
  if [ "$rc" = "33" ]; then
    # The server refused to resume (no byte ranges): start the transfer over.
    echo "server does not resume transfers; downloading from the start"
    rm -f "$GZ"
    rc=0
    curl -fL -A "$UA" --retry 5 --retry-delay 5 -o "$GZ" "$GZ_URL" || rc=$?
  fi
  [ "$rc" = "0" ] || complete=0
fi
if ! ok; then
  if [ "$complete" = "1" ]; then
    # Transfer finished but content is wrong: discard so the next run starts clean.
    rm -f "$GZ"
    echo "verification failed on a completed transfer; file discarded, rerun to download again"
  else
    echo "download interrupted; partial file kept, rerun to resume"
  fi
  exit 1
fi
echo "verified $GZ ($(size_of "$GZ") bytes)"
gunzip "$GZ"
rm -f "$GZ.md5sum" itch_candidates.tsv
echo "data/$F ready ($(size_of "$F") bytes)"
echo "ITCH_FILE=data/$F"
