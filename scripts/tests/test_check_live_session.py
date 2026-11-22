import json
import unittest

from check_live_session import REQUIRED_CHECKS, validate_report, receive_report


class SessionReportValidationTests(unittest.TestCase):
    def passing_report(self):
        return {
            "schema_version": 1,
            "status": "passed",
            "checks": {name: True for name in REQUIRED_CHECKS},
            "failures": [],
        }

    def test_accepts_complete_passing_report(self):
        self.assertEqual(validate_report(self.passing_report()), [])

    def test_rejects_missing_required_check(self):
        report = self.passing_report()
        report["checks"]["lxqt_panel"] = False
        self.assertIn(
            "required check failed: lxqt_panel",
            validate_report(report),
        )

    def test_rejects_unknown_schema(self):
        report = self.passing_report()
        report["schema_version"] = 2
        self.assertIn("unsupported schema_version", validate_report(report))

    def test_rejects_guest_failure_even_when_checks_are_true(self):
        report = self.passing_report()
        report["status"] = "failed"
        report["failures"] = ["synthetic failure"]
        errors = validate_report(report)
        self.assertIn("guest session test did not report passed status", errors)
        self.assertTrue(any("guest reported failures" in item for item in errors))

    def test_rejects_non_object_and_malformed_checks(self):
        self.assertTrue(validate_report([]))
        report = self.passing_report()
        report["checks"] = []
        self.assertIn("checks must be a JSON object", validate_report(report))

    def test_decodes_launcher_events_before_final_report(self):
        from check_live_session import decode_report_stream

        final = self.passing_report()
        final["kind"] = "final"
        stream = (
            b'{"kind":"event","stage":"launcher_started"}\n'
            b'{"kind":"event","stage":"python_starting"}\n'
            + (json.dumps(final) + "\n").encode("utf-8")
        )
        report = decode_report_stream(stream)
        self.assertEqual(report["diagnostics"], [
            {"kind": "event", "stage": "launcher_started"},
            {"kind": "event", "stage": "python_starting"},
        ])
        self.assertEqual(validate_report(report), [])

    def test_report_reader_writes_ready_marker_after_socket_connect(self):
        import socket
        import tempfile
        import threading
        from pathlib import Path
        from check_live_session import receive_report

        with tempfile.TemporaryDirectory() as directory:
            socket_path = Path(directory) / "report.sock"
            ready_path = Path(directory) / "connected"
            listener = socket.socket(socket.AF_UNIX, socket.SOCK_STREAM)
            listener.bind(str(socket_path))
            listener.listen(1)

            def send_report():
                connection, _ = listener.accept()
                with connection:
                    report = self.passing_report()
                    report["kind"] = "final"
                    connection.sendall((json.dumps(report) + "\\n").encode())
                listener.close()

            sender = threading.Thread(target=send_report)
            sender.start()
            result = receive_report(socket_path, 5, ready_path)
            sender.join(timeout=5)
            self.assertTrue(ready_path.is_file())
            self.assertEqual(ready_path.read_text(encoding="ascii"), "connected\\n")
            self.assertEqual(result["status"], "passed")

    def test_rejects_stream_without_final_report(self):
        from check_live_session import decode_report_stream

        with self.assertRaisesRegex(ValueError, "without a final report"):
            decode_report_stream(b'{"kind":"event","stage":"launcher_started"}\n')


if __name__ == "__main__":
    unittest.main()
