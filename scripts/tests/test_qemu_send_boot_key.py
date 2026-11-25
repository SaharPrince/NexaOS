from __future__ import annotations

from pathlib import Path
import socket
import tempfile
import threading
import unittest

from qemu_send_boot_key import send_key


class QemuBootKeyTests(unittest.TestCase):
    def _run_monitor(self, response: bytes) -> tuple[Path, list[bytes], threading.Thread, tempfile.TemporaryDirectory]:
        directory = tempfile.TemporaryDirectory()
        socket_path = Path(directory.name) / "monitor.sock"
        received: list[bytes] = []

        def monitor() -> None:
            with socket.socket(socket.AF_UNIX, socket.SOCK_STREAM) as server:
                server.bind(str(socket_path))
                server.listen(1)
                connection, _ = server.accept()
                with connection:
                    # Deliberately split the greeting to exercise prompt framing.
                    connection.sendall(b"QEMU monitor ready\r\n(qe")
                    connection.sendall(b"mu) ")
                    received.append(connection.recv(128))
                    connection.sendall(response)

        thread = threading.Thread(target=monitor)
        thread.start()
        return socket_path, received, thread, directory

    def test_sends_only_enter_and_waits_for_complete_monitor_prompts(self) -> None:
        socket_path, received, thread, directory = self._run_monitor(b"\r\n(qemu) ")
        try:
            send_key(socket_path, "ret", timeout_seconds=3)
            thread.join(timeout=3)
            self.assertFalse(thread.is_alive())
            self.assertEqual(received, [b"sendkey ret\n"])
        finally:
            directory.cleanup()

    def test_rejects_monitor_command_errors(self) -> None:
        socket_path, _received, thread, directory = self._run_monitor(
            b"unknown command: sendkey\r\n(qemu) "
        )
        try:
            with self.assertRaisesRegex(RuntimeError, "rejected sendkey"):
                send_key(socket_path, "ret", timeout_seconds=3)
            thread.join(timeout=3)
            self.assertFalse(thread.is_alive())
        finally:
            directory.cleanup()

    def test_rejects_other_keys(self) -> None:
        with self.assertRaisesRegex(ValueError, "only the Enter key"):
            send_key(Path("unused.sock"), "ctrl-alt-delete", timeout_seconds=0)


if __name__ == "__main__":
    unittest.main()
