#!/usr/bin/env bash
set -Eeuo pipefail

ROOT_DIR="$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")/.." && pwd)"
ISO_PATH="${1:-$ROOT_DIR/artifacts/nexaos-preview-amd64.iso}"
ARTIFACT_DIR="$ROOT_DIR/artifacts"
SCREENSHOT="$ARTIFACT_DIR/nexaos-qemu-boot-smoke.xwd"
QEMU_LOG="$ARTIFACT_DIR/nexaos-qemu-boot-smoke.log"
SESSION_SOCKET="$ARTIFACT_DIR/nexaos-session-smoke.sock"
MONITOR_SOCKET="$ARTIFACT_DIR/nexaos-qemu-monitor.sock"
SESSION_REPORT="$ARTIFACT_DIR/nexaos-session-smoke.json"
SESSION_SMOKE="${NEXAOS_SESSION_SMOKE:-0}"

for command in qemu-system-x86_64 xvfb-run xwd identify timeout python3; do
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
rm -f "$SCREENSHOT" "$QEMU_LOG" "$SESSION_SOCKET" "$MONITOR_SOCKET" "$SESSION_REPORT"

printf 'Starting isolated QEMU boot smoke test for: %s\n' "$ISO_PATH"

timeout 300s xvfb-run -a -s "-screen 0 1280x800x24" bash -c '
    set -Eeuo pipefail
    iso_path="$1"
    screenshot="$2"
    qemu_log="$3"
    session_socket="$4"
    session_report="$5"
    session_checker="$6"
    session_smoke="$7"
    monitor_socket="$8"
    boot_key_helper="$9"

    qemu_args=(
        -machine pc,accel=tcg
        -cpu qemu64
        -m 3072
        -smp 2
        -vga std
        -cdrom "$iso_path"
        -boot order=d
        -nic none
        -display sdl
        -monitor "unix:$monitor_socket,server=on,wait=off"
        -no-reboot
        -no-shutdown
    )
    if [[ "$session_smoke" == "1" ]]; then
        qemu_args+=(
            -chardev "socket,id=session,path=$session_socket,server=on,wait=off"
            -device virtio-serial-pci
            -device virtserialport,chardev=session,name=org.nexaos.session-smoke
        )
    fi

    qemu-system-x86_64 "${qemu_args[@]}" >"$qemu_log" 2>&1 &
    qemu_pid=$!

    cleanup() {
        # Preserve the last guest display even when the report channel or
        # desktop autostart fails, so CI can distinguish boot/login/UI states.
        if kill -0 "$qemu_pid" 2>/dev/null; then
            if [[ ! -s "$screenshot" ]]; then
                xwd -root -silent -out "$screenshot" 2>/dev/null || true
            fi
            kill "$qemu_pid" 2>/dev/null || true
            wait "$qemu_pid" 2>/dev/null || true
        fi
    }
    trap cleanup EXIT

    # Allow firmware and the ISO bootloader to render the live menu before
    # sending Enter; monitor availability alone does not mean the menu is ready.
    sleep 5
    # The Debian live ISO presents an interactive boot menu in this VM. Select
    # its default live entry explicitly instead of waiting for guest autostart.
    python3 "$boot_key_helper" "$monitor_socket" ret --timeout 30

    if [[ "$session_smoke" == "1" ]]; then
        python3 "$session_checker" "$session_socket" "$session_report" --timeout 210
        # Confirm the guest remains alive after the interactive checks complete.
        sleep 10
    else
        # Legacy framebuffer-only mode is retained for release-image builds.
        for second in $(seq 1 150); do
            if ! kill -0 "$qemu_pid" 2>/dev/null; then
                printf "QEMU exited before the observation window completed.\n" >&2
                cat "$qemu_log" >&2 || true
                exit 1
            fi
            sleep 1
        done
    fi

    if ! kill -0 "$qemu_pid" 2>/dev/null; then
        printf "QEMU exited before evidence capture completed.\n" >&2
        cat "$qemu_log" >&2 || true
        exit 1
    fi

    xwd -root -silent -out "$screenshot"
    test -s "$screenshot"
' _ "$ISO_PATH" "$SCREENSHOT" "$QEMU_LOG" "$SESSION_SOCKET" "$SESSION_REPORT" "$ROOT_DIR/scripts/check_live_session.py" "$SESSION_SMOKE" "$MONITOR_SOCKET" "$ROOT_DIR/scripts/qemu_send_boot_key.py"

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

if [[ "$SESSION_SMOKE" == "1" ]]; then
    test -s "$SESSION_REPORT"
    printf 'Interactive LXQt session report: %s\n' "$SESSION_REPORT"
else
    printf 'Framebuffer-only mode: no interactive session assertions were requested.\n'
fi

printf 'Captured graphical output: %s (%s, %s unique colors after downscaling).\n' \
    "$SCREENSHOT" "$dimensions" "$color_count"
printf 'QEMU smoke does not establish physical-device compatibility.\n'
