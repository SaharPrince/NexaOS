"""Read-only, home-confined file browser for the NexaOS preview.

This module deliberately does not open arbitrary files, follow symbolic links,
invoke desktop handlers, or execute Windows applications.
"""
from __future__ import annotations

import json
import os
import stat
import subprocess
from dataclasses import dataclass
from pathlib import Path

from PyQt6.QtCore import Qt
from PyQt6.QtGui import QColor, QKeySequence, QShortcut
from PyQt6.QtWidgets import (
    QCheckBox,
    QDialog,
    QHeaderView,
    QHBoxLayout,
    QLabel,
    QLineEdit,
    QMessageBox,
    QPushButton,
    QTableWidget,
    QTableWidgetItem,
    QVBoxLayout,
)


MAX_VISIBLE_ENTRIES = 1000

CONTROLLER_EXECUTABLE = Path("/usr/lib/nexaos/nexa-desktop-controller")
MAX_CONTROLLER_REQUEST_BYTES = 1024 * 1024


def build_controller_preview_request(
    reader: "SafeDirectoryReader",
    directory: Path,
    entries: list["BrowserEntry"],
    selected: "BrowserEntry",
) -> dict:
    """Build a version-1, home-relative request for the preview-only Rust controller."""
    current = reader.validate_directory(directory)
    if selected.is_symlink or selected.kind != "File" or selected.path.suffix.casefold() != ".exe":
        raise ValueError("Only a regular .exe file can be sent for preview.")
    try:
        current_relative = current.relative_to(reader.root).as_posix()
    except ValueError as error:
        raise ValueError("The current folder is outside the browser root.") from error
    if current_relative == ".":
        current_relative = ""

    serialized_entries = []
    selected_seen = False
    for entry in entries:
        if entry.is_symlink or entry.kind not in ("Folder", "File"):
            continue
        if entry.path.parent != current:
            continue
        name = entry.path.name
        serialized_entries.append({
            "name": name,
            "relative_path": name,
            "kind": "directory" if entry.kind == "Folder" else "file",
            "size": entry.size,
        })
        if entry.path == selected.path:
            selected_seen = True
    if not selected_seen:
        raise ValueError("The selected file is not part of the current safe directory listing.")
    if len(serialized_entries) > MAX_VISIBLE_ENTRIES:
        raise ValueError("The directory listing exceeds the controller limit.")

    return {
        "protocol_version": 1,
        "action": "preview_selected",
        "snapshot": {
            "protocol_version": 1,
            "root": str(reader.root),
            "current_directory": current_relative,
            "entries": serialized_entries,
        },
        "selected_relative_path": selected.path.name,
    }


def request_controller_preview(
    command: dict,
    executable: Path = CONTROLLER_EXECUTABLE,
) -> dict:
    """Call only the fixed installed Rust adapter; never invoke a shell or file handler."""
    if not executable.is_absolute() or not executable.is_file() or not os.access(executable, os.X_OK):
        raise FileNotFoundError("The NexaOS desktop controller is not installed.")
    payload = json.dumps(command, ensure_ascii=True, separators=(",", ":"))
    if len(payload.encode("utf-8")) > MAX_CONTROLLER_REQUEST_BYTES:
        raise ValueError("The controller request is too large.")
    completed = subprocess.run(
        [str(executable)],
        input=payload,
        text=True,
        capture_output=True,
        timeout=3,
        check=False,
        close_fds=True,
    )
    try:
        response = json.loads(completed.stdout)
    except json.JSONDecodeError as error:
        raise RuntimeError("The desktop controller returned an invalid response.") from error
    if completed.returncode != 0 or response.get("status") != "preview_only":
        code = response.get("error", "controller_request_failed")
        raise RuntimeError(f"The desktop controller rejected the preview: {code}")
    if response.get("protocol_version") != 1:
        raise RuntimeError("The desktop controller returned an unsupported protocol version.")
    return response



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


def filter_entries(entries: list[BrowserEntry], query: str) -> list[BrowserEntry]:
    """Filter the current bounded listing only; never scan child directories."""
    needle = query.strip().casefold()
    if not needle:
        return list(entries)
    return [
        entry for entry in entries
        if needle in entry.name.casefold() or needle in entry.kind.casefold()
    ]


def sort_entries(
    entries: list[BrowserEntry], column: int, descending: bool = False
) -> list[BrowserEntry]:
    """Sort entries while keeping folders first, independent of sort direction."""
    if column == 0:
        key = lambda entry: entry.name.casefold()
    elif column == 1:
        key = lambda entry: entry.kind.casefold()
    elif column == 2:
        key = lambda entry: -1 if entry.size is None else entry.size
    else:
        raise ValueError("Unsupported file-browser sort column.")

    folders = [entry for entry in entries if entry.kind == "Folder"]
    others = [entry for entry in entries if entry.kind != "Folder"]
    return sorted(folders, key=key, reverse=descending) + sorted(
        others, key=key, reverse=descending
    )


