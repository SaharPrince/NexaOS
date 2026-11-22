# NCE-006C: NexaOS Graphical Task Manager

## Status

**NCE-006C.1 read-only monitor implementation merged to `main`; full stage acceptance remains open.** The first Qt monitor and bounded procfs collector are implemented. Automated unit, headless UI, ISO-build, and QEMU boot-smoke checks passed on the reviewed PR head. Interactive desktop validation and physical-device resource measurements remain outstanding. Later process-control and startup-management stages remain separate review gates.

## NCE-006C.1 implementation evidence (2026-11-22)

- Implementation: [PR #16](https://github.com/SaharPrince/NexaOS/pull/16), merged as `07417f90e4509b05df610338463a718760d048ee`.
- Reviewed PR head: `1aa71e6efd75793387a5b824f19953a6d23b7e6b`.
- Automated desktop workflow [#44997769398](https://github.com/SaharPrince/NexaOS/actions/runs/44997769398) passed the task-manager tests, Qt headless smoke test, Debian 13 LXQt ISO build, QEMU boot smoke, and evidence upload. Rust tests, formatting, Clippy, dependency audit, and parser fuzzing also passed.
- Implemented process name, PID, owner, state, per-process CPU and resident memory, aggregate CPU, memory/swap, uptime, CPU model, logical CPU count, and kernel version.
- Process enumeration and procfs reads are bounded. Sampling starts only while the window is visible and stops when hidden, minimized, or closed. Search filters the current in-memory snapshot.
- Failed samples clear old metrics and display an unavailable state rather than leaving stale values visible.
- The implementation does not read command lines, environment variables, process memory, credentials, browser data, or file contents. It has no process-control or startup-management operations.
- Remaining scope: per-core CPU visualization and a separate process-details view are not yet implemented. These remain part of the broader task-manager design and require focused follow-up work.
- The QEMU check is a 150-second graphical boot/framebuffer smoke test, not proof of interactive desktop login or hardware compatibility.
- Physical low-memory CPU/RAM overhead, large-process-list responsiveness, display scaling, keyboard navigation, and session-level interaction remain unverified.

## Product goal

Provide a native-feeling graphical task manager that makes system activity understandable and gives users safe, predictable control. It should feel familiar to Windows users without copying unsafe assumptions or requiring permanent administrator privileges.

The task manager is part of the complete desktop experience. It must not become a heavyweight background service: it is an on-demand application that samples only while open and stops sampling when closed.

## Scope and stages

### NCE-006C.1 — Read-only process and performance monitor

The first implementation is deliberately read-only.

Required views:
- **Processes:** process name, PID, user, CPU usage, resident memory, and state where available.
- **Performance:** total CPU utilization, memory used/available, swap use, and basic per-core CPU view where reliable.
- **Details:** executable name, process owner, start time, and command line only when safely available and appropriate. Do not display environment variables, credentials, or sensitive command-line arguments by default.
- **System:** CPU model, logical processors, total memory, kernel version, and uptime, with unavailable fields shown honestly.

Behavior:
- Show clear loading, unavailable, permission-denied, and stale-data states.
- Refresh on a conservative default interval (target 1 second for process rows and 1–2 seconds for system graphs); allow a slower interval and pause. Do not poll continuously when minimized if the platform reports that state reliably.
- Sort and filter the current snapshot in memory; do not rescan the entire process table for every keystroke.
- Avoid opening every process executable or reading arbitrary process memory.
- Bound per-refresh work, string lengths, row counts, and retained history. Do not keep unbounded per-process samples.
- Use monotonic time for interval calculations. CPU percentages must account for elapsed time and must not report fabricated values on first observation.
- Handle process exit, PID reuse, permission changes, disappearing procfs entries, and malformed procfs data without crashing.
- No privileged helper, daemon, kernel module, or network access is required for the read-only stage.

### NCE-006C.2 — Safe same-user process actions (separate review gate)

Process termination is intentionally excluded from the first implementation. It may be added only after a separate code review and tests establish all of the following:

- The action is explicit, visibly destructive, and requires a confirmation dialog naming the selected process and PID.
- The target is revalidated immediately before the action; a stale PID must never cause a different process to be signalled.
- Prefer Linux pidfd_open and pidfd_send_signal where supported to bind the action to a process identity. Any fallback must demonstrate race-safe identity checks or disable the action.
- By default, only processes owned by the current user may be signalled. No automatic elevation, root helper, or broad process-control permission is allowed.
- Never offer termination for PID 1, kernel threads, the task manager itself, or protected system/session components. If classification is uncertain, disable the action.
- Offer a graceful termination request first. Force termination, if ever exposed, requires a second explicit confirmation and a clear explanation.
- Handle permission denial and process exit without retries that could target another process.
- Tests cover PID reuse, permission boundaries, self-protection, process exit races, and unsupported kernel interfaces.

### NCE-006C.3 — Startup applications and resource history (future gate)

Startup management and historical graphs are not part of the first implementation. They require separate design for user/system scope, autostart file provenance, enable/disable rollback, and bounded storage. Do not delete autostart entries or change service state in the initial task manager.

## Resource and privacy budgets

- No resident service; monitoring starts only when the user opens the application.
- Default to a low-frequency bounded sample loop and stop it on close.
- Read only kernel-provided summary interfaces needed for the displayed metrics.
- Do not collect process arguments by default; do not read environment blocks, memory contents, browser data, or credentials.
- Do not transmit telemetry or process data.
- Keep history in memory only, with a strict fixed-size ring buffer; do not persist process activity.
- Profile task-manager idle/open/refresh behavior on the low-memory reference device before setting final numeric budgets.

## Architecture recommendation

- Keep the UI in the existing Qt 6 desktop application stack for consistency with NexaOS Welcome.
- Separate a pure, testable process-snapshot parser/model from the Qt presentation layer.
- Use Linux procfs/sysfs read-only interfaces with explicit parsing bounds and graceful handling of unavailable fields.
- Avoid introducing a long-running Rust or Python daemon for this UI. If a Rust collector is later justified by measured performance or security needs, expose a narrow, versioned, bounded interface and keep the UI functional when the helper is unavailable.
- Do not add a new dependency until its maintenance, package availability in Debian 13, license, attack surface, and runtime cost are reviewed. A small read-only procfs collector may be preferable to a broad process-control dependency.

## Validation

### Unit tests
- Parse valid and malformed /proc/<pid>/stat records, including process names containing spaces and parentheses.
- Reject negative, overflowing, truncated, and implausibly large numeric fields.
- Verify CPU calculations across first sample, zero elapsed time, counter reset, process exit, and PID reuse.
- Verify memory units and missing/zero values.
- Bound process count, text lengths, history length, and refresh duration.
- Ensure sensitive command-line/environment data is not exposed by default.

### Integration tests
- Use synthetic procfs fixtures; do not depend on host process ordering or exact CPU percentages.
- Exercise rapidly disappearing process entries and permission-denied files.
- Verify the UI remains responsive when process enumeration is delayed or incomplete.
- Verify closing the window stops the sampling timer and releases snapshots.
- Run Qt headless UI construction and keyboard-navigation smoke tests in CI.

### Manual validation
- Inspect CPU/RAM overhead while closed and open on a documented low-memory physical device.
- Check display scaling, keyboard navigation, accessibility labels, and large process lists.
- Confirm that no process-control capability exists in the NCE-006C.1 build.

## Acceptance criteria for NCE-006C.1

- A user can open the task manager from NexaOS Welcome and the desktop application menu.
- Process and system views display truthful, permission-aware information.
- Sampling is bounded, on-demand, and stops when the app closes.
- The UI remains responsive with large or rapidly changing process lists.
- No process can be signalled, stopped, or modified by the first-stage application.
- No elevated privileges, background daemon, telemetry, or network access is introduced.
- Unit tests, headless UI tests, formatting/lint checks, desktop image build, and QEMU session smoke checks pass.
- The implementation is compared against NXP-001 resource baselines when those are available; until then, no numerical low-resource claim is made.

## Explicit non-goals

- Killing or suspending processes in the first release.
- Editing services, startup entries, kernel parameters, or system configuration.
- Reading process memory, environment variables, browser sessions, or credentials.
- Installing kernel modules or privileged monitoring agents.
- Claiming Windows Task Manager feature parity before each feature is implemented and validated.

## Dependency and release notes

NCE-006C.1 may be implemented after the desktop preview's session-level smoke checks are reliable. It does not depend on or enable Windows PE execution. NCE-006C.2 requires its own review gate and must not be bundled into the read-only monitor implementation.
