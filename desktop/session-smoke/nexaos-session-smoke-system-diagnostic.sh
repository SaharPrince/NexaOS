#!/bin/sh
# CI-only SDDM/LXQt startup diagnostic. Emits bounded state only; never logs
# environment values, command lines, credentials, or user file contents.
set -u
DEVICE=/dev/virtio-ports/org.nexaos.session-smoke
LOG=/tmp/nexaos-session-smoke-system.log
USER_NAME=user
USER_UID="$(id -u "$USER_NAME" 2>/dev/null || printf '0')"
log() { printf '%s\n' "$1" >> "$LOG"; }
if command -v modprobe >/dev/null 2>&1; then modprobe virtio_console >> "$LOG" 2>&1 || true; fi
attempt=0
while [ "$attempt" -lt 20 ] && [ ! -e "$DEVICE" ]; do sleep 1; attempt=$((attempt + 1)); done
sddm_active=false; logind_active=false; session_file=false; live_user=false
autologin_config=false; panel_config=false; user_autostart=false; user_runtime_dir=false
user_login_session=false; user_session_active=false; user_session_class_user=false
sddm_pam_file=false; sddm_pam_systemd=false; common_session_pam_systemd=false
lxqt_session_process=false; lxqt_panel_process=false; xorg_process=false
user_xorg_process=false; sddm_auth_failure=false
[ "$(systemctl is-active sddm.service 2>/dev/null || true)" = "active" ] && sddm_active=true
[ "$(systemctl is-active systemd-logind.service 2>/dev/null || true)" = "active" ] && logind_active=true
[ -r /usr/share/xsessions/lxqt.desktop ] && session_file=true
[ "$USER_UID" != "0" ] && live_user=true
[ -r /etc/sddm.conf.d/10-nexaos-live-autologin.conf ] && autologin_config=true
[ -r /home/user/.config/lxqt/panel.conf ] && panel_config=true
[ -r /home/user/.config/autostart/nexaos-session-smoke.desktop ] && user_autostart=true
[ -d "/run/user/$USER_UID" ] && user_runtime_dir=true
[ -r /etc/pam.d/sddm ] && sddm_pam_file=true
if [ -r /etc/pam.d/sddm ] && grep -Eq 'pam_systemd\.so' /etc/pam.d/sddm; then sddm_pam_systemd=true; fi
if [ -r /etc/pam.d/common-session ] && grep -Eq 'pam_systemd\.so' /etc/pam.d/common-session; then common_session_pam_systemd=true; fi
# Wait up to 35 seconds for SDDM to establish the live user's login session.
attempt=0
while [ "$attempt" -lt 35 ]; do
    sessions="$(loginctl list-sessions --no-legend 2>/dev/null || true)"
    if printf '%s\n' "$sessions" | awk -v u="$USER_NAME" '$3 == u { found=1 } END { exit !found }'; then
        user_login_session=true; break
    fi
    sleep 1; attempt=$((attempt + 1))
done
session_id="$(loginctl list-sessions --no-legend 2>/dev/null | awk -v u="$USER_NAME" '$3 == u { print $1; exit }')"
if [ -n "$session_id" ]; then
    # Active is the explicit logind boolean; State is not a reliable synonym.
    session_active="$(loginctl show-session "$session_id" -p Active --value 2>/dev/null || true)"
    [ "$session_active" = "yes" ] && user_session_active=true
    session_class="$(loginctl show-session "$session_id" -p Class --value 2>/dev/null || true)"
    [ "$session_class" = "user" ] && user_session_class_user=true
fi
# Read /proc comm names; do not depend on pidof/procps. Attribute X to live user.
for proc in /proc/[0-9]*; do
    [ -r "$proc/comm" ] || continue
    IFS= read -r comm < "$proc/comm" || true
    case "$comm" in
        lxqt-session) lxqt_session_process=true ;;
        lxqt-panel) lxqt_panel_process=true ;;
        Xorg|Xwayland)
            xorg_process=true
            if [ -r "$proc/status" ] && awk -v uid="$USER_UID" '$1 == "Uid:" && $2 == uid { found=1 } END { exit !found }' "$proc/status"; then
                user_xorg_process=true
            fi
            ;;
    esac
done
if command -v journalctl >/dev/null 2>&1; then
    if journalctl -b -u sddm --no-pager 2>/dev/null | grep -Eiq 'authentication failure|authentication failed|failed to authenticate|auth.*failed'; then
        sddm_auth_failure=true
    fi
fi
log "diagnostic service ran"
if [ -e "$DEVICE" ]; then
    printf '{"kind":"event","stage":"system_diagnostic","sddm_active":%s,"logind_active":%s,"lxqt_session_file":%s,"live_user_exists":%s,"autologin_config":%s,"user_panel_config":%s,"user_autostart_entry":%s,"user_runtime_dir":%s,"user_login_session":%s,"user_session_active":%s,"user_session_class_user":%s,"sddm_pam_file":%s,"sddm_pam_systemd":%s,"common_session_pam_systemd":%s,"lxqt_session_process":%s,"lxqt_panel_process":%s,"xorg_process":%s,"user_xorg_process":%s,"sddm_auth_failure":%s}\n' \
        "$sddm_active" "$logind_active" "$session_file" "$live_user" "$autologin_config" "$panel_config" "$user_autostart" "$user_runtime_dir" "$user_login_session" "$user_session_active" "$user_session_class_user" "$sddm_pam_file" "$sddm_pam_systemd" "$common_session_pam_systemd" "$lxqt_session_process" "$lxqt_panel_process" "$xorg_process" "$user_xorg_process" "$sddm_auth_failure" > "$DEVICE" || log "virtio report write failed"
else
    log "virtio report device unavailable after module load and wait"
fi
exit 0
