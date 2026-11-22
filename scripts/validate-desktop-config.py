#!/usr/bin/env python3
"""Validate NexaOS's first-login visual and panel configuration."""
from __future__ import annotations

import configparser
import sys
import xml.etree.ElementTree as ET
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]
OVERLAY = ROOT / "desktop/live-build/config/includes.chroot"
PACKAGES = OVERLAY / "etc/skel/.config/pcmanfm-qt/lxqt/settings.conf"
PANEL = OVERLAY / "etc/skel/.config/lxqt/panel.conf"
WALLPAPER = OVERLAY / "usr/share/backgrounds/nexaos/nexaos-default.svg"
MENU_ICON = OVERLAY / "usr/share/lxqt/themes/nexaos/nexaos-menu.svg"
PACKAGE_LIST = ROOT / "desktop/live-build/config/package-lists/nexaos-desktop.list.chroot"
NEXA_TASK_MANAGER = OVERLAY / "usr/share/applications/nexaos-task-manager.desktop"
QPS_PROCESS_MONITOR = OVERLAY / "usr/share/applications/qps.desktop"


def read_ini(path: Path, *, global_section: bool = False) -> configparser.ConfigParser:
    parser = configparser.ConfigParser(interpolation=None)
    raw = path.read_text(encoding="utf-8")
    parser.read_string(("[General]\n" + raw) if global_section else raw)
    return parser


def main() -> int:
    for path in (PACKAGES, PANEL, WALLPAPER, MENU_ICON, PACKAGE_LIST, NEXA_TASK_MANAGER, QPS_PROCESS_MONITOR):
        if not path.is_file() or path.stat().st_size == 0:
            raise SystemExit(f"required desktop asset is missing or empty: {path}")

    for artwork in (WALLPAPER, MENU_ICON):
        ET.parse(artwork)

    desktop = read_ini(PACKAGES)
    assert desktop.get("System", "IconThemeName") == "Papirus"
    assert desktop.get("Desktop", "Wallpaper") == "/usr/share/backgrounds/nexaos/nexaos-default.svg"
    assert desktop.get("Desktop", "WallpaperMode") == "stretch"
    assert {"Home", "Trash", "Computer", "Network"} <= set(
        item.strip() for item in desktop.get("Desktop", "DesktopShortcuts").split(",")
    )

    panel = read_ini(PANEL, global_section=True)
    plugin_names = {
        item.strip()
        for item in panel.get("panel1", "plugins").split(",")
        if item.strip()
    }
    required_plugins = {
        "fancymenu", "desktopswitch", "quicklaunch", "taskbar",
        "statusnotifier", "tray", "mount", "volume", "worldclock", "showdesktop",
    }
    missing_plugins = required_plugins - plugin_names
    if missing_plugins:
        raise SystemExit(f"panel profile is missing plugins: {sorted(missing_plugins)}")
    assert panel.get("fancymenu", "icon") == "/usr/share/lxqt/themes/nexaos/nexaos-menu.svg"
    assert panel.get("fancymenu", "ownIcon").lower() == "true"

    nexa_task_manager = configparser.ConfigParser(interpolation=None)
    nexa_task_manager.read(NEXA_TASK_MANAGER, encoding="utf-8")
    if not nexa_task_manager.has_section("Desktop Entry"):
        raise SystemExit("NexaOS Task Manager desktop entry is missing its Desktop Entry section")
    nexa_entry = nexa_task_manager["Desktop Entry"]
    if (
        nexa_entry.get("Name") != "NexaOS Task Manager"
        or nexa_entry.get("Exec") != "/usr/bin/python3 /usr/lib/nexaos/nexaos_task_manager.py"
        or nexa_entry.get("Type") != "Application"
    ):
        raise SystemExit("NexaOS Task Manager launcher must start the read-only NexaOS implementation")

    qps = configparser.ConfigParser(interpolation=None)
    qps.read(QPS_PROCESS_MONITOR, encoding="utf-8")
    if not qps.has_section("Desktop Entry"):
        raise SystemExit("Upstream QPS desktop entry is missing its Desktop Entry section")
    qps_entry = qps["Desktop Entry"]
    if (
        qps_entry.get("Name") != "QPS Process Monitor"
        or qps_entry.get("Exec") != "qps"
        or qps_entry.get("Type") != "Application"
    ):
        raise SystemExit("Upstream QPS must remain clearly distinct from NexaOS Task Manager")

    package_entries = [
        line.strip()
        for line in PACKAGE_LIST.read_text(encoding="utf-8").splitlines()
        if line.strip() and not line.lstrip().startswith("#")
    ]
    if any(len(entry.split()) != 1 for entry in package_entries):
        raise SystemExit("desktop package list entries must contain exactly one package name")
    seen_packages: set[str] = set()
    duplicate_packages: set[str] = set()
    for package in package_entries:
        if package in seen_packages:
            duplicate_packages.add(package)
        seen_packages.add(package)
    if duplicate_packages:
        raise SystemExit(f"desktop package list contains duplicates: {sorted(duplicate_packages)}")

    required_packages = {
        "user-setup", "gvfs-backends", "gvfs-fuse", "papirus-icon-theme",
        "qt6-svg-plugins", "fonts-noto-core", "lximage-qt", "lxqt-policykit",
        "lxqt-qtplugin", "lxqt-archiver", "featherpad", "pavucontrol-qt",
        "lxqt-powermanagement", "screengrab", "pcmanfm-qt-l10n", "qps",
    }
    missing_packages = required_packages - seen_packages
    if missing_packages:
        raise SystemExit(f"desktop package list is missing: {sorted(missing_packages)}")

    print("NexaOS desktop defaults, wallpaper, menu icon, panel plugins, NexaOS Task Manager and upstream QPS launchers, and unique feature packages are valid.")
    return 0


if __name__ == "__main__":
    sys.exit(main())
