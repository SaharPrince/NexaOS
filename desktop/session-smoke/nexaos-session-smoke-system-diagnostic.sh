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
autologin_config=false; panel_config=false; global_autostart_entry=false; user_runtime_dir=false
user_login_session=false; user_session_active=false; user_session_class_user=false
sddm_pam_file=false; sddm_pam_systemd=false; common_session_pam_systemd=false
lxqt_session_process=false; lxqt_panel_process=false; user_lxqt_session_process=false
user_lxqt_panel_process=false; xorg_process=false; user_xorg_process=false
sddm_auth_failure=false
[ "$(systemctl is-active sddm.service 2>/dev/null || true)" = "active" ] && sddm_active=true
[ "$(systemctl is-active systemd-logind.service 2>/dev/null || true)" = "active" ] && logind_active=true
[ -r /usr/share/xsessions/lxqt.desktop ] && session_file=true
[ "$USER_UID" != "0" ] && live_user=true
[ -r /etc/sddm.conf.d/10-nexaos-live-autologin.conf ] && autologin_config=true
[ -r /home/user/.config/lxqt/panel.conf ] && panel_config=true
[ -r /etc/xdg/autostart/nexaos-session-smoke.desktop ] && global_autostart_entry=true
[ -r /etc/pam.d/sddm ] && sddm_pam_file=true
if [ -r /etc/pam.d/sddm ] && grep -Eq 'pam_systemd\.so' /etc/pam.d/sddm; then sddm_pam_systemd=true; fi
if [ -r /etc/pam.d/common-session ] && grep -Eq 'pam_systemd\.so' /etc/pam.d/common-session; then common_session_pam_systemd=true; fi

# The display manager, LXQt session and XDG autostart may settle after graphical.target.
# Wait for Python to enter the test (or exit) rather than stopping at the launcher breadcrumb.
attempt=0
while [ "$attempt" -lt 190 ]; do
    [ -d "/run/user/$USER_UID" ] && user_runtime_dir=true
    sessions="$(loginctl list-sessions --no-legend 2>/dev/null || true)"
    session_id="$(printf '%s\n' "$sessions" | awk -v u="$USER_NAME" '$3 == u { print $1; exit }')"
    if [ -n "$session_id" ]; then
        user_login_session=true
        session_active="$(loginctl show-session "$session_id" -p Active --value 2>/dev/null || true)"
        [ "$session_active" = "yes" ] && user_session_active=true
        session_class="$(loginctl show-session "$session_id" -p Class --value 2>/dev/null || true)"
        [ "$session_class" = "user" ] && user_session_class_user=true
    fi

    lxqt_session_process=false; lxqt_panel_process=false
    user_lxqt_session_process=false; user_lxqt_panel_process=false
    xorg_process=false; user_xorg_process=false
    for proc in /proc/[0-9]*; do
        [ -r "$proc/comm" ] || continue
        IFS= read -r comm < "$proc/comm" || true
        case "$comm" in
            lxqt-session|lxqt-panel|Xorg|Xwayland)
                [ -r "$proc/status" ] || continue
                proc_uid="$(awk '$1 == "Uid:" { print $2; exit }' "$proc/status")"
                case "$comm" in
                    lxqt-session)
                        lxqt_session_process=true
                        [ "$proc_uid" = "$USER_UID" ] && user_lxqt_session_process=true
                        ;;
                    lxqt-panel)
                        lxqt_panel_process=true
                        [ "$proc_uid" = "$USER_UID" ] && user_lxqt_panel_process=true
                        ;;
                    Xorg|Xwayland)
                        xorg_process=true
                        [ "$proc_uid" = "$USER_UID" ] && user_xorg_process=true
                        ;;
                esac
                ;;
        esac
    done
    launcher_entered=false
    python_main_entered=false
    python_import_failure=false
    gui_checks_finished=false
    python_process_exited=false
    if grep -Fq "autostart launcher entered" /tmp/nexaos-session-smoke-launcher.log 2>/dev/null; then
        launcher_entered=true
    fi
    grep -Fq "main entered;" /tmp/nexaos-session-smoke.log 2>/dev/null && python_main_entered=true
    grep -Fq "session-smoke import failure:" /tmp/nexaos-session-smoke.log 2>/dev/null && python_import_failure=true
    grep -Fq "GUI checks finished;" /tmp/nexaos-session-smoke.log 2>/dev/null && gui_checks_finished=true
    grep -Fq "bounded Python process exited with status" /tmp/nexaos-session-smoke-launcher.log 2>/dev/null && python_process_exited=true
    if [ "$launcher_entered" = true ] && { [ "$gui_checks_finished" = true ] || [ "$python_import_failure" = true ] || [ "$python_process_exited" = true ]; }; then
        break
    fi
    sleep 1
    attempt=$((attempt + 1))
