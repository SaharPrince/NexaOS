# NexaOS Product and Engineering Roadmap

## Product contract

NexaOS is intended to be a complete, customizable, Windows-familiar operating system built on Linux, with its own Rust-first Windows compatibility engine.

The project has five co-equal product requirements:

1. **Broad Windows compatibility:** run the widest practical range of modern Windows applications and games, including anti-cheat-protected multiplayer titles where providers and publishers offer an approved path.
2. **Performance and efficiency:** deliver responsive operation, low idle CPU/RAM use, efficient storage and graphics paths, and good behavior on modest hardware.
3. **Customization and usability:** provide a polished desktop with meaningful control over appearance, layout, panels, taskbar, windows, shortcuts, and optional components.
4. **Security and resilience:** contain application failures and compromise, minimize privilege, protect system integrity, and provide trustworthy updates and recovery.
5. **Privacy and account protection:** keep personal files, credentials, browser sessions, gaming sessions, and private activity outside applications' ambient reach.

**Optimization rule: keep the features; improve the engineering.** Do not remove useful capabilities merely to improve a benchmark. Prefer efficient algorithms, bounded caches, lazy initialization, on-demand services, shared components where safe, hardware acceleration, and measured resource management. Any trade-off must be documented and must not silently weaken security, compatibility, privacy, accessibility, or user choice.

Linux provides the kernel, driver ecosystem, process and memory management, filesystems, networking, and isolation primitives. Rust is the primary language for new system-critical components because it combines low-level control with memory-safety guarantees in safe code. Use existing mature components or C/C++ interop where that is safer or more compatible than rewriting them. Neither Linux nor Rust automatically provides Windows API compatibility: that must be implemented and tested.

Anti-cheat is a first-class compatibility workstream, but only through provider/publisher-approved support, certification, signing, and documented configurations. Never bypass, spoof, disable, tamper with, or evade anti-cheat, DRM, protected processes, or security drivers. Do not claim universal compatibility or zero vulnerabilities.

## Current state and sequencing

This roadmap is based on the repository state reviewed on 2026-11-22.

- **NCE-005 — PE evidence:** the inspection and non-executing image-preparation pipeline has synthetic and compiler-produced fixture coverage plus benchmark evidence. The remaining representative-corpus and evidence-based compatibility decisions are still open.
- **NCE-006 — desktop:** the repository has a Debian/LXQt preview, Welcome hub, Rust preview-only desktop controller, and merged read-only Task Manager. The Task Manager's unit/UI tests, ISO build, and QEMU framebuffer smoke passed on the reviewed PR #16 head. This is not proof of an interactive, hardware-validated desktop; physical-device resource measurements and session-level interaction remain open. Real PE execution remains disabled.
- **NCE-007 — security and privacy:** PR #12 has been merged. The threat model, security invariants, privacy requirements, and implementation gates are documented on `main`; they are design requirements, not implemented runtime protections.

Work may proceed in parallel only where dependencies are respected. In particular, no real Windows application execution is enabled until the required NCE-007 design and implementation gates are accepted and tested.

## Milestones

### Gate 0 — Close NCE-005 evidence decisions

**Purpose:** finish the evidence needed to make sound PE inspection and image-preparation decisions without turning this work into an open-ended benchmark project.

Acceptance criteria:
- Every retained third-party PE sample has recorded source, revision/version where available, license review, architecture, and cryptographic hash.
- The sample set covers the specific unresolved layout and metadata cases identified by NCE-005, or documents why a case cannot be sourced.
- Tests distinguish specification requirements, observed real-world layouts, and deliberate current planner behavior.
- Any planner-policy change is justified by evidence and has focused regression tests.
- Benchmark reports identify host, toolchain, fixture, iterations, and limitations; synthetic microbenchmarks are not presented as whole-OS or application performance.
- NCE-005 explicitly records remaining unknowns and is closed only when its own acceptance criteria are met.

### Gate 1 — Validate the desktop foundation (NCE-006)

**Purpose:** turn the current preview into a usable and measurable desktop foundation while keeping the Windows launcher preview-only.

Acceptance criteria:
- The live image boots to a usable desktop session in automated VM testing, with assertions for session readiness rather than merely a nonblank framebuffer.
- Keyboard, pointer, panel, application menu, file manager, settings, notifications, window management, and logout/shutdown paths have smoke or integration coverage.
- The welcome app and preview launcher never execute selected PE files or build shell commands from user-controlled paths.
- File-manager enumeration and file operations have explicit path, symlink, permission, and error-handling tests.
- A physical-device test record covers at least one representative low-memory machine and one graphics/device configuration; untested hardware limitations are documented.
- A graphical task manager provides read-only process/performance monitoring. Future application-management actions are restricted to explicitly registered NexaOS-managed application instances, with OS-enforced protection for system, session, and security components; no general-purpose process killer or broad privileged helper is allowed. Any future startup controls are limited to user-owned autostart entries and cannot modify system services or security components.
- Desktop customization has a user-facing settings design for themes, icons, wallpaper, panel/taskbar layout, shortcuts, window behavior, and optional widgets/components.
- The default experience stays efficient, while optional customization remains available without loading every optional component at startup.
- No desktop milestone claims Windows application execution.

