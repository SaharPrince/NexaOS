# NexaOS Welcome

The first graphical NexaOS desktop application for the Debian 13 + LXQt preview.

## Included in this prototype

- A compact Qt 6 welcome hub exposed through the LXQt application menu.
- A NexaOS read-only file browser with folder navigation, hidden-file toggle, folder-first sorting, and bounded directory listings.\n- Appearance, display, keyboard/mouse, and system-settings shortcuts.
- A Windows executable file chooser that displays a preview-only notice.
- A headless `--self-test` mode used by CI to construct the UI and verify the non-execution preview path.

## Security boundary

- Settings utilities are started only through a fixed allowlist of absolute system executable paths and passed as argument arrays to `subprocess.Popen`.\n- The built-in browser starts in the logged-in user's home folder and refuses navigation outside that root. It walks directories with descriptor-relative `O_NOFOLLOW` opens, does not follow symbolic links, and caps each visible listing at 1,000 entries.
- No shell is invoked and user-provided filenames are never used to construct a command.
- The browser is read-only: it does not create, modify, delete, or open files. Double-clicking a folder navigates; selecting a regular file only shows a status message. A `.exe` selection shows a preview-only notice.\n- The Windows `.exe` chooser is informational only. It does not open, inspect, load, or execute the selected file.
- No file association is registered for `.exe` files.
- The Python browser is not yet connected to the Rust `DesktopController`; the controller remains a separate preview-only model/API until a reviewed IPC or in-process integration is implemented.
- The app does not elevate privileges or modify security policy.

## Build

The live image installs Debian's `python3` and `python3-pyqt6` packages. Both canonical Python modules are staged by `scripts/build-live-image.sh` into the live-build chroot overlay and launched by the `NexaOS Welcome` desktop entry.

Run the non-interactive UI smoke test on a system with PyQt6 installed:

```sh
QT_QPA_PLATFORM=offscreen python3 desktop/nexaos-welcome/nexaos_welcome.py --self-test
```

This is an early graphical integration slice, not a complete custom desktop shell, installer, Windows compatibility runtime, or executable launcher.
