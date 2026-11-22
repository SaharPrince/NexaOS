#!/usr/bin/env bash
set -Eeuo pipefail

ROOT_DIR="$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")/.." && pwd)"
BUILD_DIR="$ROOT_DIR/desktop/live-build"
ARTIFACT_DIR="$ROOT_DIR/artifacts"

for command in lb sudo find sha256sum cargo; do
    if ! command -v "$command" >/dev/null 2>&1; then
        printf 'Required command not found: %s\n' "$command" >&2
        exit 1
    fi
done

if [[ ! -f "$BUILD_DIR/config/package-lists/nexaos-desktop.list.chroot" ]]; then
    printf 'Desktop package list is missing.\n' >&2
    exit 1
fi

for source in nexaos_welcome.py nexaos_file_browser.py nexaos_task_manager.py; do
    if [[ ! -s "$ROOT_DIR/desktop/nexaos-welcome/$source" ]]; then
        printf 'NexaOS desktop application source is missing: %s\n' "$source" >&2
        exit 1
    fi
done

# Keep reviewed sources canonical; stage them into live-build's chroot overlay.
install -D -m 0644 \
    "$ROOT_DIR/desktop/nexaos-welcome/nexaos_welcome.py" \
    "$BUILD_DIR/config/includes.chroot/usr/lib/nexaos/nexaos_welcome.py"
install -D -m 0644 \
    "$ROOT_DIR/desktop/nexaos-welcome/nexaos_file_browser.py" \
    "$BUILD_DIR/config/includes.chroot/usr/lib/nexaos/nexaos_file_browser.py"
install -D -m 0644 \
    "$ROOT_DIR/desktop/nexaos-welcome/nexaos_task_manager.py" \
    "$BUILD_DIR/config/includes.chroot/usr/lib/nexaos/nexaos_task_manager.py"
# Build the small, preview-only Rust controller and stage it as a fixed system helper.
(
    cd "$ROOT_DIR"
    cargo build --locked --release -p nexa-desktop-shell --bin nexa-desktop-controller
)
install -D -m 0755 \
    "$ROOT_DIR/target/release/nexa-desktop-controller" \
    "$BUILD_DIR/config/includes.chroot/usr/lib/nexaos/nexa-desktop-controller"
mkdir -p "$ARTIFACT_DIR"
cd "$BUILD_DIR"

# Clear generated live-build stages but retain the reviewed source config.
sudo lb clean --purge

sudo lb config \
    --ignore-system-defaults \
    --mode debian \
    --distribution trixie \
    --architecture amd64 \
    --archive-areas "main non-free-firmware" \
    --binary-image iso-hybrid \
    --debian-installer none \
    --apt-recommends false \
    --firmware-chroot true \
    --firmware-binary true \
    --system live \
    --iso-application "NexaOS Preview" \
    --iso-publisher "NexaOS Project" \
    --iso-volume NEXAOS_PREVIEW \
    --bootappend-live "boot=live components" \
    --source false

sudo lb build

ISO_PATH="$(find "$BUILD_DIR" -maxdepth 2 -type f -name 'nexaos-preview-*.iso' -print -quit)"
if [[ -z "$ISO_PATH" ]]; then
    ISO_PATH="$(find "$BUILD_DIR" -maxdepth 2 -type f -name '*.iso' -print -quit)"
fi

if [[ -z "$ISO_PATH" || ! -s "$ISO_PATH" ]]; then
    printf 'live-build completed without producing a non-empty ISO.\n' >&2
    exit 1
fi

OUTPUT_ISO="$ARTIFACT_DIR/nexaos-preview-amd64.iso"
cp -- "$ISO_PATH" "$OUTPUT_ISO"
(
    cd "$ARTIFACT_DIR"
    sha256sum "$(basename "$OUTPUT_ISO")" > "$(basename "$OUTPUT_ISO").sha256"
)

printf '\nBuilt preview image:\n%s\n' "$OUTPUT_ISO"
du -h "$OUTPUT_ISO"
cat "$OUTPUT_ISO.sha256"
