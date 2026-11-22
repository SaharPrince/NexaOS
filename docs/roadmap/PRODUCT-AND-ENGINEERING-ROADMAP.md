# NexaOS Product and Engineering Roadmap

## Purpose

This roadmap consolidates NexaOS's product goals and sequences the existing NCE milestones with the missing desktop, customization, whole-system performance, security, and compatibility work.

It is a planning document, not evidence that any listed capability is implemented. Existing milestone acceptance criteria remain authoritative until explicitly revised.

## Product vision

NexaOS is a Linux-based operating system, primarily engineered in Rust, intended to provide a familiar, deeply customizable desktop and the broadest practical support for modern Windows applications and games while remaining efficient, secure, and privacy-respecting.

The product has five co-equal requirements:

1. **Windows compatibility:** progressively support ordinary applications, productivity software, creative tools, launchers, and games. Anti-cheat-protected multiplayer titles are an important target, pursued only through provider- and publisher-approved paths.
2. **Performance and efficiency:** deliver responsive operation, low idle overhead, efficient memory and storage use, fast startup and application launch, and good behavior on modest hardware.
3. **Customization and user experience:** provide a polished desktop with meaningful control over appearance, panels/taskbar, application menu, windows, shortcuts, workspaces, widgets, accessibility, and optional components.
4. **Security and resilience:** isolate applications, protect host integrity, authenticate updates, contain failures, and maintain a practical vulnerability-response process.
5. **Privacy and account protection:** keep personal files, browser sessions, credentials, MFA material, keyrings, and unrelated game/account sessions inaccessible unless a narrowly scoped, understandable user action authorizes access.

No OS can promise zero vulnerabilities, universal Windows compatibility, or immunity from third-party account enforcement. NexaOS must publish tested coverage and residual limitations honestly.

## Non-negotiable engineering principles

### Optimize engineering, not the feature list

**Do not improve benchmark numbers by deleting useful functionality.** Preserve the product capabilities and improve their implementation.

Preferred techniques include efficient algorithms and data structures, bounded caches, lazy initialization, on-demand services, shared immutable resources where safe, reduced redundant I/O, efficient rendering, careful concurrency, and explicit resource lifetimes.

Optional modules may be independently installable or loaded on demand, but user choice must remain available. Do not make a feature disappear merely to reduce idle RAM or storage.

Every optimization must be checked for:
- functional and compatibility regressions;
- CPU, memory, storage, launch-time, and responsiveness effects;
- security and privacy regressions;
- behavior under memory pressure and on representative modest hardware.

Performance changes require repeatable before/after measurements. Do not invent universal hardware minimums or resource budgets from hosted CI or a single device.

### Linux and Rust

Use Linux kernel facilities and the Linux hardware-driver ecosystem as the host foundation. Use Rust as the primary language for new system and compatibility components where practical.

Rust does not automatically make a design correct or secure: review unsafe code, FFI, concurrency, parser boundaries, and kernel-facing interfaces. Reuse mature, well-supported components or C/C++ interoperability when it materially improves compatibility or reliability; do not rewrite a working ecosystem component solely to satisfy a language preference.

Linux's proximity to hardware provides control over processes, memory, filesystems, networking, graphics, and devices. Windows compatibility still requires faithful Windows API/runtime behavior, ABI correctness, graphics/audio/input translation, and extensive application testing.

### Compatibility and anti-cheat integrity

Never bypass, spoof, disable, patch, evade, or tamper with anti-cheat, DRM, protected processes, or security drivers. Pursue official provider/publisher support, certification, signing, and documented interfaces. A user-mode compatibility layer cannot transparently replace arbitrary Windows kernel drivers.

### Security before execution

The existing PE inspection and image-preparation pipeline is non-executing. Keep it that way until the required threat model, isolation, broker, credential/privacy boundaries, and a separate runtime-execution review have passed. Never silently fall back to unsandboxed execution.

## Roadmap and dependencies

The roadmap is deliberately milestone-driven. Workstreams may proceed in parallel only where their safety dependencies allow it.

### R0 — Product architecture and roadmap alignment

**Purpose:** establish one consistent description of product goals, architecture, milestone ownership, evidence, and current implementation status.

Deliverables:
- This roadmap and architecture references agree on compatibility, performance, customization, security, and privacy.
- Every feature is labeled as implemented, prototype, planned, blocked, or unverified.
- A compatibility evidence format and a whole-system performance measurement format are defined.
- No milestone is marked complete without its own acceptance evidence.

Exit criteria:
- README, architecture overview, and milestone index point to the same active plan.
- No stale statement describes a historical branch as the current active development branch.
- Product claims distinguish the live preview, QEMU smoke test, physical-hardware validation, and installed-system behavior.

### NCE-005D — Close PE evidence and compatibility-policy decisions

**Status:** existing milestone; remains open.

