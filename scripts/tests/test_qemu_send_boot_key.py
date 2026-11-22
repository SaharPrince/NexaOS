from __future__ import annotations

from pathlib import Path
import socket
import tempfile
import threading
import unittest

from qemu_send_boot_key import send_key


class QemuBootKeyTests(unittest.TestCase):
    def test_sends_only_the_default_enter_key(self) -> None:
        with tempfile.TemporaryDirectory() as directory:
            socket_path = Path(directory) / "monitor.sock"
            received: list[bytes] = []

            def monitor() -> None:
                with socket.socket(socket.AF_UNIX, socket.SOCK_STREAM) as server:
                    server.bind(str(socket_path))
                    server.listen(1)
                    connection, _ = server.accept()
                    with connection:
                        connection.sendall(b"QEMU monitor ready\n(qemu) ")
                        received.append(connection.recv(128))
                        connection.sendall(b"(qemu) ")

            thread = threading.Thread(target=monitor)
            thread.start()
            send_key(socket_path, "ret", timeout_seconds=3)
            thread.join(timeout=3)

            self.assertFalse(thread.is_alive())
            self.assertEqual(received, [b"sendkey ret\n"])

    def test_rejects_other_keys(self) -> None:
        with self.assertRaisesRegex(ValueError, "only the Enter key"):
            send_key(Path("unused.sock"), "ctrl-alt-delete", timeout_seconds=0)


if __name__ == "__main__":
    unittest.main()
