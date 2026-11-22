#!/usr/bin/env python3
"""Send a bounded key event to a local QEMU human-monitor socket."""
from __future__ import annotations

import argparse
from pathlib import Path
import socket
import time


def send_key(socket_path: Path, key: str, timeout_seconds: int = 30) -> None:
    """Wait for QEMU's monitor and issue one allow-listed key event."""
    if key != "ret":
        raise ValueError("only the Enter key is permitted by this boot helper")
    deadline = time.monotonic() + timeout_seconds
    last_error: OSError | None = None
    while time.monotonic() < deadline:
        try:
            with socket.socket(socket.AF_UNIX, socket.SOCK_STREAM) as connection:
                connection.settimeout(2)
                connection.connect(str(socket_path))
                # Consume the monitor greeting before issuing a command.
                connection.recv(4096)
                connection.sendall(f"sendkey {key}\n".encode("ascii"))
                try:
                    connection.recv(4096)
                except socket.timeout:
                    pass
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
    print("Sent Enter to the QEMU live boot menu.", flush=True)
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