Purpose:
- Close the remaining provenance- and license-reviewed PE corpus evidence gaps.
- Resolve section-order/adjacency and zero-VirtualSize policy based on evidence.
- Preserve clear boundaries between synthetic tests, hosted-runner diagnostics, physical-device observations, and real-world binary evidence.

Exit criteria:
- Selected samples have provenance, license review, architecture, SHA-256, and reproducible read-only inspection results.
- Evidence-based disposition is documented for outstanding PE layout questions.
- Any implementation change is narrowly justified and regression-tested.
- Linux/Windows tests, formatting, Clippy, fuzz smoke, and dependency audit pass on the final head.
- No PE samples or entry points are executed; no loader or runtime capability is implied.

### NPE-001 — Whole-system performance and hardware baseline

**Purpose:** measure the actual NexaOS desktop/system experience separately from synthetic PE-preparation microbenchmarks.

Measure, with reproducible scripts and environment records:
- boot-to-usable-desktop time and shutdown behavior;
- idle and interactive CPU usage;
- idle, peak, and workload RAM, including memory pressure and recovery;
- storage footprint, boot-time I/O, and background I/O;
- desktop responsiveness, frame-time consistency, animation/compositor behavior, and input latency where measurable;
- native application cold/warm launch time;
- background-service count and resource contribution;
- behavior on representative modest hardware as well as a reference machine.

Required methodology:
- distinguish QEMU/live-preview results from physical hardware and installed-system results;
- record CPU, RAM, storage, GPU/driver, kernel, desktop session, power mode, and exact commit;
- use repeated fresh runs, report min/median/p90/max, and preserve raw artifacts;
- avoid comparing unlike metrics or environments;
- establish budgets only after sufficient representative measurements.

Exit criteria:
- a repeatable baseline exists for the current desktop preview;
- at least one physical target profile is documented before claiming hardware readiness;
- the team can identify the dominant resource costs rather than guessing;
- no features are removed to satisfy a benchmark.

### NCE-006C — Complete the desktop experience and customization foundation

**Status:** builds on the existing LXQt live-preview, safe file-browser, and Rust preview-controller work. The current preview is not an installed OS and does not provide real Windows application launching.

Deliverables:
- reliable graphical session startup and session-level QEMU assertions;
- usable desktop shell integration and a consistent visual system;
- a Windows-familiar taskbar/panel and application menu;
- a graphical task manager with process, CPU, memory, storage, and startup views where supported by the host;
- coherent system settings and customization controls;
- theme, color, icon, font, wallpaper, panel/taskbar, window, shortcut, and workspace customization;
- accessibility, keyboard navigation, scaling, and input behavior;
- safe file-manager integration and clear preview-only application-entry status.

Performance constraints:
- customization must not require every optional widget or service to run continuously;
- use efficient rendering and on-demand component activation;
- retain all user-facing capabilities while reducing their idle cost;
- measure the desktop before and after significant UI changes.

Exit criteria:
- session-level automated smoke tests verify the desktop is actually usable, not merely that a VM and framebuffer exist;
- core settings and customization survive a preview session where persistence is supported;
- task manager values are sourced from real host metrics and clearly labeled;
- no UI action launches a PE or bypasses the runtime-security gate;
- physical-device validation and resource results are reported separately from QEMU.

### NCE-007A–D — Security design, isolation prototype, broker, and privacy boundaries

**Status:** NCE-007 is an existing mandatory design gate; its sub-stages must be reviewed independently.

Deliverables:
- threat model and sensitive-data flow inventory;
- supported-kernel baseline and per-application identity/filesystem isolation design;
- adversarial tests for path traversal, symlink races, descriptor/environment leakage, cross-app access, and fail-closed setup;
- bounded, authenticated, versioned broker IPC with cancellation and crash handling;
- explicit, revocable file/device permissions;
- credential, browser-session, keyring, clipboard, diagnostics, and telemetry boundaries;
- performance overhead measurements for security controls.

Exit criteria:
- documented threat model and residual risks;
- per-app isolation proven by adversarial integration tests;
- no ambient access to browser/password/MFA/keyring/SSH/unrelated game-session data;
- telemetry off by default, diagnostics minimized and opt-in;
- isolation failures fail closed, with no unsandboxed fallback;
- security overhead measured without weakening required protections.

### NCE-007E — Separate runtime-execution review

**Hard gate:** no Windows executable launch until NCE-007A–D are accepted and this stage receives a separate implementation approval.

Review:
- executable-memory policy and W^X;
- process lifecycle and address-space setup;
- loader and dependency provenance/search order;
- DLL loading and API boundaries;
- crash containment, teardown, and recovery;
- security and performance test plans.

Exit criteria:
- independent review of the complete execution boundary;
- synthetic, controlled tests and disposable identities;
- documented rollback and emergency disable strategy;
- explicit approval before real PE execution is introduced.

