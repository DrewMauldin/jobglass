#!/bin/sh
set -eu

profile=local
if [ "$#" -eq 2 ] && [ "$1" = "--hosted" ]; then
  profile=hosted
  shift
fi
if [ "$#" -ne 1 ] || [ ! -x "$1" ]; then
  printf 'usage: scripts/measure-launch.sh [--hosted] /path/to/jobglass-executable\n' >&2
  exit 64
fi

executable=$1
warm_budget_ms=1500
collect_budget_ms=1000
diagnostics_budget_ms=500
if [ "$profile" = "hosted" ]; then
  warm_budget_ms=5000
fi
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
collect_ms=$(printf '%s\n' "$collect_receipt" | perl -ne 'print $1 if /"collectMs":([0-9]+(?:\.[0-9]+)?)/')
diagnostics_ms=$(printf '%s\n' "$scan_receipt" | perl -ne 'print $1 if /"diagnosticsMs":([0-9]+(?:\.[0-9]+)?)/')
if [ -z "$collect_ms" ] || [ -z "$diagnostics_ms" ]; then
  printf 'JobGlass emitted an incomplete performance receipt.\n' >&2
  exit 1
fi
printf 'profile=%s warm-launch-ms=%s prime-launch-ms=%s warm-budget-ms=%s collect-budget-ms=%s diagnostics-budget-ms=%s %s %s\n' \
  "$profile" "$elapsed_ms" "$prime_ms" "$warm_budget_ms" "$collect_budget_ms" "$diagnostics_budget_ms" "$collect_receipt" "$scan_receipt"
if ! awk \
  -v elapsed="$elapsed_ms" \
  -v collect="$collect_ms" \
  -v diagnostics="$diagnostics_ms" \
  -v warm_budget="$warm_budget_ms" \
  -v collect_budget="$collect_budget_ms" \
  -v diagnostics_budget="$diagnostics_budget_ms" \
  'BEGIN { exit !(elapsed < warm_budget && collect < collect_budget && diagnostics < diagnostics_budget) }'; then
  printf 'JobGlass exceeded a configured launch or scan budget.\n' >&2
  exit 1
fi
