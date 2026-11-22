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


def decode_report_stream(data: bytes) -> dict:
    """Decode diagnostic JSONL events followed by one authoritative final report."""
    events: list[dict] = []
    final_report: dict | None = None
    for raw_line in data.splitlines():
        if not raw_line:
            continue
        item = json.loads(raw_line.decode("utf-8"))
        if not isinstance(item, dict):
            raise ValueError("session-smoke stream records must be JSON objects")
        if item.get("kind") == "final":
            final_report = item
            break
        if item.get("kind") == "event":
            events.append(item)
        else:
            raise ValueError("unknown session-smoke stream record")
    if final_report is None:
        raise ValueError("QEMU closed the session-smoke channel without a final report")
    final_report["diagnostics"] = events[-32:]
    return final_report


def receive_report(
    socket_path: Path,
    timeout_seconds: int,
    ready_file: Path | None = None,
) -> dict:
    """Connect to QEMU's local UNIX socket and receive bounded JSONL records."""
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

    if ready_file is not None:
        ready_file.parent.mkdir(parents=True, exist_ok=True)
        ready_file.write_text("connected\n", encoding="ascii")

    data = bytearray()
    try:
        connection.settimeout(max(0.1, deadline - time.monotonic()))
        final_marker = data.find(b'"kind":"final"')
        while final_marker < 0 or b"\n" not in data[final_marker:]:
            try:
                chunk = connection.recv(1024)
            except TimeoutError as error:
                partial = bytes(data).decode("utf-8", errors="replace")[-1000:]
                raise TimeoutError(
                    "connected to QEMU, but no final guest report arrived; "
                    "diagnostic stream tail=" + partial
                ) from error
            if not chunk:
                break
            data.extend(chunk)
            final_marker = data.find(b'"kind":"final"')
            if len(data) > MAX_REPORT_BYTES:
                raise ValueError("session-smoke report exceeded its size limit")
        return decode_report_stream(bytes(data))
    finally:
        connection.close()


def main() -> int:
    parser = argparse.ArgumentParser()
    parser.add_argument("socket_path", type=Path)
    parser.add_argument("report_path", type=Path)
    parser.add_argument("--timeout", type=int, default=210)
    parser.add_argument(
        "--ready-file",
        type=Path,
        help="write this marker after the QEMU report socket is connected",
    )
    args = parser.parse_args()
    try:
        report = receive_report(args.socket_path, args.timeout, args.ready_file)
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
