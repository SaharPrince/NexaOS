#!/bin/sh
# CI-only LXQt autostart wrapper. Keep a bounded, non-sensitive record of
# interpreter startup and guarantee a failure report if Python exits silently.
set -u

LOG=/tmp/nexaos-session-smoke-launcher.log
DEVICE=/dev/virtio-ports/org.nexaos.session-smoke
SCRIPT=/usr/lib/nexaos/nexaos-session-smoke.py
REPORT_SENT=/tmp/nexaos-session-smoke-report-sent
PYTHON_LOG=/tmp/nexaos-session-smoke.log
PID_FILE=/tmp/nexaos-session-smoke-python.pid
MAX_RUNTIME_SECONDS=180

log() {
    printf '%s %s\n' "$(date -u '+%Y-%m-%dT%H:%M:%SZ' 2>/dev/null || printf 'time-unavailable')" "$1" >> "$LOG"
}

emit() {
    stage=$1
    display_set=false
    dbus_set=false
    [ -n "\${DISPLAY:-}" ] && display_set=true
    [ -n "\${DBUS_SESSION_BUS_ADDRESS:-}" ] && dbus_set=true
    if [ -e "$DEVICE" ]; then
        printf '{"kind":"event","stage":"%s","display_set":%s,"dbus_session_set":%s}\n' \
            "$stage" "$display_set" "$dbus_set" > "$DEVICE" 2>> "$LOG" || log "report transport failed at $stage"
    else
        log "virtio report device missing at $stage"
    fi
}

write_fallback_report() {
    reason=$1
    if [ -e "$REPORT_SENT" ]; then
        log "Python already recorded a final report; fallback suppressed"
        return
    fi
    log "writing bounded fallback report: $reason"
    if [ -e "$DEVICE" ]; then
        printf '{"kind":"final","schema_version":1,"status":"failed","checks":{"x11_display":false,"dbus_session_bus":false,"lxqt_session":false,"lxqt_panel":false,"welcome_window":false,"welcome_controls":false,"qt_keyboard_activation":false,"task_manager_sampling":false,"qt_mouse_activation":false,"file_browser_dialog":false},"failures":["%s"]}\n' \
            "$reason" > "$DEVICE" 2>> "$LOG" || log "fallback report transport failed"
    else
        log "fallback report unavailable because virtio device is missing"
    fi
}

rm -f "$REPORT_SENT" "$PYTHON_LOG" "$PID_FILE"
log "autostart launcher entered"
emit "launcher_started"
if [ ! -x /usr/bin/python3 ]; then
    log "python interpreter missing"
    emit "python_missing"
    write_fallback_report "python_interpreter_missing"
    exit 127
fi
if [ ! -r "$SCRIPT" ]; then
    log "session smoke script missing or unreadable"
    emit "script_missing"
    write_fallback_report "session_smoke_script_missing"
    exit 126
fi

log "starting session smoke Python"
emit "python_starting"
/usr/bin/python3 -u "$SCRIPT" >> "$LOG" 2>&1 &
python_pid=$!
printf '%s\n' "$python_pid" > "$PID_FILE"
log "python child process created"
emit "python_process_started"

elapsed=0
while kill -0 "$python_pid" 2>/dev/null && [ "$elapsed" -lt "$MAX_RUNTIME_SECONDS" ]; do
    sleep 1
    elapsed=$((elapsed + 1))
done

if kill -0 "$python_pid" 2>/dev/null; then
    log "Python process exceeded bounded runtime"
    kill "$python_pid" 2>/dev/null || true
    sleep 1
    kill -KILL "$python_pid" 2>/dev/null || true
    wait "$python_pid" 2>/dev/null || true
    write_fallback_report "python_process_timeout"
    exit 124
fi

wait "$python_pid"
python_status=$?
log "Python child process exited with status $python_status"
if [ ! -e "$REPORT_SENT" ]; then
    if [ ! -s "$PYTHON_LOG" ]; then
        write_fallback_report "python_exited_before_script_started"
    else
        write_fallback_report "python_exited_without_final_report"
    fi
    exit 1
fi

log "Python final report marker found"
exit "$python_status"
