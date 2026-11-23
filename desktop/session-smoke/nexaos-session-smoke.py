#!/usr/bin/env python3
"""CI-only interactive LXQt session smoke test.

This module is staged into the live image only when NEXAOS_SESSION_SMOKE=1.
It exercises the real desktop session and NexaOS Qt dialogs, then reports a
small, non-sensitive result over a dedicated QEMU virtio-serial port.
"""
from __future__ import annotations

import json
import os
from pathlib import Path
import sys
import time
import traceback

DIAGNOSTIC_LOG = Path("/tmp/nexaos-session-smoke.log")


def diagnostic(message: str) -> None:
    """Write bounded CI-only startup diagnostics without recording secrets."""
    try:
        with DIAGNOSTIC_LOG.open("a", encoding="utf-8") as stream:
            stream.write(message[:1000] + "\n")
            stream.flush()
    except OSError:
        pass


REPORT_DEVICE = Path("/dev/virtio-ports/org.nexaos.session-smoke")
REQUIRED_CHECKS = (
    "x11_display",
    "dbus_session_bus",
    "lxqt_session",
    "lxqt_panel",
    "welcome_window",
    "welcome_controls",
    "qt_keyboard_activation",
    "task_manager_sampling",
    "qt_mouse_activation",
    "file_browser_dialog",
)


def send_report(report: dict) -> None:
    """Send one bounded final JSON line to the host-owned QEMU test socket."""
    report["kind"] = "final"
    if not REPORT_DEVICE.exists():
        raise FileNotFoundError("QEMU session-smoke virtio port is unavailable")
    payload = (json.dumps(report, sort_keys=True, separators=(",", ":")) + "\n").encode("utf-8")
    if len(payload) > 8192:
        raise ValueError("session-smoke report exceeded its size limit")
    descriptor = os.open(REPORT_DEVICE, os.O_WRONLY)
    try:
        written = os.write(descriptor, payload)
        if written != len(payload):
            raise OSError("session-smoke report was not fully written")
    finally:
        os.close(descriptor)
    # The launcher uses this local marker to avoid sending a duplicate fallback
    # report if the guest test has already completed its authoritative write.
    Path("/tmp/nexaos-session-smoke-report-sent").write_text("sent\\n", encoding="ascii")


diagnostic("session-smoke launcher entered Python")
try:
    from PyQt6.QtCore import Qt, QTimer
    from PyQt6.QtTest import QTest
    from PyQt6.QtWidgets import QApplication, QPushButton
    from nexaos_welcome import NexaWelcome
except Exception as error:
    # Report import failures before optional GUI dependencies can prevent
    # the main smoke-test routine from starting. Do not send exception text,
    # paths, environment values, or tracebacks to the host.
    diagnostic("session-smoke import failure: " + type(error).__name__)
    failed_report = {
        "schema_version": 1,
        "status": "failed",
        "checks": {name: False for name in REQUIRED_CHECKS},
        "failures": ["startup_import_error:" + type(error).__name__],
    }
    try:
        send_report(failed_report)
    except Exception as report_error:
        diagnostic("unable to transmit import-failure report: " + type(report_error).__name__)
    raise


def process_exists(expected: str) -> bool:
    """Check process comm names only; never read command lines or environments."""
    try:
        entries = Path("/proc").iterdir()
    except OSError:
        return False
    for entry in entries:
        if not entry.name.isdecimal():
            continue
        try:
            name = (entry / "comm").read_text(encoding="ascii").strip()
        except (OSError, UnicodeError):
            continue
        if name == expected:
            return True
    return False


