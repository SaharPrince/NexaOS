# NexaOS Architecture — Initial Direction

NexaOS is planned as a lightweight Linux-based operating system focused on running supported Windows applications and games without requiring a Windows virtual machine. The Linux kernel and its hardware-driver ecosystem provide the host foundation; Nexa's original work is the Nexa Compatibility Engine (NCE), its desktop integration, and the operating-system experience around it.

The project has two co-equal product priorities:

1. **Low resource use:** keep idle memory use, CPU activity, storage footprint, boot time, and background work low enough to remain practical on modest computers.
2. **Useful Windows compatibility:** implement Windows behavior carefully, test against real applications, and expand compatibility based on measured user value rather than feature-count claims.

Neither priority may be traded away silently. A compatibility feature that imposes substantial overhead needs profiling and an explicit design decision; a lightweight shortcut that breaks common application behavior is not a successful optimization.

## Lightweight-system principles

- Start with a small, maintainable base system and a restrained default service set.
- Avoid always-running compatibility daemons when on-demand activation or per-application processes are sufficient.
- Keep optional components modular so users do not pay the memory or storage cost of features they do not use.
- Prefer efficient native implementations, bounded caches, lazy initialization, and predictable resource limits.
- Avoid duplicating large runtime components across applications where safe sharing is possible.
- Keep the desktop responsive under memory pressure; resource exhaustion must not turn into system-wide instability.
- Measure idle and workload behavior on representative low-end hardware. Do not publish minimum-spec or performance claims until they are supported by repeatable measurements.
- Treat power use, thermals, storage reads, and startup latency as performance concerns alongside frame rate and application launch time.

## Nexa Compatibility Engine (NCE)

Planned components include PE inspection and parsing, image loading, import/export resolution, Windows x64 ABI bridging, NT-style user-mode services, Win32 APIs, registry and COM behavior, networking, audio, graphics translation, and per-application security boundaries.

These are work areas, not claims of implemented functionality. Components should be added incrementally behind clear interfaces, with compatibility tests and resource measurements. The engine should load only the subsystems an application needs where the architecture permits it.

## Compatibility strategy

- Prioritize common desktop application foundations first: process startup, memory, files, threads, synchronization, dynamic libraries, and core Win32 behavior.
- Build a repeatable compatibility test suite using redistributable or purpose-built test programs and appropriately licensed application fixtures.
- Track compatibility by application, version, feature set, and known limitations; avoid blanket claims that all Windows software works.
- Prefer upstreamable, well-documented behavior over application-specific hacks. Keep unavoidable workarounds isolated and testable.
- Use graphics and audio translation layers only with explicit performance and correctness testing across supported hardware.
- Preserve a clear distinction between native Linux applications and Windows applications running through NCE.

A user-mode compatibility engine can implement user-mode behavior. It cannot transparently replace arbitrary Windows kernel drivers. Multiplayer anti-cheat support is limited to configurations officially supported by the publisher and provider; NexaOS will not promise unsupported kernel-driver or anti-cheat compatibility.

## Performance and compatibility validation

Every major runtime milestone should add measurements as well as correctness tests. At minimum, record:

- idle RAM and CPU use with the default desktop and background services;
- incremental RAM and CPU use when the compatibility runtime is idle and when an application is active;
- cold and warm application launch time;
- CPU, memory, and frame-time behavior for representative workloads where applicable;
- storage footprint and cache growth;
- behavior under constrained memory and slower storage.

Use repeatable test machines or documented hardware profiles. Compare changes against a recorded baseline, and reject optimizations that materially regress compatibility or stability without a justified trade-off. Initial numeric budgets will be set only after representative low-end hardware is selected and measured; inventing targets before that would create misleading promises.

## Compatibility and security boundaries

The compatibility runtime must not execute untrusted inputs during inspection, must validate binary structures before use, and must isolate application state and privileges. Prefer least privilege and explicit access to host resources. Security boundaries and crash containment are core architecture requirements, not optional performance features.

## NCE-001 boundary

NCE-001 is a read-only metadata inspector. It must not map executable sections into executable memory, resolve imports for execution, load Windows DLLs, or transfer control to an entry point. Its purpose is to establish a safe, testable PE foundation for later loader work.
