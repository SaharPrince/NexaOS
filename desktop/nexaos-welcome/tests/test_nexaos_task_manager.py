import tempfile
import unittest
from unittest.mock import patch
from pathlib import Path

from PyQt6.QtWidgets import QApplication
from nexaos_task_manager import (
    NexaTaskManager,
    ProcfsCollector,
    SystemSample,
    parse_cpu_line,
    parse_memory_info,
    parse_process_stat,
)


def make_stat(pid=321, name="worker (render)", user=5, system=2, start=77, rss=10):
    fields = ["0"] * 22
    fields[0] = "S"
    fields[11] = str(user)
    fields[12] = str(system)
    fields[19] = str(start)
    fields[21] = str(rss)
    return f"{pid} ({name}) " + " ".join(fields) + "\n"


class FakeClock:
    def __init__(self):
        self.value = 10.0

    def __call__(self):
        return self.value

    def advance(self, amount=1.0):
        self.value += amount


class TaskManagerDisplayTests(unittest.TestCase):
    @classmethod
    def setUpClass(cls):
        cls.app = QApplication.instance() or QApplication([])

    def test_sampling_timer_starts_and_stops_with_window_lifecycle(self):
        class StaticCollector:
            def snapshot(self):
                return (
                    SystemSample(
                        None, 1024 * 1024, 512 * 1024, 0, 0,
                        60.0, "Test CPU", 2, "test-kernel",
                    ),
                    [],
                )

        manager = NexaTaskManager()
        manager.collector = StaticCollector()
        self.assertFalse(manager.timer.isActive())

        manager.show()
        self.app.processEvents()
        self.assertTrue(manager.timer.isActive())

        manager.hide()
        self.app.processEvents()
        self.assertFalse(manager.timer.isActive())

        manager.show()
        self.app.processEvents()
        self.assertTrue(manager.timer.isActive())

        manager.close()
        self.app.processEvents()
        self.assertFalse(manager.timer.isActive())

    def test_failed_refresh_clears_stale_system_metrics(self):
        class FailingCollector:
            def snapshot(self):
                raise OSError("synthetic procfs failure")

        manager = NexaTaskManager()
        manager.cpu_label.setText("CPU usage: 88.0%")
        manager.cpu_bar.setValue(88)
        manager.memory_label.setText("Memory: old sample")
        manager.memory_bar.setValue(72)
        manager.swap_label.setText("Swap: old sample")
        manager.swap_bar.setValue(20)
        manager.system_label.setText("Old CPU information")
        manager.collector = FailingCollector()

        manager.refresh_snapshot()

        self.assertEqual(manager.cpu_label.text(), "CPU: unavailable")
        self.assertEqual(manager.cpu_bar.value(), 0)
        self.assertEqual(manager.memory_label.text(), "Memory: unavailable")
        self.assertEqual(manager.memory_bar.value(), 0)
        self.assertEqual(manager.swap_label.text(), "Swap: unavailable")
        self.assertEqual(manager.swap_bar.value(), 0)
        self.assertEqual(manager.system_label.text(), "System information unavailable")
        self.assertIn("synthetic procfs failure", manager.status.text())
        manager.close()


