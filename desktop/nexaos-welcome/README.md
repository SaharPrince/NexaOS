# NexaOS Welcome

The first graphical NexaOS desktop application for the Debian 13 + LXQt preview.

## Included in this prototype

- A compact Qt 6 welcome hub exposed through the LXQt application menu.
- A home-confined file browser with folder navigation, hidden-file toggle, folder-first sorting, bounded directory listings, and a narrowly scoped **New folder** operation.
- Appearance, display, keyboard/mouse, and system-settings shortcuts.
- A Windows executable file chooser that displays a preview-only notice.
- A headless self-test mode used by CI to construct the UI and verify safe file-browser and non-execution preview behavior.

## Security boundary

- Settings utilities are started only through a fixed allowlist of absolute system executable paths and passed as argument arrays to subprocess.Popen.
- The built-in browser starts in the logged-in user's canonical home folder and refuses navigation outside that root. It walks directories with descriptor-relative O_NOFOLLOW opens, does not follow symbolic links, and caps each visible listing at 1,000 entries.
- No shell is invoked and user-provided filenames are never used to construct a command.
- The browser does not open, modify, rename, delete, copy, move, or execute existing files. Double-clicking a folder navigates; selecting a regular file only shows a status message; a .exe selection uses the preview-only Rust controller.
- The sole write operation is creating a new child folder in the current directory. The name must be one valid path component; creation is descriptor-relative, does not follow symbolic links, and never overwrites an existing item.
- The Windows .exe chooser is informational only. It does not open, inspect, load, or execute the selected file.
- No file association is registered for .exe files.
- The Python browser calls a fixed, unprivileged Rust controller helper using bounded JSON, a structured argument array, closed inherited file descriptors, and a short timeout. The helper remains preview-only and never starts a process.
- The app does not elevate privileges or modify security policy.

## Build

The live image installs Debian's python3 and python3-pyqt6 packages. The canonical application modules are staged by scripts/build-live-image.sh into the live-build chroot overlay and launched by the NexaOS Welcome desktop entry.

Run the non-interactive UI smoke test on a system with PyQt6 installed:

```sh
QT_QPA_PLATFORM=offscreen python3 desktop/nexaos-welcome/nexaos_welcome.py --self-test
```

This is an early graphical integration slice, not a complete custom desktop shell, installer, Windows compatibility runtime, or executable launcher.

## Planned desktop work

The graphical task manager is specified separately in [NCE-006C](../../docs/milestones/NCE-006C-task-manager.md). Its first stage is read-only process and performance monitoring; process termination is explicitly excluded until a separate safety review.


## Read-only task manager (NCE-006C.1)

The NexaOS Task Manager is an on-demand Qt application opened from NexaOS Welcome and the desktop application menu. Its first stage reads bounded Linux procfs summaries to show process names, PIDs, owners, CPU usage, resident memory, state, and basic system CPU/memory/swap information.

- Sampling runs only while the task-manager window is visible and stops when it is hidden, minimized, or closed.
- Process filtering uses the in-memory snapshot; it does not rescan procfs for each keystroke.
- The collector caps process enumeration and record sizes, handles processes disappearing during reads, and does not read command lines, environment variables, process memory, credentials, or browser data.
- The NexaOS Task Manager is read-only: it has no kill, suspend, priority, startup, or service controls.
- The existing Debian QPS utility remains a separate upstream application in the preview image and appears as **QPS Process Monitor**. It retains upstream process controls and runs with the current user's privileges; the NexaOS read-only guarantee applies only to the separate NexaOS Task Manager.
- Resource use must still be measured on a documented low-memory physical device before making performance claims.
