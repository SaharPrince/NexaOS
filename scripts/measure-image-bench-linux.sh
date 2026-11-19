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

# Ask Cargo for the actual executable path instead of assuming the target
# directory layout (which may include a target triple or custom target dir).
cargo bench --locked -p nexa-inspect --bench nexa_image --no-run --message-format=json > "$TMP/cargo-build.jsonl"
BENCH="$(python3 - "$TMP/cargo-build.jsonl" <<'PY'
import json
import sys

path = sys.argv[1]
with open(path, encoding="utf-8") as stream:
    for line in stream:
        try:
            message = json.loads(line)
        except json.JSONDecodeError:
            continue
        if message.get("reason") != "compiler-artifact":
            continue
        target = message.get("target", {})
        if "bench" in target.get("kind", []) and target.get("name") == "nexa_image":
            executable = message.get("executable")
            if executable:
                print(executable)
                break
PY
)"
if [[ -z "$BENCH" || ! -x "$BENCH" ]]; then
  echo "Cargo did not report an executable nexa_image benchmark" >&2
  cat "$TMP/cargo-build.jsonl" >&2
  exit 1
fi
echo "benchmark_executable=$BENCH"

EXPECTED_PHASES=(
  parse_inspect
  layout_planning
  image_buffer_allocation
  image_allocation_and_population
  population_only
  prepare_image_end_to_end_counting_allocator
  prepare_image_end_to_end_system_allocator
  prepare_image_import_bearing_counting_allocator
  prepare_image_relocation_bearing_counting_allocator
)
declare -A PHASE_VALUES=()
RSS_VALUES=()
for ((run=1; run<=RUNS; run++)); do
  echo
  echo "=== BENCHMARK RUN $run OF $RUNS ==="
  /usr/bin/time -f '%M' -o "$TMP/rss" "$BENCH" >"$TMP/stdout" 2>"$TMP/stderr"
  cat "$TMP/stdout"
  cat "$TMP/stderr" >&2
  if ! grep -qx 'execution: NOT_ATTEMPTED' "$TMP/stdout"; then
    echo "benchmark run $run did not confirm that fixture execution was not attempted" >&2
    exit 1
  fi
  rss="$(tr -d '[:space:]' < "$TMP/rss")"
  if ! [[ "$rss" =~ ^[0-9]+$ ]] || (( rss <= 0 )); then
    echo "GNU time did not return a valid maximum-RSS value for run $run" >&2
    exit 1
  fi
  RSS_VALUES+=("$rss")
  echo "run_${run}_max_rss_kib=$rss"
  declare -A RUN_PHASES=()
  while IFS= read -r line; do
    if [[ "$line" =~ ^([A-Za-z0-9_]+):[[:space:]]iterations=[0-9]+,[[:space:]]total_ms=[0-9.]+,[[:space:]]ns_per_iteration=([0-9]+) ]]; then
      phase="${BASH_REMATCH[1]}"
      value="${BASH_REMATCH[2]}"
      if [[ " ${EXPECTED_PHASES[*]} " == *" $phase "* ]]; then
        if [[ -n "${RUN_PHASES[$phase]:-}" ]]; then
          echo "benchmark run $run emitted duplicate phase '$phase'" >&2
          exit 1
        fi
        RUN_PHASES["$phase"]=1
        PHASE_VALUES["$phase"]="${PHASE_VALUES[$phase]:-} $value"
      fi
    fi
  done < "$TMP/stdout"
  for phase in "${EXPECTED_PHASES[@]}"; do
    if [[ -z "${RUN_PHASES[$phase]:-}" ]]; then
      echo "benchmark run $run is missing required phase '$phase'" >&2
      exit 1
    fi
  done
  unset RUN_PHASES
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
for phase in "${EXPECTED_PHASES[@]}"; do
  if [[ -z "${PHASE_VALUES[$phase]:-}" ]]; then
    echo "required benchmark phase '$phase' was missing from every run" >&2
    exit 1
  fi
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
