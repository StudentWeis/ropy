#!/usr/bin/env bash
set -euo pipefail

# record_build_size.sh
# Usage:
#   script/record_build_size.sh [path/to/binary ...]
# If no binary args are provided, defaults to target/release/ropy

CSV_FILE="doc/build_sizes.csv"
DATE=$(date -u +"%Y-%m-%dT%H:%M:%SZ")
COMMIT="$(git rev-parse --short HEAD 2>/dev/null || echo unknown)"

if [ "$#" -eq 0 ]; then
  set -- target/release/ropy
fi

ensure_header() {
  if [ ! -f "$CSV_FILE" ]; then
    printf "%s\n" "date,commit,binary,size_bytes,gzip_bytes,delta_bytes,delta_gzip" > "$CSV_FILE"
  fi
}

get_size() {
  local f="$1"
  wc -c < "$f" | tr -d ' '
}

get_gzip_size() {
  local f="$1"
  gzip -c "$f" | wc -c | tr -d ' '
}

last_values_for() {
  local bin="$1"
  if [ ! -f "$CSV_FILE" ]; then
    echo
    return
  fi
  awk -F, -v b="$bin" '$3==b{last=$0}END{if(last)print last}' "$CSV_FILE"
}

ensure_header

for bin in "$@"; do
  if [ ! -f "$bin" ]; then
    echo "Warning: binary not found: $bin" >&2
    continue
  fi

  size=$(get_size "$bin")
  gsize=$(get_gzip_size "$bin")

  last=$(last_values_for "$bin")
  if [ -n "$last" ]; then
    IFS="," read -r ldate lcommit lbinary lsize lgsize ldelta lgdelta <<< "$last"
    delta=$((size - lsize))
    gdelta=$((gsize - lgsize))
  else
    delta=""
    gdelta=""
  fi

  # Append CSV line
  printf "%s,%s,%s,%s,%s,%s,%s\n" "$DATE" "$COMMIT" "$bin" "$size" "$gsize" "$delta" "$gdelta" >> "$CSV_FILE"

  # Print short summary
  printf "Recorded %s: size=%s bytes, gzip=%s bytes" "$bin" "$size" "$gsize"
  if [ -n "$delta" ]; then
    printf ", delta=%+d bytes, gzip_delta=%+d bytes" "$delta" "$gdelta"
  fi
  printf "\n"
done

echo "Appended to $CSV_FILE"
