#!/usr/bin/env python3
"""Receive and validate the CI-only LXQt session report from QEMU."""
from __future__ import annotations

import argparse
import json
from pathlib import Path
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


def receive_report(stream_path: Path, timeout_seconds: int) -> dict:
    """Poll QEMU's guest-output file until the final JSONL record arrives."""
    deadline = time.monotonic() + timeout_seconds
    data = b""
    while time.monotonic() < deadline:
        try:
            data = stream_path.read_bytes()
        except FileNotFoundError:
            data = b""
        if len(data) > MAX_REPORT_BYTES:
            raise ValueError("session-smoke report stream exceeded its size limit")
        final_marker = data.find(b'"kind":"final"')
        if final_marker >= 0 and b"\n" in data[final_marker:]:
            return decode_report_stream(data)
        time.sleep(0.25)
    partial = data.decode("utf-8", errors="replace")[-1000:]
    raise TimeoutError(
        "QEMU report file did not receive a complete final guest report; "
        "diagnostic stream tail=" + partial
    )


def main() -> int:
    parser = argparse.ArgumentParser()
    parser.add_argument("stream_path", type=Path)
    parser.add_argument("report_path", type=Path)
    parser.add_argument("--timeout", type=int, default=210)
    args = parser.parse_args()
    try:
        report = receive_report(args.stream_path, args.timeout)
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
