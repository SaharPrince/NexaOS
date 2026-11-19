#!/usr/bin/env bash
set -euo pipefail

# Repeated fresh-process diagnostic measurements for the synthetic NCE image
# preparation benchmark. GNU time reports process maximum RSS in KiB.
RUNS="${1:-5}"
if ! [[ "$RUNS" =~ ^[1-9][0-9]*$ ]] || (( RUNS > 20 )); then
  echo "usage: $0 [runs: 1..20]" >&2
  exit 2
fi
for tool in cargo /usr/bin/time; do
  command -v "$tool" >/dev/null 2>&1 || { echo "required tool not found: $tool" >&2; exit 2; }
done

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$ROOT"
TMP="$(mktemp -d)"
trap 'rm -rf "$TMP"' EXIT

: "${NEXA_BENCH_PARSE_ITERS:=1000}"
: "${NEXA_BENCH_LAYOUT_ITERS:=1000}"
: "${NEXA_BENCH_BUFFER_ITERS:=200}"
: "${NEXA_BENCH_PREPARE_ITERS:=200}"
: "${NEXA_BENCH_HOLD_MS:=1000}"
export NEXA_BENCH_PARSE_ITERS NEXA_BENCH_LAYOUT_ITERS
export NEXA_BENCH_BUFFER_ITERS NEXA_BENCH_PREPARE_ITERS NEXA_BENCH_HOLD_MS

echo "NexaOS NCE-005c Linux repeated benchmark"
echo "os=$(uname -srv)"
echo "architecture=$(uname -m)"
echo "cpu_model=$(awk -F: '/model name/ {sub(/^[ \t]+/, "", $2); print $2; exit}' /proc/cpuinfo 2>/dev/null || true)"
echo "logical_processor_count=$(nproc)"
echo "installed_memory_kib=$(awk '/MemTotal:/ {print $2; exit}' /proc/meminfo)"
echo "rustc=$(rustc --version)"
echo "cargo=$(cargo --version)"
echo "measurement_runs=$RUNS"
echo "parse_iterations=$NEXA_BENCH_PARSE_ITERS"
echo "plan_iterations=$NEXA_BENCH_LAYOUT_ITERS"
echo "buffer_iterations=$NEXA_BENCH_BUFFER_ITERS"
echo "preparation_iterations=$NEXA_BENCH_PREPARE_ITERS"
echo "hold_milliseconds=$NEXA_BENCH_HOLD_MS"

cargo bench --locked -p nexa-inspect --bench nexa_image --no-run
BENCH="$(find target/release/deps -maxdepth 1 -type f -name 'nexa_image-*' -executable -print | sort | tail -n 1)"
if [[ -z "$BENCH" ]]; then
  echo "could not locate executable nexa_image benchmark under target/release/deps" >&2
  exit 1
fi
echo "benchmark_executable=$BENCH"

declare -A PHASE_VALUES=()
RSS_VALUES=()
for ((run=1; run<=RUNS; run++)); do
  echo
  echo "=== BENCHMARK RUN $run OF $RUNS ==="
  /usr/bin/time -f '%M' -o "$TMP/rss" "$BENCH" >"$TMP/stdout" 2>"$TMP/stderr"
  cat "$TMP/stdout"
  cat "$TMP/stderr" >&2
  rss="$(tr -d '[:space:]' < "$TMP/rss")"
  if ! [[ "$rss" =~ ^[0-9]+$ ]] || (( rss <= 0 )); then
    echo "GNU time did not return a valid maximum-RSS value for run $run" >&2
    exit 1
  fi
  RSS_VALUES+=("$rss")
  echo "run_${run}_max_rss_kib=$rss"
  while IFS= read -r line; do
    if [[ "$line" =~ ^([A-Za-z0-9_]+):[[:space:]]iterations=[0-9]+,[[:space:]]total_ms=[0-9.]+,[[:space:]]ns_per_iteration=([0-9]+) ]]; then
      phase="${BASH_REMATCH[1]}"
      value="${BASH_REMATCH[2]}"
      PHASE_VALUES["$phase"]="${PHASE_VALUES[$phase]:-} $value"
    fi
  done < "$TMP/stdout"
done

summarize() {
  local label="$1"; shift
  local values=("$@") sorted count min max median p90
  mapfile -t sorted < <(printf '%s\n' "${values[@]}" | sort -n)
  count="${#sorted[@]}"
  min="${sorted[0]}"
  max="${sorted[$((count-1))]}"
  if (( count % 2 )); then
    median="${sorted[$((count/2))]}"
  else
    median="$(( (sorted[count/2-1] + sorted[count/2] + 1) / 2 ))"
  fi
  p90_index="$(( (9*count + 9)/10 - 1 ))"
  p90="${sorted[$p90_index]}"
  echo "summary=${label},samples=${count},min=${min},median=${median},p90_nearest_rank=${p90},max=${max}"
}

echo
echo "=== REPEATED-RUN PHASE SUMMARY (NANOSECONDS / ITERATION) ==="
for phase in "${!PHASE_VALUES[@]}"; do
  read -r -a values <<< "${PHASE_VALUES[$phase]}"
  if (( ${#values[@]} != RUNS )); then
    echo "phase $phase produced ${#values[@]} samples; expected $RUNS" >&2
    exit 1
  fi
  summarize "$phase" "${values[@]}"
done
echo
echo "=== PROCESS MAXIMUM RSS SUMMARY (KIB) ==="
summarize "process_maximum_rss_kib" "${RSS_VALUES[@]}"
echo "measurement=GNU time %M maximum resident set size for each fresh benchmark process; KiB"
echo "scope=benchmark process only; not system-wide memory, private bytes, or total heap allocations"
echo "note=hosted CI is a CI baseline, not a low-end hardware measurement"
