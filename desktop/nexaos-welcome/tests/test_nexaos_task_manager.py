import tempfile
import unittest
from pathlib import Path

from nexaos_task_manager import (
    ProcfsCollector,
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
        fields = [0] * 22
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
