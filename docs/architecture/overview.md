# NexaOS Architecture — Initial Direction

NexaOS is planned as a lightweight Linux-based operating system with an ambitious long-term goal: run the widest possible range of modern Windows applications and games without requiring a Windows virtual machine for normal use. This includes multiplayer games using anti-cheat, pursued through legitimate provider- and publisher-approved support. The Linux kernel and its hardware-driver ecosystem provide the host foundation; Nexa's original work is the Nexa Compatibility Engine (NCE), its desktop integration, and the operating-system experience around it.

The five non-negotiable product goals are:

1. **Maximum practical Windows compatibility:** support as much modern Windows software and gaming as technically and legally possible, including anti-cheat-protected multiplayer games where vendors and publishers provide an approved path.
2. **Performance and efficiency:** deliver a responsive system with low avoidable CPU, RAM, storage, and launch overhead, including on modest hardware. Keep useful features and improve their engineering rather than removing them to improve benchmarks.
3. **Customization and usability:** provide a complete, polished desktop with meaningful control over appearance, layout, panels, taskbar, windows, shortcuts, and optional components.
4. **Strong system security and resilience:** minimize attack surface, isolate failures, prevent application compromise from becoming host compromise, detect vulnerabilities through continuous testing and review, and deliver trustworthy recovery and security updates.
5. **Privacy and account protection:** keep credentials, browser sessions, gaming accounts, personal files, and private activity inaccessible to applications unless the user explicitly authorizes a narrow capability.

Lightweight operation is an important engineering quality across all three goals: avoid unnecessary services and memory overhead, but never weaken security or break compatibility merely to achieve a smaller footprint.

No operating system can guarantee zero vulnerabilities or universal third-party compatibility. Nexa must state its tested coverage and residual risks precisely. A compatibility workaround that disables host protections, bypasses anti-cheat, or exposes credentials is not an acceptable compatibility improvement.

## Maximum compatibility strategy

The long-term target is broad support for current Windows application and game ecosystems, not a permanently limited whitelist. Coverage must be earned through reproducible testing and expanded in layers:

- Core PE/COFF parsing, process lifecycle, virtual memory, threads, synchronization, files, registry, COM, networking, and Windows user-mode APIs.
- Dynamic library loading, application manifests, runtime dependencies, installer and update behavior, and common Windows services where safe and feasible.
- Graphics, presentation, input, audio, controller, display, and performance paths across supported hardware and drivers.
- Developer tools, productivity suites, creative applications, launchers, single-player games, multiplayer games, and demanding modern titles.
- Compatibility tracking by exact application/version, architecture, hardware/driver, online mode, anti-cheat provider/version, and tested feature set.

Do not equate a successful PE parse, process start, launcher window, or offline menu with a fully working game. Record installation, startup, rendering, input, audio, online connectivity, multiplayer, anti-cheat initialization, update behavior, suspend/resume, and stability as separate test dimensions where applicable.

### Anti-cheat and publisher-controlled systems

Anti-cheat support is a first-class compatibility objective, not an excluded category. However, it is also a trust boundary controlled by third-party providers and publishers.

- Inventory the technical requirements of major anti-cheat providers and games, including user-mode components, kernel drivers, signing requirements, boot/security assumptions, and server-side policy.
- Pursue official vendor engineering cooperation, compatibility declarations, certification, signing, or provider-supported Linux paths wherever required.
- Implement only approved interfaces and supported configurations. Never bypass, spoof, disable, patch, evade, or tamper with anti-cheat, DRM, protected processes, game memory, or security drivers.
- Do not claim support when a required Windows kernel driver or publisher authorization is unavailable. A user-mode compatibility layer cannot transparently replace an arbitrary Windows kernel driver.
- Fail closed and explain the limitation when integrity checks or required support are absent. Do not offer an unsafe mode that disables host protections.
- Maintain a transparent matrix: provider/game, version, hardware, approved configuration, test date, online/multiplayer status, and known limitations.

Some games may remain unavailable until their publisher or anti-cheat provider supports the platform. This is a real external dependency, not something Nexa can ethically or safely defeat through a technical workaround.

## Strong system security and resilience

Security is a system-wide property. Design for compromise resistance, containment, detection, response, and recovery:

