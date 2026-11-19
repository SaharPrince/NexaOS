"""Read-only, home-confined file browser for the NexaOS preview.

This module deliberately does not open arbitrary files, follow symbolic links,
invoke desktop handlers, or execute Windows applications.
"""
from __future__ import annotations

import os
import stat
from dataclasses import dataclass
from pathlib import Path

from PyQt6.QtCore import Qt
from PyQt6.QtGui import QColor
from PyQt6.QtWidgets import (
    QCheckBox,
    QDialog,
    QHeaderView,
    QHBoxLayout,
    QLabel,
    QMessageBox,
    QPushButton,
    QTableWidget,
    QTableWidgetItem,
    QVBoxLayout,
)


MAX_VISIBLE_ENTRIES = 1000


def safe_display_name(value: str) -> str:
    """Replace control characters before untrusted names reach the UI."""
    return "".join(char if char.isprintable() else "\uFFFD" for char in value)[:180]


@dataclass(frozen=True)
class BrowserEntry:
    name: str
    path: Path
    kind: str
    size: int | None
    is_symlink: bool = False


class SafeDirectoryReader:
    """Lists one directory at a time, confined to a canonical user-home root."""

    def __init__(self, root: Path, limit: int = MAX_VISIBLE_ENTRIES) -> None:
        self.root = root.resolve(strict=True)
        if not self.root.is_dir():
            raise ValueError("The browser root must be a directory.")
        if limit < 1:
            raise ValueError("The entry limit must be positive.")
        if (
            not hasattr(os, "O_DIRECTORY")
            or not hasattr(os, "O_NOFOLLOW")
            or os.open not in os.supports_dir_fd
            or os.scandir not in os.supports_fd
        ):
            raise RuntimeError("This browser requires safe descriptor-relative directory access.")
        self.limit = limit

    def _open_directory(self, candidate: Path) -> tuple[Path, int]:
        # Walk from the trusted root with O_NOFOLLOW so a symlink cannot redirect
        # navigation outside the home folder, including during path resolution.
        lexical = Path(os.path.abspath(candidate))
        try:
            relative = lexical.relative_to(self.root)
        except ValueError as error:
            raise PermissionError("This location is outside the allowed home folder.") from error

        flags = os.O_RDONLY | os.O_DIRECTORY | os.O_NOFOLLOW | getattr(os, "O_CLOEXEC", 0)
        descriptor = os.open(self.root, flags)
        try:
            for component in relative.parts:
                next_descriptor = os.open(component, flags, dir_fd=descriptor)
                os.close(descriptor)
                descriptor = next_descriptor
            if not stat.S_ISDIR(os.fstat(descriptor).st_mode):
                raise NotADirectoryError(str(candidate))
            return lexical, descriptor
        except Exception:
            os.close(descriptor)
            raise

    def validate_directory(self, candidate: Path) -> Path:
        lexical, descriptor = self._open_directory(candidate)
        os.close(descriptor)
        return lexical

    def list_entries(
        self, directory: Path, show_hidden: bool = False
    ) -> tuple[list[BrowserEntry], bool]:
        current, descriptor = self._open_directory(directory)
        entries: list[BrowserEntry] = []
        truncated = False
        try:
            with os.scandir(descriptor) as iterator:
                for item in iterator:
                    if not show_hidden and item.name.startswith("."):
                        continue
                    try:
                        metadata = item.stat(follow_symlinks=False)
                    except OSError:
                        continue

                    is_link = stat.S_ISLNK(metadata.st_mode)
                    if is_link:
                        kind, size = "Link (not opened)", None
                    elif stat.S_ISDIR(metadata.st_mode):
                        kind, size = "Folder", None
                    elif stat.S_ISREG(metadata.st_mode):
                        kind, size = "File", metadata.st_size
                    else:
                        kind, size = "Special item (not opened)", None

                    if len(entries) >= self.limit:
                        truncated = True
                        break
                    entries.append(
                        BrowserEntry(
                            name=safe_display_name(item.name),
                            path=current / item.name,
                            kind=kind,
                            size=size,
                            is_symlink=is_link,
                        )
                    )
        except OSError as error:
            raise OSError(f"Could not read this folder: {error}") from error
        finally:
            os.close(descriptor)

        entries.sort(
            key=lambda entry: (
                0 if entry.kind == "Folder" else 1,
                entry.name.casefold(),
            )
        )
        return entries, truncated


def format_size(size: int | None) -> str:
    if size is None:
        return "—"
    units = ("B", "KB", "MB", "GB", "TB")
    value = float(size)
    for unit in units:
        if value < 1024 or unit == units[-1]:
            return f"{value:.0f} {unit}" if unit == "B" else f"{value:.1f} {unit}"
        value /= 1024
    return f"{value:.1f} TB"


