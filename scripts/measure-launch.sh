#!/bin/sh
set -eu

if [ "$#" -ne 1 ] || [ ! -x "$1" ]; then
  printf 'usage: scripts/measure-launch.sh /path/to/jobglass-executable\n' >&2
  exit 64
fi

executable=$1
receipt_dir=$(mktemp -d)
receipt_log=$receipt_dir/jobglass-launch.log
app_pid=

cleanup() {
  if [ -n "$app_pid" ] && kill -0 "$app_pid" 2>/dev/null; then
    kill "$app_pid"
    wait "$app_pid" 2>/dev/null || true
  fi
  rm -rf "$receipt_dir"
}
trap cleanup EXIT INT TERM

measure_launch() {
  : >"$receipt_log"
  started=$(perl -MTime::HiRes=clock_gettime,CLOCK_MONOTONIC -e 'printf "%.9f", clock_gettime(CLOCK_MONOTONIC)')
  "$executable" >"$receipt_log" 2>&1 &
  app_pid=$!

  attempt=0
  while ! grep -q '"event":"scan.complete"' "$receipt_log"; do
    if ! kill -0 "$app_pid" 2>/dev/null; then
      printf 'JobGlass exited before the scan receipt was emitted.\n' >&2
      sed -n '1,80p' "$receipt_log" >&2
      exit 1
    fi
    attempt=$((attempt + 1))
    if [ "$attempt" -ge 300 ]; then
      printf 'JobGlass did not emit a scan receipt within 15 seconds.\n' >&2
      exit 1
    fi
    sleep 0.05
  done

  finished=$(perl -MTime::HiRes=clock_gettime,CLOCK_MONOTONIC -e 'printf "%.9f", clock_gettime(CLOCK_MONOTONIC)')
  elapsed_ms=$(perl -e 'printf "%.1f", ($ARGV[1] - $ARGV[0]) * 1000' "$started" "$finished")
  collect_receipt=$(grep -oE '\{"event":"scan\.collect"[^}]+\}' "$receipt_log" | head -n 1)
  scan_receipt=$(grep -oE '\{"event":"scan\.complete"[^}]+\}' "$receipt_log" | head -n 1)
}

measure_launch
prime_ms=$elapsed_ms
kill "$app_pid"
wait "$app_pid" 2>/dev/null || true
app_pid=

measure_launch
printf 'warm-launch-ms=%s prime-launch-ms=%s %s %s\n' "$elapsed_ms" "$prime_ms" "$collect_receipt" "$scan_receipt"
awk -v elapsed="$elapsed_ms" 'BEGIN { exit !(elapsed < 1500) }'
