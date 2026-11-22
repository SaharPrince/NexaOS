"""Read-only, bounded Linux process and system monitor for NexaOS."""
from __future__ import annotations

import os
import platform
import pwd
import sys
import time
from dataclasses import dataclass
from pathlib import Path
from typing import Callable

from PyQt6.QtCore import QEvent, Qt, QTimer
from PyQt6.QtWidgets import (
    QApplication, QDialog, QLabel, QLineEdit, QProgressBar, QTabWidget,
    QTableWidget, QTableWidgetItem, QVBoxLayout, QWidget,
)

MAX_PROCESSES = 4096
MAX_STAT_BYTES = 4096
MAX_SYSTEM_FILE_BYTES = 65536
MAX_NAME_CHARS = 160


@dataclass(frozen=True)
class ProcessStat:
    name: str
    state: str
    user_ticks: int
    system_ticks: int
    start_ticks: int
    resident_pages: int


@dataclass(frozen=True)
class ProcessSample:
    pid: int
    name: str
    owner: str
    state: str
    cpu_percent: float | None
    memory_bytes: int
    start_ticks: int


@dataclass(frozen=True)
class SystemSample:
    cpu_percent: float | None
    memory_total: int
    memory_available: int
    swap_total: int
    swap_free: int
    uptime_seconds: float | None
    cpu_model: str
    logical_cpus: int
    kernel: str


def parse_process_stat(text: str) -> ProcessStat:
    """Parse /proc/PID/stat while preserving spaces and ')' inside comm."""
    left, right = text.find("("), text.rfind(")")
    if left < 0 or right <= left or right + 2 > len(text):
        raise ValueError("malformed process stat")
    name = text[left + 1:right]
    fields = text[right + 1:].strip().split()
    # fields[0] is kernel field 3 (state); field 24 (rss) is fields[21].
    if len(fields) < 22 or len(name) > MAX_NAME_CHARS:
        raise ValueError("truncated or oversized process stat")
    if len(fields[0]) != 1:
        raise ValueError("invalid process state")
    try:
        user_ticks = int(fields[11], 10)
        system_ticks = int(fields[12], 10)
        start_ticks = int(fields[19], 10)
        resident_pages = int(fields[21], 10)
    except (ValueError, IndexError) as error:
        raise ValueError("invalid numeric process stat field") from error
    if min(user_ticks, system_ticks, start_ticks) < 0:
        raise ValueError("negative process counter")
    return ProcessStat(name, fields[0], user_ticks, system_ticks,
                       start_ticks, max(0, resident_pages))


def parse_cpu_line(text: str) -> tuple[int, int]:
    fields = text.split()
    if not fields or fields[0] != "cpu" or len(fields) < 5:
        raise ValueError("invalid aggregate CPU line")
    try:
        values = [int(value, 10) for value in fields[1:]]
    except ValueError as error:
        raise ValueError("invalid CPU counter") from error
    if any(value < 0 for value in values):
        raise ValueError("negative CPU counter")
    # guest and guest_nice are already included in user and nice.
    total = sum(values[:8])
    idle = values[3] + (values[4] if len(values) > 4 else 0)
    return total, idle


def parse_memory_info(text: str) -> dict[str, int]:
    result: dict[str, int] = {}
    wanted = {"MemTotal", "MemAvailable", "SwapTotal", "SwapFree"}
    for line in text.splitlines():
        key, separator, value = line.partition(":")
        if not separator or key not in wanted:
            continue
        fields = value.split()
        if not fields:
            continue
        try:
            amount = int(fields[0], 10)
        except ValueError:
            continue
        if amount >= 0:
            result[key] = amount * 1024
    if "MemTotal" not in result:
        raise ValueError("MemTotal is unavailable")
    result.setdefault("MemAvailable", 0)
    result.setdefault("SwapTotal", 0)
    result.setdefault("SwapFree", 0)
    result["MemAvailable"] = min(result["MemAvailable"], result["MemTotal"])
    result["SwapFree"] = min(result["SwapFree"], result["SwapTotal"])
    return result