class FileBrowserDialog(QDialog):
    """Small read-only browser. Directories navigate; files are never opened."""

    def __init__(self, parent=None, root: Path | None = None) -> None:
        super().__init__(parent)
        self.reader = SafeDirectoryReader(root or Path.home())
        self.current = self.reader.root
        self.history: list[Path] = []
        self.entries: list[BrowserEntry] = []

        self.setWindowTitle("NexaOS Files")
        self.setMinimumSize(720, 480)
        layout = QVBoxLayout(self)

        heading = QLabel("Your files")
        heading.setStyleSheet("font-size: 22px; font-weight: 650;")
        layout.addWidget(heading)

        navigation = QHBoxLayout()
        self.back_button = QPushButton("Back")
        self.up_button = QPushButton("Up")
        self.home_button = QPushButton("Home")
        self.refresh_button = QPushButton("Refresh")
        for button in (self.back_button, self.up_button, self.home_button, self.refresh_button):
            navigation.addWidget(button)
        layout.addLayout(navigation)

        self.location_label = QLabel()
        self.location_label.setTextInteractionFlags(
            Qt.TextInteractionFlag.TextSelectableByMouse
        )
        layout.addWidget(self.location_label)

        self.hidden_toggle = QCheckBox("Show hidden files")
        layout.addWidget(self.hidden_toggle)

        self.table = QTableWidget(0, 3)
        self.table.setHorizontalHeaderLabels(["Name", "Type", "Size"])
        self.table.setSelectionBehavior(
            QTableWidget.SelectionBehavior.SelectRows
        )
        self.table.setEditTriggers(QTableWidget.EditTrigger.NoEditTriggers)
        self.table.setAlternatingRowColors(True)
        self.table.horizontalHeader().setStretchLastSection(False)
        self.table.horizontalHeader().setSectionResizeMode(0, QHeaderView.ResizeMode.Stretch)
        self.table.setColumnWidth(1, 150)
        self.table.setColumnWidth(2, 90)
        layout.addWidget(self.table)

        self.status_label = QLabel(
            "Read-only preview. Files are not launched or opened."
        )
        self.status_label.setWordWrap(True)
        layout.addWidget(self.status_label)

        self.back_button.clicked.connect(self.go_back)
        self.up_button.clicked.connect(self.go_up)
        self.home_button.clicked.connect(self.go_home)
        self.refresh_button.clicked.connect(self.refresh)
        self.hidden_toggle.toggled.connect(self.refresh)
        self.table.cellDoubleClicked.connect(self.activate_row)
        self.refresh()

    def navigate(self, destination: Path) -> None:
        try:
            target = self.reader.validate_directory(destination)
        except (OSError, ValueError) as error:
            self.status_label.setText(f"Folder not opened: {error}")
            return
        if target == self.current:
            self.refresh()
            return
        self.history.append(self.current)
        self.current = target
        self.refresh()

    def go_back(self) -> None:
        while self.history:
            previous = self.history.pop()
            try:
                self.current = self.reader.validate_directory(previous)
                break
            except (OSError, ValueError):
                continue
        self.refresh()

    def go_up(self) -> None:
        if self.current != self.reader.root:
            self.navigate(self.current.parent)

    def go_home(self) -> None:
        self.navigate(self.reader.root)

    def refresh(self) -> None:
        try:
            self.current = self.reader.validate_directory(self.current)
            self.entries, truncated = self.reader.list_entries(
                self.current, self.hidden_toggle.isChecked()
            )
        except (OSError, ValueError) as error:
            self.entries = []
            self.status_label.setText(f"Folder unavailable: {error}")
            self.table.setRowCount(0)
            return

        self.location_label.setText(str(self.current))
        self.table.setRowCount(len(self.entries))
        for row, entry in enumerate(self.entries):
            name_item = QTableWidgetItem(entry.name)
            name_item.setData(Qt.ItemDataRole.UserRole, row)
            if entry.is_symlink or entry.kind != "Folder":
                name_item.setForeground(QColor(Qt.GlobalColor.darkGray))
            self.table.setItem(row, 0, name_item)
            self.table.setItem(row, 1, QTableWidgetItem(entry.kind))
            self.table.setItem(row, 2, QTableWidgetItem(format_size(entry.size)))

        self.back_button.setEnabled(bool(self.history))
        self.up_button.setEnabled(self.current != self.reader.root)
        suffix = f" Showing the first {self.reader.limit} items." if truncated else ""
        self.status_label.setText(
            f"{len(self.entries)} item(s). Read-only: files are not opened or executed.{suffix}"
        )

    def activate_row(self, row: int, _column: int) -> None:
        if row < 0 or row >= len(self.entries):
            return
        entry = self.entries[row]
        if entry.is_symlink:
            self.status_label.setText("Symbolic links are not opened in this preview.")
        elif entry.kind == "Folder":
            self.navigate(entry.path)
        elif entry.kind == "File" and entry.path.suffix.casefold() == ".exe":
            QMessageBox.information(
                self,
                "Windows application preview",
                f"Selected: {entry.name}\n\n"
                "Windows application execution is not enabled. "
                "No process was started and the file was not executed.",
            )
        else:
            self.status_label.setText(
                f"Selected {entry.name}. File opening is not enabled in this read-only preview."
            )
