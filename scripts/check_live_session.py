#!/usr/bin/env python3
"""Receive and validate the CI-only LXQt session report from QEMU."""
from __future__ import annotations

import argparse
import json
from pathlib import Path
import socket
import time
from typing import Any

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
MAX_REPORT_BYTES = 8192


def validate_report(report: Any) -> list[str]:
    """Return validation errors for a versioned, bounded session report."""
    errors: list[str] = []
    if not isinstance(report, dict):
        return ["report must be a JSON object"]
    if report.get("schema_version") != 1:
        errors.append("unsupported schema_version")
    if report.get("status") != "passed":
        errors.append("guest session test did not report passed status")
    checks = report.get("checks")
    if not isinstance(checks, dict):
        errors.append("checks must be a JSON object")
    else:
        for name in REQUIRED_CHECKS:
            if checks.get(name) is not True:
                errors.append(f"required check failed: {name}")
    failures = report.get("failures")
    if not isinstance(failures, list):
        errors.append("failures must be a JSON array")
    elif failures:
        errors.append("guest reported failures: " + ", ".join(str(item)[:120] for item in failures[:10]))
    return errors


def receive_report(socket_path: Path, timeout_seconds: int) -> dict:
    """Connect to QEMU's local UNIX socket and receive exactly one JSON line."""
    deadline = time.monotonic() + timeout_seconds
    connection: socket.socket | None = None
    while time.monotonic() < deadline:
        try:
            connection = socket.socket(socket.AF_UNIX, socket.SOCK_STREAM)
            connection.settimeout(min(5.0, max(0.1, deadline - time.monotonic())))
            connection.connect(str(socket_path))
            break
        except (FileNotFoundError, ConnectionRefusedError, TimeoutError, OSError):
            if connection is not None:
                connection.close()
                connection = None
            time.sleep(0.5)
    if connection is None:
        raise TimeoutError("QEMU session-smoke socket did not become available")

    data = bytearray()
    try:
        connection.settimeout(max(0.1, deadline - time.monotonic()))
        while b"\n" not in data:
            try:
                chunk = connection.recv(1024)
            except TimeoutError as error:
                raise TimeoutError(
                    "connected to QEMU, but the guest sent no session report before the deadline"
                ) from error
            if not chunk:
                raise ConnectionError("QEMU closed the session-smoke channel before reporting")
            data.extend(chunk)
            if len(data) > MAX_REPORT_BYTES:
                raise ValueError("session-smoke report exceeded its size limit")
    finally:
        connection.close()

    line = bytes(data).split(b"\n", 1)[0]
    return json.loads(line.decode("utf-8"))


def main() -> int:
    parser = argparse.ArgumentParser()
    parser.add_argument("socket_path", type=Path)
    parser.add_argument("report_path", type=Path)
    parser.add_argument("--timeout", type=int, default=210)
    args = parser.parse_args()
    try:
        report = receive_report(args.socket_path, args.timeout)
    except Exception as error:
        print(f"Desktop session validation failed: {error}", flush=True)
        return 1
    args.report_path.parent.mkdir(parents=True, exist_ok=True)
    args.report_path.write_text(
        json.dumps(report, indent=2, sort_keys=True) + "\n",
        encoding="utf-8",
    )
    errors = validate_report(report)
    if errors:
        print("Desktop session validation failed: " + "; ".join(errors), flush=True)
        return 1
    print("LXQt session and NexaOS GUI interaction checks passed.", flush=True)
    for name in REQUIRED_CHECKS:
        print(f"  PASS {name}", flush=True)
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