### NCE-008 — First secure Windows user-mode compatibility slice

Purpose: deliver one deliberately small, testable end-to-end compatibility path rather than claiming broad support prematurely.

Scope:
- one supported architecture and a narrowly selected Win32 API surface;
- controlled process/thread lifecycle, virtual memory, files, synchronization, and required runtime dependencies;
- deterministic test applications with known expected behavior;
- isolated per-app state and clear permission handling;
- structured logs that never contain secrets.

Exit criteria:
- repeatable success/failure tests on the exact supported configurations;
- host isolation and crash containment tests pass;
- native-versus-compatibility performance baselines are recorded;
- no claim beyond the APIs and test programs actually validated.

### NCE-009 — Expand Windows API and application compatibility

Expand incrementally based on real application demand:
- Win32 and NT user-mode behavior;
- process/thread, memory, synchronization, filesystem, registry, COM, networking, and runtime dependencies;
- DLL search/load semantics and installer/update behavior;
- compatibility tracking by exact app/version/architecture/configuration.

Each API family needs conformance tests, negative tests, fuzzing where appropriate, and compatibility evidence before being declared supported.

### NCE-010 — Graphics, audio, input, and device integration

Deliver:
- graphics presentation and translation paths, prioritizing Vulkan and supported host drivers;
- input, controllers, keyboard/mouse, display, audio, and multimedia behavior;
- latency and frame-time profiling;
- hardware/driver compatibility matrix;
- graceful capability detection and clear unsupported-feature reporting.

Do not treat a launcher window or static menu as a working game. Test rendering, input, audio, stability, suspend/resume, and online behavior separately.

### NCE-011 — Gaming and approved anti-cheat compatibility

Begin provider research and publisher outreach early, but do not claim support without approved access.

Deliver:
- game/provider requirement inventory;
- official provider/publisher engineering and certification pathways;
- exact per-game, per-version, hardware, driver, and online-mode test matrix;
- frame-time, input, audio, networking, and stability results;
- clear supported, partially supported, unverified, and blocked statuses.

Never use bypasses, spoofing, evasion, or primary personal accounts for risky compatibility experiments.

### NCE-012 — Installation, hardware coverage, updates, recovery, and release readiness

Deliver:
- reproducible installable images and hardware detection;
- driver and firmware boundary documentation;
- authenticated updates, rollback protection, and recovery;
- accessibility, localization, documentation, and support policy;
- security disclosure and patch process;
- release testing across representative devices and workloads.

Exit criteria:
- clean installation, upgrade, rollback, and recovery are tested;
- hardware support claims match actual tested devices;
- security and compatibility limitations are published;
- release artifacts are reproducible and verifiable.

## Cross-cutting scorecard

Every release candidate and major milestone should report:

| Dimension | Required evidence |
|---|---|
| Compatibility | Exact app/game versions, host configuration, tested behaviors, limitations |
| Performance | CPU, RAM, I/O, launch time, responsiveness, frame-time where applicable |
| Customization | Available settings, persistence, accessibility, optional-component behavior |
| Security | Threat coverage, isolation tests, dependency/update integrity, residual risks |
| Privacy | Permission behavior, secret isolation, telemetry defaults, data retention |
| Reliability | Crash containment, recovery, regression tests, reproducible CI |

A milestone cannot compensate for a serious regression in another pillar by reporting a single improved benchmark.

## Current state and immediate sequence

Based on the repository evidence reviewed for this roadmap:

1. NCE-005 remains open: NCE-005a/b/c have documented work, but NCE-005d and broader corpus evidence remain.
2. NCE-006 has a live-preview build, QEMU boot-smoke evidence, a safe file-browser prototype, and a preview-only Rust controller. A complete graphical shell, task manager, deep customization, physical-device validation, and real application runtime are not established.
3. NCE-007 is a documentation/design gate. It is not implemented isolation and does not establish account protection today.
4. The latest reviewed NCE-007 branch CI passed Rust checks (including Linux and Windows workspace tests, formatting, Clippy, and external compiler PE layout checks), PE parser fuzz smoke, and dependency security audit. This validates those CI jobs only; it does not validate desktop usability, physical hardware, runtime security, or Windows application execution.

Recommended next execution order:
- Keep NCE-005D evidence closeout moving without enabling execution.
- Establish NPE-001 whole-system performance baselines on the current live preview and a physical device when available.
- Continue NCE-006C desktop, task-manager, and customization work against the preview-only launcher.
- Complete NCE-007A–D security design and adversarial validation before proposing NCE-007E.
- Only then consider the separately approved NCE-008 execution slice.

NCE-005D, NPE-001, and NCE-006C can progress in parallel where their evidence and environments permit. NCE-007's security gates remain mandatory before Windows execution. No milestone in this roadmap authorizes bypassing those gates.