class ProcfsCollectorTests(unittest.TestCase):
    def test_process_stat_handles_spaces_and_parentheses(self):
        parsed = parse_process_stat(make_stat())
        self.assertEqual(parsed.name, "worker (render)")
        self.assertEqual(parsed.user_ticks, 5)
        self.assertEqual(parsed.system_ticks, 2)
        self.assertEqual(parsed.start_ticks, 77)
        self.assertEqual(parsed.resident_pages, 10)

    def test_rejects_truncated_and_negative_process_counters(self):
        with self.assertRaises(ValueError):
            parse_process_stat("123 (broken) S 1 2")
        fields = ["0"] * 22
        fields[0] = "R"
        fields[11] = "-1"
        with self.assertRaises(ValueError):
            parse_process_stat("123 (bad) " + " ".join(fields))

    def test_cpu_parser_excludes_guest_counters(self):
        total, idle = parse_cpu_line("cpu 10 2 3 40 5 6 7 8 90 10")
        self.assertEqual(total, 81)
        self.assertEqual(idle, 45)

    def test_memory_values_are_bounded_and_converted(self):
        parsed = parse_memory_info(
            "MemTotal: 1000 kB\nMemAvailable: 1200 kB\n"
            "SwapTotal: 500 kB\nSwapFree: 600 kB\n"
        )
        self.assertEqual(parsed["MemTotal"], 1024000)
        self.assertEqual(parsed["MemAvailable"], parsed["MemTotal"])
        self.assertEqual(parsed["SwapFree"], parsed["SwapTotal"])

    def test_system_stats_read_large_cpu_files_with_bounded_access(self):
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary)
            (root / "stat").write_text(
                "cpu 100 0 50 800 0 0 0 0 0 0\n"
                + ("cpu0 1 0 1 8 0 0 0 0 0 0\n" * 2000),
                encoding="ascii",
            )
            (root / "meminfo").write_text("MemTotal: 1000 kB\n", encoding="ascii")
            (root / "cpuinfo").write_text(
                "model name : Test CPU\n" + ("large cpu record\n" * 5000),
                encoding="ascii",
            )
            collector = ProcfsCollector(root, page_size=4096, cpu_count=2)
            system, total, idle = collector._read_system()
            self.assertEqual((total, idle), (950, 800))
            self.assertEqual(system.cpu_model, "Test CPU")

    def test_process_enumeration_respects_configured_limit(self):
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary)
            (root / "stat").write_text(
                "cpu 100 0 50 800 0 0 0 0 0 0" + chr(10), encoding="ascii"
            )
            (root / "meminfo").write_text(
                chr(10).join(["MemTotal: 1000 kB", "MemAvailable: 500 kB"]) + chr(10),
                encoding="ascii",
            )
            (root / "uptime").write_text("10.0 0.0" + chr(10), encoding="ascii")
            (root / "cpuinfo").write_text("model name : Test CPU" + chr(10), encoding="ascii")
            for pid in range(100, 110):
                process = root / str(pid)
                process.mkdir()
                (process / "stat").write_text(
                    make_stat(pid=pid, name=f"worker-{pid}", start=pid),
                    encoding="ascii",
                )

            class FakeScandir:
                def __enter__(self):
                    return self

                def __exit__(self, *_args):
                    return False

                def __iter__(self):
                    return iter(
                        [type("Entry", (), {"name": str(pid)})() for pid in range(100, 110)]
                    )

            collector = ProcfsCollector(
                root, page_size=4096, cpu_count=2, max_processes=2
            )
            with patch("nexaos_task_manager.os.scandir", return_value=FakeScandir()):
                _, processes = collector.snapshot()
            self.assertEqual(len(processes), 2)

    def test_collector_first_sample_has_no_fabricated_cpu(self):
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary)
            process = root / "321"
            process.mkdir()
            (process / "stat").write_text(make_stat(), encoding="ascii")
            (root / "stat").write_text("cpu 100 0 50 800 0 0 0 0 0 0\n", encoding="ascii")
            (root / "meminfo").write_text(
                "MemTotal: 1000 kB\nMemAvailable: 400 kB\n"
                "SwapTotal: 200 kB\nSwapFree: 150 kB\n", encoding="ascii"
            )
            (root / "uptime").write_text("123.5 20.0\n", encoding="ascii")
            (root / "cpuinfo").write_text("model name : Test CPU\n", encoding="ascii")
            clock = FakeClock()
            collector = ProcfsCollector(root, page_size=4096, cpu_count=2, clock=clock)
            first_system, first_processes = collector.snapshot()
            self.assertIsNone(first_system.cpu_percent)
            self.assertEqual(len(first_processes), 1)
            self.assertIsNone(first_processes[0].cpu_percent)
            self.assertEqual(first_processes[0].memory_bytes, 40960)

            clock.advance()
            (root / "stat").write_text("cpu 110 0 60 880 0 0 0 0 0 0\n", encoding="ascii")
            (process / "stat").write_text(
                make_stat(user=15, system=5), encoding="ascii"
            )
            second_system, second_processes = collector.snapshot()
            self.assertAlmostEqual(second_system.cpu_percent, 20.0)
            self.assertAlmostEqual(second_processes[0].cpu_percent, 26.0)

    def test_pid_reuse_does_not_reuse_old_cpu_baseline(self):
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary)
            process = root / "321"
            process.mkdir()
            (process / "stat").write_text(make_stat(start=77), encoding="ascii")
            (root / "stat").write_text("cpu 100 0 50 800 0 0 0 0 0 0\n", encoding="ascii")
            (root / "meminfo").write_text("MemTotal: 1000 kB\n", encoding="ascii")
            (root / "uptime").write_text("10.0 0.0\n", encoding="ascii")
            clock = FakeClock()
            collector = ProcfsCollector(root, page_size=4096, cpu_count=1, clock=clock)
            collector.snapshot()
            clock.advance()
            (root / "stat").write_text("cpu 110 0 60 880 0 0 0 0 0 0\n", encoding="ascii")
            (process / "stat").write_text(make_stat(user=100, start=88), encoding="ascii")
            _, samples = collector.snapshot()
            self.assertIsNone(samples[0].cpu_percent)


if __name__ == "__main__":
    unittest.main()
