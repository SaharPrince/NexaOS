#!/usr/bin/env bash
set -Eeuo pipefail

ROOT_DIR="$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")/.." && pwd)"
ISO_PATH="${1:-$ROOT_DIR/artifacts/nexaos-preview-amd64.iso}"
ARTIFACT_DIR="$ROOT_DIR/artifacts"
SCREENSHOT="$ARTIFACT_DIR/nexaos-qemu-boot-smoke.xwd"
QEMU_LOG="$ARTIFACT_DIR/nexaos-qemu-boot-smoke.log"

for command in qemu-system-x86_64 xvfb-run xwd identify timeout; do
    if ! command -v "$command" >/dev/null 2>&1; then
        printf 'Required command not found: %s\n' "$command" >&2
        exit 1
    fi
done

if [[ ! -s "$ISO_PATH" ]]; then
    printf 'A non-empty preview ISO is required: %s\n' "$ISO_PATH" >&2
    exit 1
fi

mkdir -p "$ARTIFACT_DIR"
rm -f "$SCREENSHOT" "$QEMU_LOG"

printf 'Starting isolated QEMU boot smoke test for: %s\n' "$ISO_PATH"

timeout 300s xvfb-run -a -s "-screen 0 1280x800x24" bash -c '
    set -Eeuo pipefail
    iso_path="$1"
    screenshot="$2"
    qemu_log="$3"

    qemu-system-x86_64 \
        -machine pc,accel=tcg \
        -cpu qemu64 \
        -m 3072 \
        -smp 2 \
        -vga std \
        -cdrom "$iso_path" \
        -boot order=d \
        -nic none \
        -display sdl \
        -no-reboot \
        -no-shutdown \
        >"$qemu_log" 2>&1 &
    qemu_pid=$!

    cleanup() {
        if kill -0 "$qemu_pid" 2>/dev/null; then
            kill "$qemu_pid" 2>/dev/null || true
            wait "$qemu_pid" 2>/dev/null || true
        fi
    }
    trap cleanup EXIT

    # Allow the live boot to reach its graphical startup path.
    for second in $(seq 1 150); do
        if ! kill -0 "$qemu_pid" 2>/dev/null; then
            printf "QEMU exited before the observation window completed.\n" >&2
            cat "$qemu_log" >&2 || true
            exit 1
        fi
        sleep 1
    done

    xwd -root -silent -out "$screenshot"
    test -s "$screenshot"
' _ "$ISO_PATH" "$SCREENSHOT" "$QEMU_LOG"

dimensions="$(identify -quiet -format '%wx%h' "$SCREENSHOT")"
color_count="$(identify -quiet -format '%k' "$SCREENSHOT")"

if [[ "$dimensions" != "1280x800" ]]; then
    printf 'Unexpected VM screenshot dimensions: %s\n' "$dimensions" >&2
    exit 1
fi

if [[ ! "$color_count" =~ ^[0-9]+$ ]] || (( color_count < 8 )); then
    printf 'VM screenshot appears blank or invalid (unique colors: %s).\n' "$color_count" >&2
    exit 1
fi

printf 'QEMU remained running for the 150-second observation window.\n'
printf 'Captured graphical output: %s (%s, %s unique colors after downscaling).\n' \
    "$SCREENSHOT" "$dimensions" "$color_count"
printf 'This smoke test does not establish successful desktop login or hardware compatibility.\n'