class FileBrowserDialog(QDialog):
    """Small read-only browser. Directories navigate; files are never opened."""

    def __init__(self, parent=None, root: Path | None = None) -> None:
        super().__init__(parent)
        self.reader = SafeDirectoryReader(root or Path.home())
        self.current = self.reader.root
        self.history: list[Path] = []
        self.entries: list[BrowserEntry] = []
        self.truncated = False
        self.sort_column = 0
        self.sort_descending = False

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

        self.search_input = QLineEdit()
        self.search_input.setPlaceholderText("Search this folder…")
        self.search_input.setClearButtonEnabled(True)
        self.search_input.setAccessibleName("Search current folder")
        layout.addWidget(self.search_input)

        self.location_label = QLabel()
        self.location_label.setTextInteractionFlags(
            Qt.TextInteractionFlag.TextSelectableByMouse
        )
        layout.addWidget(self.location_label)

        self.hidden_toggle = QCheckBox("Show hidden files")
        layout.addWidget(self.hidden_toggle)

        self.table = QTableWidget(0, 3)
        self.table.setHorizontalHeaderLabels(["Name", "Type", "Size"])
        self.table.setSortingEnabled(False)
        self.table.setSelectionBehavior(
            QTableWidget.SelectionBehavior.SelectRows
        )
        self.table.setEditTriggers(QTableWidget.EditTrigger.NoEditTriggers)
        self.table.setAlternatingRowColors(True)
        self.table.horizontalHeader().setStretchLastSection(False)
        self.table.horizontalHeader().setSectionResizeMode(0, QHeaderView.ResizeMode.Stretch)
        self.table.setColumnWidth(1, 150)
        self.table.setColumnWidth(2, 90)
        self.table.horizontalHeader().sectionClicked.connect(self.sort_by_column)
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
        self.search_input.textChanged.connect(self.apply_filter)
        self.table.cellDoubleClicked.connect(self.activate_row)
        self._shortcuts = []
        for sequence, callback in (
            ("Ctrl+F", self.search_input.setFocus),
            ("Alt+Left", self.go_back),
            ("Alt+Up", self.go_up),
            ("Alt+Home", self.go_home),
            ("F5", self.refresh),
            ("Escape", self.search_input.clear),
        ):
            shortcut = QShortcut(QKeySequence(sequence), self)
            shortcut.activated.connect(callback)
            self._shortcuts.append(shortcut)
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
        self.truncated = truncated
        self.back_button.setEnabled(bool(self.history))
        self.up_button.setEnabled(self.current != self.reader.root)
        self.apply_filter()

    def apply_filter(self) -> None:
        """Render a filtered/sorted view without changing the trusted source listing."""
        visible = sort_entries(
            filter_entries(self.entries, self.search_input.text()),
            self.sort_column,
            self.sort_descending,
        )
        source_indices = {entry.path: index for index, entry in enumerate(self.entries)}
        self.table.setRowCount(len(visible))
        for row, entry in enumerate(visible):
            name_item = QTableWidgetItem(entry.name)
            name_item.setData(Qt.ItemDataRole.UserRole, source_indices[entry.path])
            if entry.is_symlink or entry.kind != "Folder":
                name_item.setForeground(QColor(Qt.GlobalColor.darkGray))
            self.table.setItem(row, 0, name_item)
            self.table.setItem(row, 1, QTableWidgetItem(entry.kind))
            self.table.setItem(row, 2, QTableWidgetItem(format_size(entry.size)))

        query = self.search_input.text().strip()
        if query:
            status = f"Showing {len(visible)} of {len(self.entries)} item(s) matching ‘{safe_display_name(query)}’."
        else:
            status = f"{len(visible)} item(s)."
        status += " Read-only: files are not opened or executed."
        if self.truncated:
            status += f" Showing the first {self.reader.limit} items from this folder."
        self.status_label.setText(status)

    def sort_by_column(self, column: int) -> None:
        if column == self.sort_column:
            self.sort_descending = not self.sort_descending
        else:
            self.sort_column = column
            self.sort_descending = False
        self.apply_filter()

    def activate_row(self, row: int, _column: int) -> None:
        if row < 0 or row >= self.table.rowCount():
            return
        name_item = self.table.item(row, 0)
        if name_item is None:
            return
        entry_index = name_item.data(Qt.ItemDataRole.UserRole)
        if not isinstance(entry_index, int) or not 0 <= entry_index < len(self.entries):
            self.status_label.setText("The selected item is no longer available. Refresh the folder.")
            return
        entry = self.entries[entry_index]
        if entry.is_symlink:
            self.status_label.setText("Symbolic links are not opened in this preview.")
        elif entry.kind == "Folder":
            self.navigate(entry.path)
        elif entry.kind == "File" and entry.path.suffix.casefold() == ".exe":
            try:
                command = build_controller_preview_request(
                    self.reader, self.current, self.entries, entry
                )
                response = request_controller_preview(command)
            except (OSError, ValueError, RuntimeError, subprocess.TimeoutExpired) as error:
                self.status_label.setText(f"Preview unavailable: {error}")
                return
            QMessageBox.information(
                self,
                "Windows application preview",
                f"Selected: {entry.name}\n\n"
                f"{response['explanation']}\n"
                "Windows application execution is not enabled.",
            )
        else:
            self.status_label.setText(
                f"Selected {entry.name}. File opening is not enabled in this read-only preview."
            )