### Gate 2 — Complete the NCE-007 security and privacy design

**Purpose:** establish the threat model, data flows, isolation boundaries, broker permissions, credential boundaries, and update/recovery model before executable runtime work.

Acceptance criteria:
- NCE-007A documents protected assets, attacker capabilities, trust boundaries, sensitive data flows, assumptions, and residual risks.
- NCE-007B demonstrates unprivileged per-application process and filesystem isolation with adversarial tests for path traversal, symlink races, descriptor leakage, inherited environment, and cross-app access.
- NCE-007C defines bounded, authenticated IPC with message limits, timeouts, cancellation, replay handling, and broker crash behavior.
- NCE-007D proves by synthetic secret-canary tests that browser cookies, password stores, MFA material, keyrings, SSH/API keys, game sessions, and secrets do not leak into app prefixes, arguments, environment, logs, crash reports, or telemetry.
- Telemetry is off by default; optional diagnostics are opt-in, minimized, redacted, bounded, and revocable.
- Isolation setup fails closed. No unsandboxed fallback is available.
- Linux mechanisms are selected against a documented supported kernel baseline and tested in the actual target environment; enabling one primitive alone is not accepted as proof of a secure sandbox.
- NCE-007E runtime execution remains a separate design/review gate. No real PE execution is authorized merely by completing the design document.
- NCE-007F defines authenticated updates, rollback protection, interrupted-update recovery, and trust-root handling.
- NCE-007G records official anti-cheat/provider paths and blockers without implementing bypasses.
- NCE-007H defines vulnerability disclosure, triage, patching, supported release policy, and recovery exercises.

### Gate 3 — Establish whole-OS performance budgets (NXP-001)

**Purpose:** create repeatable baselines before adding more always-running services or runtime complexity.

Acceptance criteria:
- Define representative hardware tiers from measured devices, including a modest/low-memory target; do not invent minimum specifications.
- Publish reproducible measurements for cold boot to usable session, login/session readiness, shutdown, idle CPU, idle RAM, memory pressure, storage footprint, background service count, and desktop interaction latency.
- Measure the desktop alone, desktop plus task manager, compatibility runtime idle, and representative active workloads separately.
- Record cold and warm application launch latency once a safe test application path exists.
- Record GPU/compositor frame pacing and input responsiveness on supported graphics configurations.
- Every measurement includes device/CPU/RAM/storage/GPU, kernel, driver, desktop session, power mode, tool versions, test duration, and repetitions.
- Use repeated runs and report median plus spread/percentiles; separate hosted CI results from physical-device results.
- Set resource budgets only after baseline measurements and user-experience requirements are reviewed.
- CI catches material regressions with stable, non-flaky thresholds; noisy hardware metrics are reported as artifacts rather than used as false precision gates.
- Optimizations preserve feature behavior and pass security, compatibility, accessibility, and regression tests.

### Gate 4 — Deliver the customizable desktop experience (NXP-002)

**Purpose:** provide a complete and coherent desktop, not merely a bootable preview.

Acceptance criteria:
- Deliver a graphical task manager with process/resource views, startup entries where safely supported, and permission-aware process actions.
- Deliver central settings for appearance, themes, icons, wallpaper, panels/taskbar, keyboard shortcuts, window behavior, display, and accessibility.
- Support multiple coherent desktop presets without fragmenting the core UI or duplicating background services.
- Settings persist safely and can be reset; invalid configuration must not make the desktop unbootable.
- Optional widgets and components are on-demand and have bounded resource use.
- Keyboard navigation, readable scaling, screen-reader semantics where supported, and reduced-motion options are tested.
- Compare idle and active resource measurements against NXP-001 baselines.
- No feature is removed merely to meet a resource target; improve implementation or document an evidence-based trade-off.

### Gate 5 — First secure Windows user-mode execution slice (NCE-008)

**Purpose:** move from data-only PE preparation to the smallest controlled, testable Windows application execution path.

This gate is blocked until NCE-007A through NCE-007D are reviewed and accepted, and NCE-007E receives a separate implementation approval.