def main() -> int:
    diagnostic(
        "main entered; DISPLAY set=" + str(bool(os.environ.get("DISPLAY")))
        + "; DBUS session set=" + str(bool(os.environ.get("DBUS_SESSION_BUS_ADDRESS")))
        + "; desktop=" + os.environ.get("XDG_CURRENT_DESKTOP", "unset")[:80]
    )
    checks = {
        "x11_display": bool(os.environ.get("DISPLAY")),
        "dbus_session_bus": bool(os.environ.get("DBUS_SESSION_BUS_ADDRESS")),
        "lxqt_session": process_exists("lxqt-session"),
        "lxqt_panel": process_exists("lxqt-panel"),
        "welcome_window": False,
        "welcome_controls": False,
        "qt_keyboard_activation": False,
        "task_manager_sampling": False,
        "qt_mouse_activation": False,
        "file_browser_dialog": False,
    }
    failures: list[str] = []
    # LXQt starts autostart applications while its remaining session
    # components may still be coming up. Wait briefly, but fail closed.
    deadline = time.monotonic() + 45
    while time.monotonic() < deadline and not all(checks[name] for name in (
        "x11_display", "dbus_session_bus", "lxqt_session", "lxqt_panel"
    )):
        time.sleep(0.5)
        checks["x11_display"] = bool(os.environ.get("DISPLAY"))
        checks["dbus_session_bus"] = bool(os.environ.get("DBUS_SESSION_BUS_ADDRESS"))
        checks["lxqt_session"] = process_exists("lxqt-session")
        checks["lxqt_panel"] = process_exists("lxqt-panel")

    report = {"schema_version": 1, "status": "failed", "checks": checks, "failures": failures}

    # Do not attempt GUI tests when the session prerequisites are missing.
    if not all(checks[name] for name in (
        "x11_display", "dbus_session_bus", "lxqt_session", "lxqt_panel"
    )):
        failures.extend(name for name in (
            "x11_display", "dbus_session_bus", "lxqt_session", "lxqt_panel"
        ) if not checks[name])
        send_report(report)
        return 1

    diagnostic("session prerequisites passed; creating QApplication")
    try:
        app = QApplication(sys.argv[:1])
        window = NexaWelcome()
        window.show()
    except Exception as error:
        diagnostic("GUI startup exception: " + traceback.format_exc())
        report["failures"].append(f"startup_exception:{type(error).__name__}")
        report["status"] = "failed"
        try:
            send_report(report)
        except Exception as report_error:
            diagnostic("unable to transmit GUI-startup failure report: " + type(report_error).__name__)
        return 2
    diagnostic("NexaOS Welcome window shown")

    def run_checks() -> None:
        try:
            QTest.qWait(250)
            checks["welcome_window"] = (
                window.isVisible() and window.width() >= 760 and window.height() >= 520
            )

            buttons = {
                button.text(): button
                for button in window.findChildren(QPushButton)
            }
            expected = {
                "Open Files", "Appearance", "Display", "Keyboard & Mouse",
                "System Settings", "Windows App Preview", "Task Manager",
            }
            checks["welcome_controls"] = expected <= set(buttons)

            task_button = buttons.get("Task Manager")
            if task_button is not None:
                modal_result: dict[str, bool] = {}

                def inspect_task_manager() -> None:
                    dialog = QApplication.activeModalWidget()
                    if dialog is None:
                        modal_result["valid"] = False
                        return
                    timer = getattr(dialog, "timer", None)
                    modal_result["valid"] = (
                        dialog.windowTitle() == "NexaOS Task Manager"
                        and timer is not None
                        and timer.interval() == 1200
                        and timer.isActive()
                    )
                    dialog.close()

                QTimer.singleShot(350, inspect_task_manager)
                task_button.setFocus()
                QTest.keyClick(task_button, Qt.Key.Key_Return)
                # Process the Qt event loop until the scheduled modal inspection
                # runs; reading modal_result immediately races the 350 ms timer.
                QTest.qWait(400)
                checks["qt_keyboard_activation"] = modal_result.get("valid", False)
                checks["task_manager_sampling"] = modal_result.get("valid", False)

            files_button = buttons.get("Open Files")
            if files_button is not None:
                modal_result = {}

                def inspect_file_browser() -> None:
                    dialog = QApplication.activeModalWidget()
                    if dialog is None:
                        modal_result["valid"] = False
                        return
                    modal_result["valid"] = (
                        dialog.windowTitle() == "NexaOS Files"
                        and getattr(dialog, "table", None) is not None
                        and getattr(dialog, "new_folder_button", None) is not None
                        and dialog.new_folder_button.isEnabled()
                        and dialog.current == Path.home().resolve()
                    )
                    dialog.close()

                QTimer.singleShot(350, inspect_file_browser)
                QTest.mouseClick(files_button, Qt.MouseButton.LeftButton)
                # Allow the same scheduled inspection to run deterministically.
                QTest.qWait(400)
                checks["qt_mouse_activation"] = modal_result.get("valid", False)
                checks["file_browser_dialog"] = modal_result.get("valid", False)

            for name in REQUIRED_CHECKS:
                if not checks[name]:
                    failures.append(name)

        except Exception as error:  # report the failure instead of silently timing out
            diagnostic("GUI check exception: " + traceback.format_exc())
            failures.append(f"exception:{type(error).__name__}")
        finally:
            report["status"] = "passed" if not failures and all(checks.values()) else "failed"
            diagnostic("GUI checks finished; status=" + report["status"])
            window.close()
            try:
                send_report(report)
            except Exception:
                # Without the host report channel, the QEMU test must fail closed.
                app.exit(2)
                return
            app.exit(0 if report["status"] == "passed" else 1)

    QTimer.singleShot(1200, run_checks)
    return app.exec()


if __name__ == "__main__":
    raise SystemExit(main())