- Keep the trusted computing base small and explicit; minimize privileged code and always-running services.
- Apply least privilege and defense in depth to applications, desktop services, brokers, installers, updaters, and system management.
- Separate application identities, private data, processes, IPC, devices, and host resources. Fail closed if policy setup fails.
- Use memory-safe implementation languages where practical, strict input validation, fuzzing, static analysis, adversarial testing, and independent security review.
- Harden boot and update trust, package provenance, dependency resolution, signing keys, rollback protection, and recovery.
- Establish a vulnerability disclosure and response process, supported security update policy, and measurable remediation workflow.
- Treat kernel, firmware, physical access, and third-party service risks as explicit residual risks; do not claim NCE alone solves them.
- Measure attack-surface reduction and isolation with tests. "No known issue" is not proof of security, and "unbreakable" is not an acceptable claim.

## Privacy and account protection

- No ambient access from a Windows application to browser cookies, saved passwords, password-manager databases, MFA material, OS keyrings, SSH keys, API tokens, or unrelated game-launcher sessions.
- Isolate per-application state and expose host files/devices only through narrow, understandable, revocable user grants.
- Mediate clipboard, screen, microphone, camera, accessibility, and input-sensitive capabilities through scoped user intent where the platform supports it.
- Never put secrets in command-line arguments, environment variables, logs, crash dumps, or routine telemetry.
- Disable telemetry by default. Optional diagnostics must be informed, minimized, redacted, revocable, and bounded in retention.
- Do not silently proxy application traffic, profile users across apps, or transmit account identifiers to Nexa services.
- Make it possible for users to inspect and revoke permissions and understand what application data is stored.
- Clearly distinguish Nexa's host-side protections from third-party account enforcement, which Nexa cannot guarantee.

## Lightweight-system principles

- Start with a small, maintainable base system and a restrained default service set.
- Avoid always-running compatibility daemons when on-demand activation or per-application processes are sufficient.
- Keep optional components modular so users do not pay the memory or storage cost of features they do not use.
- Prefer efficient native implementations, bounded caches, lazy initialization, and predictable resource limits.
- Avoid duplicating large runtime components across applications where safe sharing is possible.
- Keep the desktop responsive under memory pressure; resource exhaustion must not turn into system-wide instability.
- Measure idle and workload behavior on representative low-end hardware. Do not publish minimum-spec or performance claims until supported by repeatable measurements.

## NCE component strategy

Planned components include PE inspection and parsing, image loading, import/export resolution, Windows x64 ABI bridging, NT-style user-mode services, Win32 APIs, registry and COM behavior, networking, audio, graphics translation, and per-application security boundaries.

These are work areas, not claims of implemented functionality. Components should be added incrementally behind clear interfaces, with compatibility tests, security tests, and resource measurements. The engine should load only the subsystems an application needs where the architecture permits it.

## Validation and compatibility evidence

Every major runtime milestone should add measurements and compatibility results as well as correctness tests. At minimum, record:

- idle RAM and CPU use with the default desktop and background services;
- incremental RAM and CPU use when the compatibility runtime is idle and active;
- cold and warm application launch time;
- CPU, memory, frame-time, rendering, and input behavior for representative workloads;
- storage footprint and cache growth;
- behavior under constrained memory and slower storage;
- security isolation, denied-access behavior, and recovery from application/runtime crashes;
- per-app and per-game compatibility results with exact versions, hardware, driver, and online/anti-cheat configuration.

Use repeatable test machines or documented hardware profiles. Compare changes against recorded baselines. Do not publish compatibility or performance claims without evidence. Prioritize broad coverage while keeping each result honest and reproducible.

## Current implementation boundaries

NCE-001 is a read-only metadata inspector. It must not map executable sections into executable memory, resolve imports for execution, load Windows DLLs, or transfer control to an entry point.

NCE-002 defines a staged PE32+ AMD64 image-layout plan. Its implementation uses bounded, non-executable memory and does not transfer control to an image entry point.

NCE-004a defines bounded PE32+ AMD64 base-relocation metadata, pure DIR64 patch planning, and all-or-nothing image-local application. This does not enable application loading or execution.

NCE-004b composes inspection, layout, bounded non-executable allocation, population, explicit-catalogue import resolution, symbolic IAT validation, and image-local relocation into a failure-atomic preparation pipeline. Its output remains non-executable prepared data; it does not write IAT values, load DLLs, set page permissions, or transfer control to an image entry point.

The [NCE-007 milestone](../milestones/NCE-007.md) is the mandatory security-design gate before NCE can launch Windows executables. Its security requirements and staged implementation gates are documented on `main`; the [NCE-007A threat-model and data-flow inventory](../milestones/NCE-007A-threat-model-data-flow.md) now makes the assets, trust boundaries, attacker assumptions, sensitive data flows, and residual risks explicit. These documents are design requirements, not implemented isolation or proof that user accounts are protected today. NCE-007B through NCE-007H remain separate implementation and review gates, and real PE execution remains disabled.
