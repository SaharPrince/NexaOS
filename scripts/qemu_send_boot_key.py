#!/usr/bin/env python3
"""Send and verify one bounded key event through a local QEMU HMP socket."""
from __future__ import annotations

import argparse
from pathlib import Path
import socket
import time


PROMPT = b"(qemu) "
MAX_MONITOR_RESPONSE = 16 * 1024


def _read_until_prompt(connection: socket.socket, deadline: float) -> bytes:
    """Read a complete HMP response, not merely one arbitrary socket chunk."""
    data = bytearray()
    while not data.endswith(PROMPT):
        remaining = deadline - time.monotonic()
        if remaining <= 0:
            raise TimeoutError("QEMU monitor did not return its command prompt")
        connection.settimeout(min(2.0, remaining))
        try:
            chunk = connection.recv(4096)
        except socket.timeout as error:
            raise TimeoutError("timed out waiting for QEMU monitor prompt") from error
        if not chunk:
            raise ConnectionError("QEMU monitor closed before returning its prompt")
        data.extend(chunk)
        if len(data) > MAX_MONITOR_RESPONSE:
            raise ValueError("QEMU monitor response exceeded its size limit")
    return bytes(data)


def send_key(socket_path: Path, key: str, timeout_seconds: int = 30) -> None:
    """Wait for QEMU HMP readiness, issue only Enter, and verify its response."""
    if key != "ret":
        raise ValueError("only the Enter key is permitted by this boot helper")
    deadline = time.monotonic() + timeout_seconds
    last_error: OSError | None = None
    while time.monotonic() < deadline:
        try:
            with socket.socket(socket.AF_UNIX, socket.SOCK_STREAM) as connection:
                connection.connect(str(socket_path))
                # HMP can split its greeting across multiple socket reads.
                _read_until_prompt(connection, deadline)
                connection.sendall(b"sendkey ret\n")
                response = _read_until_prompt(connection, deadline)
                lowered = response.lower()
                if b"unknown command" in lowered or b"error:" in lowered:
                    raise RuntimeError(
                        "QEMU monitor rejected sendkey ret: "
                        + response.decode("utf-8", errors="replace")[-500:]
                    )
                return
        except (FileNotFoundError, ConnectionRefusedError, TimeoutError, OSError) as error:
            last_error = error
            time.sleep(0.25)
    raise TimeoutError(f"QEMU monitor did not become ready: {last_error}")


def main() -> int:
    parser = argparse.ArgumentParser()
    parser.add_argument("socket_path", type=Path)
    parser.add_argument("key", choices=("ret",))
    parser.add_argument("--timeout", type=int, default=30)
    args = parser.parse_args()
    send_key(args.socket_path, args.key, args.timeout)
    print("QEMU monitor accepted the Enter command.", flush=True)
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
