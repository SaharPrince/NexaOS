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
- The QEMU check is a 150-second graphical boot/framebuffer smoke test. On the reviewed PR head it captured 1280×800 output with 3,590 unique colors; the ISO SHA-256 is `97a20da3d7601cb07ca8e77df09f1aa09f430f04d150793e2dfa6a331306bd39`. This is not proof of interactive desktop login or hardware compatibility.
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
- **Details:** executable name, process owner, start time, and application identity where available. Do not display command lines, environment variables, credentials, or process memory in the initial implementation. Any future command-line view requires a separate privacy review and reliable secret redaction.
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

### NCE-006C.2 — Safe managed-application actions (separate security gate)

The goal is to let users manage applications they intentionally launched without turning Task Manager into a system-administration tool. This stage is **not** a general-purpose process killer.

#### Allowed actions

- Request graceful close of an eligible NexaOS-managed application.
- Restart an eligible application only through its recorded launcher entry and original validated launch configuration.
- Show a clear result when an app exits, refuses to close, is already gone, or cannot safely be controlled.
- Force-close is not part of the initial NCE-006C.2 scope. It would require a later explicit design and a second confirmation.

#### Hard exclusions

Task Manager must never offer or perform actions against:

- Kernel threads, PID 1, system services, system/session infrastructure, login/session managers, desktop shell components, or package/update services.
- NexaOS security enforcement, Action Guardian, Sentinel, credential protection, integrity verification, update/recovery trust components, or security event services.
- Drivers, protected anti-cheat components, protected game processes, or processes whose classification is unknown.
- Arbitrary processes merely because they appear in the process list or share the current user's UID.

A disabled button is not an enforcement boundary. The UI is untrusted for authorization purposes. The OS must reject disallowed requests even if a caller bypasses the Task Manager interface.

#### Required authorization architecture

- Limit the action target to an application instance launched and registered by the NexaOS application launcher. Do not accept a caller-supplied PID as sufficient authority.
- Give each managed application a stable instance identity, launch provenance, and an OS-managed containment scope. Bind actions to that identity and scope, not a process name or a stale PID.
- Keep protected system and security components outside application-control scopes and under distinct OS-managed service identities. Security components must not share an application-control identity or be controllable through the user-app control interface.
- Enforce the allow/deny decision in an OS-owned component independent of the Task Manager UI. The authorization policy must be root-owned or otherwise protected from ordinary applications, read-only to the UI, and fail closed if missing, corrupt, stale, or unavailable.
- The control interface must expose only a narrow operation such as `request_close(managed_instance_id)`; it must not expose arbitrary PID signalling, arbitrary command execution, service management, privilege elevation, or generic filesystem writes.
- Prefer pidfd-based process identity binding where supported. Revalidate the registered application instance and containment scope immediately before acting. PID reuse or instance mismatch must cause a safe refusal.
- No broad `CAP_KILL`, root shell, unrestricted privileged helper, or general-purpose root D-Bus method. If the required OS enforcement cannot be achieved without such broad authority, defer the feature rather than weakening the boundary.
- The UI must not be able to register arbitrary processes as managed applications, change protected classifications, edit the policy, or delegate control authority.
- A failed, timed-out, crashed, or compromised control component must not fall back to raw PID signalling or unsandboxed control.
- Record only minimal, privacy-preserving action metadata (managed instance ID, requested action, result, timestamp). Never log command lines, tokens, file contents, or credentials.

#### User interaction

- Make the target application name and instance identity visible before confirmation.
- Explain that the action asks the application to close and may cause unsaved work to be lost.
- Require explicit confirmation for each close request; do not batch-close applications by default.
- If the application does not close, explain the safe next step. Do not silently escalate to force termination.
- Never present a system/security process as an eligible target. If eligibility cannot be proven, show it as protected or unavailable.

#### Mandatory tests before implementation

- UI bypass: direct API calls cannot control arbitrary PIDs or unregistered applications.
- Protected-target denial: PID 1, kernel threads, system services, session infrastructure, security components, credential services, update services, and protected anti-cheat processes are denied.
- Identity safety: PID reuse, process exit races, stale instance IDs, scope replacement, and launcher restart cannot redirect an action to a different process.
- Policy safety: missing, malformed, downgraded, or unavailable policy fails closed.
- Privilege review: no broad kill capability, root shell, arbitrary command execution, or generic privileged operation is available to the UI.
- Isolation: an ordinary application cannot register itself as trusted, alter protected classifications, or invoke the broker to affect another application's private state.
- Failure containment: broker crash, timeout, IPC truncation, malformed messages, and resource exhaustion leave protected processes untouched.
- UX: confirmation, refusal, unsaved-work warning, and close-result states are accessible and understandable.

Acceptance requires adversarial tests and a security review of the enforcement component, not merely UI tests. Until then, NCE-006C.2 remains design-only and the shipped Task Manager stays read-only.

### NCE-006C.3 — User startup applications and resource history (future gate)

Startup management and historical graphs are not part of the first implementation and require a separate review gate.

- Startup controls may manage only entries explicitly created and owned by the current user in that user's autostart configuration.
- Do not edit, mask, disable, delete, or change system-provided autostart entries, systemd system units, init scripts, login/session managers, package/update services, drivers, or any NexaOS security component.
- Do not offer system-wide startup management, service control, scheduled-task administration, kernel-module management, or privileged configuration changes.
- If an entry's ownership or provenance is uncertain, show it as unavailable and make no change.
- Changes must be reversible, preserve the original entry and metadata, and fail closed if rollback cannot be guaranteed.
- Resource history should remain bounded and in memory by default. Any persistent history requires a separate privacy review, explicit user choice, and bounded retention.
- Do not collect command lines, environment variables, credentials, browser/session data, or file contents for resource history.

The future startup UI is an application convenience, not a system-administration console. Do not delete autostart entries or change service state in the initial Task Manager.

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

- General-purpose process killing or suspending.
- Any control of system services, session infrastructure, kernel components, or security-critical components.
- Editing services, startup entries, kernel parameters, or system configuration.
- Reading process memory, environment variables, browser sessions, or credentials.
- Installing kernel modules or privileged monitoring agents.
- Claiming Windows Task Manager feature parity before each feature is implemented and validated.

## Dependency and release notes

NCE-006C.1 may be implemented after the desktop preview's session-level smoke checks are reliable. It does not depend on or enable Windows PE execution. NCE-006C.2 requires its own review gate and must not be bundled into the read-only monitor implementation.