done
if command -v journalctl >/dev/null 2>&1; then
    if journalctl -b -u sddm --no-pager 2>/dev/null | grep -Eiq 'authentication failure|authentication failed|failed to authenticate|auth.*failed'; then
        sddm_auth_failure=true
    fi
fi
launcher_log_exists=false; launcher_entered=false; python_starting=false; python_process_created=false
python_process_exited=false; python_timed_out=false; python_log_exists=false
python_main_entered=false; python_import_failure=false; gui_checks_finished=false
[ -r /tmp/nexaos-session-smoke-launcher.log ] && launcher_log_exists=true
[ -r /tmp/nexaos-session-smoke.log ] && python_log_exists=true
grep -Fq "autostart launcher entered" /tmp/nexaos-session-smoke-launcher.log 2>/dev/null && launcher_entered=true
grep -Fq "starting session smoke Python" /tmp/nexaos-session-smoke-launcher.log 2>/dev/null && python_starting=true
grep -Fq "bounded Python process created" /tmp/nexaos-session-smoke-launcher.log 2>/dev/null && python_process_created=true
grep -Fq "bounded Python process exited with status" /tmp/nexaos-session-smoke-launcher.log 2>/dev/null && python_process_exited=true
grep -Fq "Python process exceeded bounded runtime" /tmp/nexaos-session-smoke-launcher.log 2>/dev/null && python_timed_out=true
grep -Fq "main entered;" /tmp/nexaos-session-smoke.log 2>/dev/null && python_main_entered=true
grep -Fq "session-smoke import failure:" /tmp/nexaos-session-smoke.log 2>/dev/null && python_import_failure=true
grep -Fq "GUI checks finished;" /tmp/nexaos-session-smoke.log 2>/dev/null && gui_checks_finished=true
log "diagnostic service completed its bounded session observation"
if [ -e "$DEVICE" ]; then
    printf '{"kind":"event","stage":"system_diagnostic","sddm_active":%s,"logind_active":%s,"lxqt_session_file":%s,"live_user_exists":%s,"autologin_config":%s,"user_panel_config":%s,"global_autostart_entry":%s,"user_runtime_dir":%s,"user_login_session":%s,"user_session_active":%s,"user_session_class_user":%s,"sddm_pam_file":%s,"sddm_pam_systemd":%s,"common_session_pam_systemd":%s,"lxqt_session_process":%s,"lxqt_panel_process":%s,"user_lxqt_session_process":%s,"user_lxqt_panel_process":%s,"xorg_process":%s,"user_xorg_process":%s,"sddm_auth_failure":%s,"launcher_log_exists":%s,"launcher_entered":%s,"python_starting":%s,"python_process_created":%s,"python_process_exited":%s,"python_timed_out":%s,"python_log_exists":%s,"python_main_entered":%s,"python_import_failure":%s,"gui_checks_finished":%s}\n' \
        "$sddm_active" "$logind_active" "$session_file" "$live_user" "$autologin_config" "$panel_config" "$global_autostart_entry" "$user_runtime_dir" "$user_login_session" "$user_session_active" "$user_session_class_user" "$sddm_pam_file" "$sddm_pam_systemd" "$common_session_pam_systemd" "$lxqt_session_process" "$lxqt_panel_process" "$user_lxqt_session_process" "$user_lxqt_panel_process" "$xorg_process" "$user_xorg_process" "$sddm_auth_failure" "$launcher_log_exists" "$launcher_entered" "$python_starting" "$python_process_created" "$python_process_exited" "$python_timed_out" "$python_log_exists" "$python_main_entered" "$python_import_failure" "$gui_checks_finished" > "$DEVICE" || log "virtio report write failed"
else
    log "virtio report device unavailable after module load and wait"
fi
exit 0
