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


def read_ini(path: Path, *, global_section: bool = False) -> configparser.ConfigParser:
    parser = configparser.ConfigParser(interpolation=None)
    raw = path.read_text(encoding="utf-8")
    parser.read_string(("[General]\n" + raw) if global_section else raw)
    return parser


def main() -> int:
    for path in (PACKAGES, PANEL, WALLPAPER, MENU_ICON, PACKAGE_LIST):
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
        "lxqt-powermanagement", "screengrab", "pcmanfm-qt-l10n",
    }
    missing_packages = required_packages - seen_packages
    if missing_packages:
        raise SystemExit(f"desktop package list is missing: {sorted(missing_packages)}")

    print("NexaOS desktop defaults, wallpaper, menu icon, panel plugins, and unique feature packages are valid.")
    return 0


if __name__ == "__main__":
    sys.exit(main())