class ProcfsCollector:
    """Read bounded procfs summaries; never reads cmdline, environ, or process memory."""

    def __init__(
        self, proc_root: Path = Path("/proc"), *, page_size: int | None = None,
        cpu_count: int | None = None, clock: Callable[[], float] = time.monotonic,
        max_processes: int = MAX_PROCESSES,
    ) -> None:
        if not 1 <= max_processes <= MAX_PROCESSES:
            raise ValueError("invalid process limit")
        self.proc_root = Path(proc_root)
        self.page_size = page_size or int(os.sysconf("SC_PAGE_SIZE"))
        self.cpu_count = max(1, cpu_count or os.cpu_count() or 1)
        self.clock = clock
        self.max_processes = max_processes
        self._previous_cpu: tuple[int, int] | None = None
        self._previous_processes: dict[int, tuple[int, int]] = {}
        self._previous_time: float | None = None

    @staticmethod
    def _read_bounded(path: Path, limit: int) -> str:
        with path.open("rb") as stream:
            data = stream.read(limit + 1)
        if len(data) > limit:
            raise ValueError("procfs record exceeds size limit")
        return data.decode("ascii", errors="replace")

    def _read_system(self) -> tuple[SystemSample, int, int]:
        lines = self._read_bounded(self.proc_root / "stat", 8192).splitlines()
        if not lines:
            raise ValueError("CPU statistics unavailable")
        total, idle = parse_cpu_line(lines[0])
        memory = parse_memory_info(
            self._read_bounded(self.proc_root / "meminfo", MAX_SYSTEM_FILE_BYTES)
        )
        try:
            uptime = float(self._read_bounded(self.proc_root / "uptime", 128).split()[0])
            if uptime < 0:
                uptime = None
        except (OSError, ValueError, IndexError):
            uptime = None
        model = "Unavailable"
        try:
            for line in self._read_bounded(
                self.proc_root / "cpuinfo", MAX_SYSTEM_FILE_BYTES
            ).splitlines():
                key, separator, value = line.partition(":")
                if separator and key.strip() in ("model name", "Hardware"):
                    model = value.strip()[:MAX_NAME_CHARS] or model
                    break
        except OSError:
            pass
        sample = SystemSample(
            None, memory["MemTotal"], memory["MemAvailable"],
            memory["SwapTotal"], memory["SwapFree"], uptime,
            model, self.cpu_count, platform.release(),
        )
        return sample, total, idle

    def _read_processes(self, total_delta: int, elapsed: float) -> list[ProcessSample]:
        samples: list[ProcessSample] = []
        current: dict[int, tuple[int, int]] = {}
        try:
            directory = os.scandir(self.proc_root)
        except OSError:
            self._previous_processes = {}
            return samples
        with directory:
            for entry in directory:
                if len(samples) >= self.max_processes:
                    break
                if not entry.name.isdecimal():
                    continue
                try:
                    pid = int(entry.name, 10)
                    if pid <= 0:
                        continue
                    process_dir = self.proc_root / entry.name
                    parsed = parse_process_stat(
                        self._read_bounded(process_dir / "stat", MAX_STAT_BYTES)
                    )
                    uid = process_dir.stat().st_uid
                    try:
                        owner = pwd.getpwuid(uid).pw_name
                    except (KeyError, OSError):
                        owner = str(uid)
                except (OSError, ValueError, OverflowError):
                    # A process may exit or become inaccessible during enumeration.
                    continue
                ticks = parsed.user_ticks + parsed.system_ticks
                current[pid] = (parsed.start_ticks, ticks)
                cpu = None
                previous = self._previous_processes.get(pid)
                if previous and previous[0] == parsed.start_ticks and total_delta > 0 and elapsed > 0:
                    delta = ticks - previous[1]
                    if delta >= 0:
                        cpu = min(100.0 * self.cpu_count,
                                  100.0 * delta * self.cpu_count / total_delta)
                samples.append(ProcessSample(
                    pid, parsed.name[:MAX_NAME_CHARS], owner[:MAX_NAME_CHARS],
                    parsed.state, cpu, parsed.resident_pages * self.page_size,
                    parsed.start_ticks,
                ))
        self._previous_processes = current
        samples.sort(key=lambda item: (item.name.casefold(), item.pid))
        return samples

    def snapshot(self) -> tuple[SystemSample, list[ProcessSample]]:
        system, total, idle = self._read_system()
        now = self.clock()
        total_delta = idle_delta = 0
        cpu = None
        if self._previous_cpu is not None:
            total_delta = total - self._previous_cpu[0]
            idle_delta = idle - self._previous_cpu[1]
            if total_delta > 0 and 0 <= idle_delta <= total_delta:
                cpu = max(0.0, min(100.0, 100.0 * (total_delta - idle_delta) / total_delta))
        elapsed = now - self._previous_time if self._previous_time is not None else 0.0
        if elapsed <= 0:
            total_delta = 0
        self._previous_cpu = (total, idle)
        self._previous_time = now
        processes = self._read_processes(total_delta, elapsed)
        system = SystemSample(
            cpu, system.memory_total, system.memory_available,
            system.swap_total, system.swap_free, system.uptime_seconds,
            system.cpu_model, system.logical_cpus, system.kernel,
        )
        return system, processes


