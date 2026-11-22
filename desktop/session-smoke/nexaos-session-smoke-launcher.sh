#!/bin/sh
# CI-only LXQt autostart wrapper. It leaves a local breadcrumb and sends
# bounded, non-sensitive startup events over QEMU's dedicated report port.
set -u

LOG=/tmp/nexaos-session-smoke-launcher.log
DEVICE=/dev/virtio-ports/org.nexaos.session-smoke
SCRIPT=/usr/lib/nexaos/nexaos-session-smoke.py

log() {
    printf '%s %s\n' "$(date -u '+%Y-%m-%dT%H:%M:%SZ' 2>/dev/null || printf 'time-unavailable')" "$1" >> "$LOG"
}

emit() {
    stage=$1
    display_set=false
    dbus_set=false
    [ -n "${DISPLAY:-}" ] && display_set=true
    [ -n "${DBUS_SESSION_BUS_ADDRESS:-}" ] && dbus_set=true
    if [ -e "$DEVICE" ]; then
        printf '{"kind":"event","stage":"%s","display_set":%s,"dbus_session_set":%s}\n' \
            "$stage" "$display_set" "$dbus_set" > "$DEVICE" 2>> "$LOG" || log "report transport failed at $stage"
    else
        log "virtio report device missing at $stage"
    fi
}

log "autostart launcher entered"
emit "launcher_started"
if [ ! -x /usr/bin/python3 ]; then
    log "python interpreter missing"
    emit "python_missing"
    exit 127
fi
if [ ! -r "$SCRIPT" ]; then
    log "session smoke script missing or unreadable"
    emit "script_missing"
    exit 126
fi
log "starting session smoke Python"
emit "python_starting"
exec /usr/bin/python3 "$SCRIPT" >> "$LOG" 2>&1