Acceptance criteria:
- A separate implementation plan specifies loader, executable memory, process lifecycle, dependency policy, and rollback/cleanup behavior.
- The first executable test is a tiny project-owned, reproducibly built, non-networked test program—not an unknown third-party binary.
- The test runs under an unprivileged identity with per-app filesystem and process isolation.
- W^X and executable-memory transitions are explicitly reviewed and tested; no writable-and-executable mapping is accepted as a shortcut.
- No ambient access to host credentials, browser profiles, keyrings, unrelated app data, or primary user sessions exists.
- Failure, cancellation, malformed input, missing dependency, and sandbox setup failure leave no unsandboxed process or leaked resources.
- End-to-end tests verify process exit, signals, memory cleanup, file boundaries, and host responsiveness.
- Real-world application compatibility claims remain absent until separate application/version tests pass.

### Gate 6 — Expand Windows API and application compatibility (NCE-009)

**Purpose:** grow the compatibility engine from the first vertical slice based on actual application requirements.

Acceptance criteria:
- Maintain a versioned API-coverage matrix tied to test programs and observed application requirements.
- Implement process/thread lifecycle, virtual memory, synchronization, file semantics, registry behavior, environment/command-line rules, DLL loading, and required Win32/NT user-mode behavior in independently testable slices.
- Each API slice includes positive, negative, malformed-input, concurrency, and resource-limit tests where applicable.
- DLL search and dependency resolution are deterministic, provenance-aware, and confined to approved application/runtime locations.
- Use reproducible test applications and appropriately licensed compatibility fixtures.
- Track application install, launch, core workflow, update, crash, and uninstall separately; opening a window is not a compatibility pass.
- Add compatibility coverage without bypassing application licensing, security controls, or publisher restrictions.

### Gate 7 — Graphics, gaming, devices, and approved anti-cheat (NCE-010)

**Purpose:** extend the runtime to demanding graphics and game workloads while preserving system integrity.

Acceptance criteria:
- Document supported graphics APIs, translation layers, driver combinations, and hardware capabilities.
- Test rendering correctness, GPU/CPU utilization, frame-time distributions, input latency, audio, controllers, display modes, suspend/resume, and stability.
- Compare representative workloads against a documented reference environment using repeatable settings; do not claim performance parity without measurements.
- Maintain a per-game matrix including exact version, hardware, driver, online mode, anti-cheat provider/version, approval status, test date, and limitations.
- Anti-cheat tests use provider/publisher-approved arrangements only. If no approved path exists, mark the title blocked or unsupported and state why.
- Never bypass, spoof, disable, tamper with, or evade anti-cheat, DRM, protected processes, or security drivers.

### Gate 8 — Installable, updateable, recoverable release (NXP-003)

**Purpose:** make the system safe to install, update, troubleshoot, and maintain.

Acceptance criteria:
- Reproducible image/package builds include a software bill of materials and verified package provenance.
- Installer validates target selection and provides a clear recovery path; destructive disk operations require explicit user confirmation.
- Updates are authenticated, atomic or recoverable, rollback-protected, and tested under interruption and low-storage conditions.
- Secure boot and trust-chain behavior are documented for supported hardware; unsupported configurations are stated.
- Hardware support matrix covers boot, graphics, networking, audio, Bluetooth, input, storage, suspend/resume, and multiple displays.
- Accessibility, localization, diagnostics consent, privacy defaults, vulnerability response, and supported update windows are documented.
- Release claims are limited to tested hardware, applications, games, and security properties.

## Cross-cutting engineering rules

- **Rust-first, not Rust-only:** use Rust for new system-critical code where it improves safety and maintainability; use mature existing components and FFI where they provide better compatibility or reliability.
- **No premature hardware claims:** Linux driver availability is a foundation, not proof that every device works.
- **No feature-count optimization:** retain useful functionality and optimize algorithms, allocation, service lifecycle, rendering, and data movement.
- **On-demand by default:** avoid unnecessary resident daemons and load optional components only when needed, without breaking responsiveness or security.
- **No security downgrade for compatibility:** fail closed when a required boundary cannot be established.
- **No unverified marketing claims:** compatibility, security, privacy, resource usage, and performance statements must link to reproducible evidence.
- **Reviewable increments:** each milestone has a focused branch/PR, explicit acceptance criteria, CI evidence, and documented residual issues. Do not merge without project-owner approval.

## Immediate next actions

1. Complete NCE-006 desktop session validation, including the read-only task manager, and record physical-device limitations.
2. Finish the remaining high-value NCE-005 evidence decisions.
3. Convert the merged NCE-007 design into separately reviewed implementation milestones; keep Windows executable launch disabled until the required gates pass.
4. Establish NXP-001 whole-OS performance baselines before setting numerical resource budgets.
5. Apply the secure-transparency review to public documentation, source changes, and release artifacts.
6. Start later runtime milestones only when their dependencies and acceptance criteria are met.