def format_bytes(value: int) -> str:
    amount = float(max(0, value))
    for unit in ("B", "KiB", "MiB", "GiB", "TiB"):
        if amount < 1024.0 or unit == "TiB":
            return f"{amount:.1f} {unit}"
        amount /= 1024.0
    return f"{amount:.1f} TiB"


class NexaTaskManager(QDialog):
    """On-demand, read-only monitor. Sampling stops whenever the window hides."""

    def __init__(self, parent: QWidget | None = None) -> None:
        super().__init__(parent)
        self.setWindowTitle("NexaOS Task Manager")
        self.resize(920, 620)
        self.collector = ProcfsCollector()
        self._snapshot: list[ProcessSample] = []
        self.timer = QTimer(self)
        self.timer.setInterval(1200)
        self.timer.timeout.connect(self.refresh_snapshot)

        layout = QVBoxLayout(self)
        self.status = QLabel("Monitoring starts while this window is open.")
        self.status.setWordWrap(True)
        layout.addWidget(self.status)
        self.tabs = QTabWidget(self)
        layout.addWidget(self.tabs)

        process_page = QWidget()
        process_layout = QVBoxLayout(process_page)
        self.search = QLineEdit()
        self.search.setPlaceholderText("Filter the current process snapshot…")
        self.search.textChanged.connect(self._render_processes)
        process_layout.addWidget(self.search)
        self.process_table = QTableWidget(0, 6)
        self.process_table.setHorizontalHeaderLabels(
            ("Process", "PID", "User", "CPU", "Memory", "State")
        )
        self.process_table.setEditTriggers(QTableWidget.EditTrigger.NoEditTriggers)
        self.process_table.setSelectionBehavior(QTableWidget.SelectionBehavior.SelectRows)
        self.process_table.setAlternatingRowColors(True)
        self.process_table.setSortingEnabled(True)
        process_layout.addWidget(self.process_table)
        self.tabs.addTab(process_page, "Processes")

        performance_page = QWidget()
        performance_layout = QVBoxLayout(performance_page)
        self.cpu_label = QLabel("CPU: collecting first sample…")
        self.cpu_bar = QProgressBar()
        self.cpu_bar.setRange(0, 100)
        self.memory_label = QLabel("Memory: unavailable")
        self.memory_bar = QProgressBar()
        self.memory_bar.setRange(0, 100)
        self.swap_label = QLabel("Swap: unavailable")
        self.swap_bar = QProgressBar()
        self.swap_bar.setRange(0, 100)
        self.system_label = QLabel("System information unavailable")
        for widget in (
            self.cpu_label, self.cpu_bar, self.memory_label, self.memory_bar,
            self.swap_label, self.swap_bar, self.system_label,
        ):
            performance_layout.addWidget(widget)
        performance_layout.addStretch(1)
        self.tabs.addTab(performance_page, "Performance")

    def showEvent(self, event) -> None:
        super().showEvent(event)
        self.refresh_snapshot()
        self.timer.start()

    def changeEvent(self, event) -> None:
        super().changeEvent(event)
        if event.type() == QEvent.Type.WindowStateChange:
            if self.isMinimized():
                self.timer.stop()
            elif self.isVisible():
                self.refresh_snapshot()
                self.timer.start()

    def hideEvent(self, event) -> None:
        self.timer.stop()
        super().hideEvent(event)

    def closeEvent(self, event) -> None:
        self.timer.stop()
        super().closeEvent(event)

    def _render_processes(self) -> None:
        query = self.search.text().casefold().strip()
        visible = [
            item for item in self._snapshot
            if not query or query in item.name.casefold()
            or query in str(item.pid) or query in item.owner.casefold()
        ]
        self.process_table.setSortingEnabled(False)
        self.process_table.setRowCount(len(visible))
        for row, item in enumerate(visible):
            values = (
                item.name, str(item.pid), item.owner,
                "—" if item.cpu_percent is None else f"{item.cpu_percent:.1f}%",
                format_bytes(item.memory_bytes), item.state,
            )
            for column, value in enumerate(values):
                cell = QTableWidgetItem(value)
                if column == 1:
                    cell.setData(Qt.ItemDataRole.DisplayRole, item.pid)
                self.process_table.setItem(row, column, cell)
        self.process_table.setSortingEnabled(True)

    def refresh_snapshot(self) -> None:
        try:
            system, processes = self.collector.snapshot()
        except (OSError, ValueError, RuntimeError) as error:
            self._snapshot = []
            self._render_processes()
            self.status.setText(f"System information unavailable: {str(error)[:180]}")
            return
        self._snapshot = processes
        self._render_processes()
        self.status.setText(
            f"{len(processes)} processes shown • Read-only; process controls are disabled."
        )
        if system.cpu_percent is None:
            self.cpu_label.setText("CPU: collecting first sample…")
            self.cpu_bar.setValue(0)
        else:
            self.cpu_label.setText(f"CPU usage: {system.cpu_percent:.1f}%")
            self.cpu_bar.setValue(round(system.cpu_percent))
        used = max(0, system.memory_total - system.memory_available)
        memory_pct = 100.0 * used / system.memory_total if system.memory_total else 0.0
        self.memory_label.setText(
            f"Memory: {format_bytes(used)} used / {format_bytes(system.memory_total)} total"
        )
        self.memory_bar.setValue(round(max(0.0, min(100.0, memory_pct))))
        swap_used = max(0, system.swap_total - system.swap_free)
        swap_pct = 100.0 * swap_used / system.swap_total if system.swap_total else 0.0
        self.swap_label.setText(
            f"Swap: {format_bytes(swap_used)} used / {format_bytes(system.swap_total)} total"
        )
        self.swap_bar.setValue(round(max(0.0, min(100.0, swap_pct))))
        uptime = "Unavailable" if system.uptime_seconds is None else f"{system.uptime_seconds / 3600:.1f} hours"
        self.system_label.setText(
            f"{system.cpu_model} • {system.logical_cpus} logical CPUs • "
            f"Kernel {system.kernel} • Uptime {uptime}"
        )


def main() -> int:
    app = QApplication(sys.argv[:1])
    window = NexaTaskManager()
    window.show()
    return app.exec()


if __name__ == "__main__":
    raise SystemExit(main())
