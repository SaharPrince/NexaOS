#!/usr/bin/env python3
"""NexaOS Welcome: a lightweight LXQt-facing desktop hub.

This application provides a read-only home-folder browser and opens installed
desktop settings through fixed executable paths. It never constructs shell
commands and never executes Windows files. The .exe chooser is preview-only.
"""
from __future__ import annotations

import argparse
import os
import subprocess
import sys
import tempfile
from pathlib import Path

from PyQt6.QtCore import Qt
from nexaos_file_browser import (
    FileBrowserDialog,
    SafeDirectoryReader,
    build_controller_preview_request,
    request_controller_preview,
)

from PyQt6.QtWidgets import (
    QApplication,
    QFileDialog,
    QFrame,
    QGridLayout,
    QLabel,
    QMainWindow,
    QMessageBox,
    QPushButton,
    QVBoxLayout,
    QWidget,
)


def safe_display_name(value: str) -> str:
    """Remove control characters before showing untrusted filenames."""
    return "".join(char if char.isprintable() else "\uFFFD" for char in value)[:180]


class NexaWelcome(QMainWindow):
    PROGRAMS = {
        "lxqt-config-appearance": "/usr/bin/lxqt-config-appearance",
        "lxqt-config-monitor": "/usr/bin/lxqt-config-monitor",
        "lxqt-config-input": "/usr/bin/lxqt-config-input",
        "lxqt-config": "/usr/bin/lxqt-config",
    }

    def __init__(self) -> None:
        super().__init__()
        self.setWindowTitle("NexaOS Welcome")
        self.setMinimumSize(760, 520)
        self.resize(900, 620)

        root = QWidget(self)
        root.setObjectName("root")
        outer = QVBoxLayout(root)
        outer.setContentsMargins(32, 28, 32, 28)
        outer.setSpacing(18)

        brand = QLabel("NexaOS")
        brand.setObjectName("brand")
        subtitle = QLabel("A familiar desktop. Your way.")
        subtitle.setObjectName("subtitle")
        outer.addWidget(brand)
        outer.addWidget(subtitle)

        status = QLabel(
            "Your desktop is ready. Personalize your workspace or open your files."
        )
        status.setWordWrap(True)
        status.setObjectName("status")
        outer.addWidget(status)

        grid = QGridLayout()
        grid.setHorizontalSpacing(14)
        grid.setVerticalSpacing(14)
        outer.addLayout(grid)

        self.file_button = self._tile(
            "Open Files", "Browse documents, downloads, and folders.",
            self._open_file_browser,
        )
        self.appearance_button = self._tile(
            "Appearance", "Choose themes, icons, and desktop appearance.",
            lambda: self._open_program("lxqt-config-appearance"),
        )
        self.display_button = self._tile(
            "Display", "Adjust your screen and monitor settings.",
            lambda: self._open_program("lxqt-config-monitor"),
        )
        self.input_button = self._tile(
            "Keyboard & Mouse", "Configure keyboard and pointing devices.",
            lambda: self._open_program("lxqt-config-input"),
        )
        self.settings_button = self._tile(
            "System Settings", "Open the LXQt settings center.",
            lambda: self._open_program("lxqt-config"),
        )
        self.preview_button = self._tile(
            "Windows App Preview", "Inspect a selected .exe filename without running it.",
            self._choose_executable,
        )

        for index, tile in enumerate((
            self.file_button, self.appearance_button, self.display_button,
            self.input_button, self.settings_button, self.preview_button,
        )):
            grid.addWidget(tile, index // 2, index % 2)

        footer = QLabel(
            "NexaOS preview • Windows application execution is not enabled"
        )
        footer.setObjectName("footer")
        footer.setAlignment(Qt.AlignmentFlag.AlignCenter)
        outer.addStretch(1)
        outer.addWidget(footer)
        self.setCentralWidget(root)
        self.setStyleSheet("""
            QWidget#root { background: #101722; color: #edf3fb; }
            QLabel#brand { color: #f5f8ff; font-size: 34px; font-weight: 700; }
            QLabel#subtitle { color: #9fb4cc; font-size: 17px; }
            QLabel#status { color: #d4deea; font-size: 14px; padding: 10px 0; }
            QLabel#footer { color: #8093aa; font-size: 11px; padding-top: 10px; }
            QFrame[tile="true"] {
                background: #1a2737; border: 1px solid #2b3b50;
                border-radius: 12px;
            }
            QFrame[tile="true"]:hover {
                background: #22354a; border: 1px solid #5c8fc4;
            }
            QPushButton {
                color: #f5f8ff; background: transparent; border: 0;
                text-align: left; font-size: 16px; font-weight: 650;
                padding: 16px 16px 4px 16px;
            }
            QPushButton:focus { border: 2px solid #83b8f0; border-radius: 8px; }
            QLabel.tile-description {
                color: #a9b9ca; font-size: 12px; padding: 0 16px 16px 16px;
            }
        """)

    def _tile(self, title: str, description: str, callback) -> QFrame:
        tile = QFrame()
        tile.setProperty("tile", True)
        layout = QVBoxLayout(tile)
        layout.setContentsMargins(0, 8, 0, 0)
        button = QPushButton(title)
        button.setMinimumHeight(48)
        button.clicked.connect(callback)
        detail = QLabel(description)
        detail.setProperty("class", "tile-description")
        detail.setWordWrap(True)
        detail.setContentsMargins(16, 0, 16, 12)
        layout.addWidget(button)
        layout.addWidget(detail)
        tile.setMinimumHeight(118)
        return tile

    def _open_program(self, program: str) -> None:
        executable = self.PROGRAMS.get(program)
        if executable is None or not os.path.isfile(executable) or not os.access(executable, os.X_OK):
            QMessageBox.warning(
                self, "Not available",
                f"{program} is not installed in this desktop image.",
            )
            return
        try:
            subprocess.Popen([executable], close_fds=True)
        except OSError as error:
            QMessageBox.critical(
                self, "Could not open application",
                f"The desktop application could not be started: {error}",
            )

    def _open_file_browser(self) -> None:
        browser = FileBrowserDialog(self)
        browser.exec()

    def _choose_executable(self) -> None:
        path, _ = QFileDialog.getOpenFileName(
            self, "Choose a Windows application", str(Path.home()),
            "Windows executable (*.exe);;All files (*)",
        )
        if path:
            self.show_executable_preview(path)

    @staticmethod
    def executable_preview_text(path: str) -> str:
        # The path is display-only. It is never passed to a process or shell.
        name = safe_display_name(os.path.basename(path))
        return (
            f"Selected: {name}\n\n"
            "This NexaOS preview can identify the selected file only. "
            "Windows application execution is not available yet. "
            "No process was started and the selected file was not executed."
        )

    def show_executable_preview(self, path: str) -> None:
        QMessageBox.information(
            self, "Windows application preview",
            self.executable_preview_text(path),
        )


def main() -> int:
    parser = argparse.ArgumentParser(description="NexaOS desktop welcome hub")
    parser.add_argument(
        "--self-test", action="store_true",
        help="construct the desktop UI and run non-execution smoke assertions",
    )
    args = parser.parse_args()
    app = QApplication(sys.argv[:1])
    window = NexaWelcome()
    if args.self_test:
        assert window.windowTitle() == "NexaOS Welcome"
        assert window.file_button is not None
        assert window.preview_button is not None
        preview = window.executable_preview_text("/tmp/example\n.exe")
        assert "example" in preview
        assert "\uFFFD.exe" in preview
        assert "execution is not available yet" in preview
        assert "No process was started" in preview
        assert "not executed" in preview
        # Exercise real directory enumeration only inside a disposable fixture.
        with tempfile.TemporaryDirectory(prefix="nexaos-browser-test-") as temporary:
            root = Path(temporary)
            (root / "Documents").mkdir()
            (root / "notes.txt").write_text("preview fixture", encoding="utf-8")
            (root / "demo.exe").write_bytes(b"not an executable; preview fixture only")
            (root / ".hidden").write_text("hidden fixture", encoding="utf-8")
            (root / "folder-link").symlink_to(root / "Documents", target_is_directory=True)
            reader = SafeDirectoryReader(root)
            entries, truncated = reader.list_entries(root)
            assert not truncated
            assert [entry.name for entry in entries] == ["Documents", "demo.exe", "folder-link", "notes.txt"]
            assert next(entry for entry in entries if entry.name == "folder-link").is_symlink
            executable_entry = next(entry for entry in entries if entry.name == "demo.exe")
            command = build_controller_preview_request(reader, root, entries, executable_entry)
            assert command["action"] == "preview_selected"
            assert command["selected_relative_path"] == "demo.exe"
            assert command["snapshot"]["entries"][0]["kind"] == "directory"
            assert all(item["name"] != "folder-link" for item in command["snapshot"]["entries"])
            controller_binary = Path.cwd() / "target" / "release" / "nexa-desktop-controller"
            response = request_controller_preview(command, controller_binary)
            assert response["status"] == "preview_only"
            assert "no process was created" in response["explanation"]
            try:
                reader.validate_directory(root / "folder-link")
            except OSError:
                pass
            else:
                raise AssertionError("browser followed a symbolic link")
            limited, truncated = SafeDirectoryReader(root, limit=1).list_entries(root)
            assert len(limited) == 1 and truncated
            visible, _ = reader.list_entries(root, show_hidden=True)
            assert ".hidden" in [entry.name for entry in visible]
            try:
                reader.validate_directory(root.parent)
            except PermissionError:
                pass
            else:
                raise AssertionError("browser allowed navigation outside its root")
            browser = FileBrowserDialog(root=root)
            assert browser.table.rowCount() == 4
            assert browser.current == root.resolve()
            browser.close()
        # The self-test creates no process and displays no modal dialog.
        return 0
    window.show()
    return app.exec()


if __name__ == "__main__":
    raise SystemExit(main())
