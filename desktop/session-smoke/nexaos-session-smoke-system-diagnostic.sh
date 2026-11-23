#!/bin/sh
# CI-only boot diagnostic. Reports bounded booleans; never exports environment
# values, command lines, credentials, or user data.
set -u

DEVICE=/dev/virtio-ports/org.nexaos.session-smoke
LOG=/tmp/nexaos-session-smoke-system.log

log() {
    printf '%s\n' "$1" >> "$LOG"
}

if command -v modprobe >/dev/null 2>&1; then
    modprobe virtio_console >> "$LOG" 2>&1 || true
fi

attempt=0
while [ "$attempt" -lt 20 ] && [ ! -e "$DEVICE" ]; do
    sleep 1
    attempt=$((attempt + 1))
done

sddm_active=false
session_file=false
live_user=false
autologin_config=false
panel_config=false
[ "$(systemctl is-active sddm.service 2>/dev/null || true)" = "active" ] && sddm_active=true
[ -r /usr/share/xsessions/lxqt.desktop ] && session_file=true
id user >/dev/null 2>&1 && live_user=true
[ -r /etc/sddm.conf.d/10-nexaos-live-autologin.conf ] && autologin_config=true
[ -r /home/user/.config/lxqt/panel.conf ] && panel_config=true
user_autostart=false
lxqt_session_process=false
lxqt_panel_process=false
[ -r /home/user/.config/autostart/nexaos-session-smoke.desktop ] && user_autostart=true
sleep 5
command -v pidof >/dev/null 2>&1 && pidof lxqt-session >/dev/null 2>&1 && lxqt_session_process=true
command -v pidof >/dev/null 2>&1 && pidof lxqt-panel >/dev/null 2>&1 && lxqt_panel_process=true

log "diagnostic service ran"
if [ -e "$DEVICE" ]; then
    printf '{"kind":"event","stage":"system_diagnostic","sddm_active":%s,"lxqt_session_file":%s,"live_user_exists":%s,"autologin_config":%s,"user_panel_config":%s,"user_autostart_entry":%s,"lxqt_session_process":%s,"lxqt_panel_process":%s}\n' \
        "$sddm_active" "$session_file" "$live_user" "$autologin_config" "$panel_config" "$user_autostart" "$lxqt_session_process" "$lxqt_panel_process" > "$DEVICE" || log "virtio report write failed"
else
    log "virtio report device unavailable after module load and wait"
fi
exit 0
