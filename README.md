# NexaOS

NexaOS is an experimental Linux-based operating system project with an ambitious goal: run the widest possible range of modern Windows applications and games through an independently developed compatibility engine, without requiring a Windows virtual machine for normal use.

**Long-term ambition:** make Nexa a practical OS choice for people who depend on Windows software and PC gaming—including multiplayer titles with anti-cheat—while protecting the host system, personal information, and online accounts. This is a development target, not a claim of current compatibility or a guarantee that every third-party application will work.

**Implementation status: early development.** Nexa currently has a Rust PE inspection and non-executing image-preparation foundation. It is not yet a general-purpose Windows application runtime or installable desktop OS. Compatibility, performance, hardware support, and low-memory targets remain goals—not guarantees.

## The three non-negotiable goals

### 1. Maximum modern Windows application and game compatibility

The long-term target is the broadest practical coverage of modern Windows software: productivity and creative applications, developer tools, launchers, demanding 3D applications, single-player games, multiplayer games, and titles using anti-cheat systems.

- Build a compatibility engine that progressively implements the Windows user-mode behavior applications actually require.
- Prioritize compatibility by real user demand and measured application coverage, not by the number of APIs implemented.
- Pursue support for anti-cheat-protected games, including multiplayer titles, through legitimate compatibility paths approved by the anti-cheat provider and game publisher.
- Work with vendors and publishers where their cooperation, certification, signing, or platform changes are necessary. Treat vendor relationships and official support as engineering workstreams, not assumptions.
- Never bypass, spoof, disable, tamper with, or evade anti-cheat, DRM, protected processes, game memory, or security drivers. A game that requires unsupported Windows kernel-driver behavior cannot be declared supported merely because its launcher opens.
- Do not promise universal compatibility. Some applications depend on undocumented Windows behavior, proprietary services, hardware-specific drivers, publisher restrictions, or kernel components that a user-mode compatibility layer cannot safely reproduce.
- Publish a transparent compatibility record by application, version, hardware/driver configuration, online mode, and anti-cheat status. Distinguish tested, partially tested, blocked, unsupported, and not-yet-tested results.

### 2. Strong system security and resilience

NexaOS must be engineered to make compromise difficult, limit the impact of vulnerabilities, and recover safely when something fails. No operating system can honestly promise zero security vulnerabilities; Nexa's goal is to reduce attack surface, prevent one application from compromising the host, find weaknesses continuously, and patch them quickly.

- Use least privilege, per-application isolation, defense in depth, secure defaults, and fail-closed behavior.
- Keep the kernel, boot chain, system services, package manager, compatibility broker, and update process within explicit trust boundaries.
- Validate untrusted files and IPC input; use memory-safe code where practical, fuzzing, adversarial testing, and independent security review.
- Require authenticated updates, dependency integrity, rollback protection, and recoverable installation.
- Keep security controls enabled when applications fail. Never silently fall back to an unsandboxed launch to improve compatibility.
- Maintain a vulnerability response process, security advisories, supported update windows, and a way to recover from a failed update.
- Measure security through tests and review evidence. Do not describe Nexa as unbreakable or vulnerability-free.

### 3. User privacy and account protection

NexaOS must protect users' personal files, identity, credentials, browser sessions, gaming accounts, and private activity.

- Windows applications receive no ambient access to browser cookies, saved passwords, password-manager databases, MFA material, OS keyrings, SSH keys, API tokens, or unrelated game-launcher sessions.
- Keep per-app data isolated. Host files, devices, clipboard, screen, camera, microphone, and other sensitive capabilities require scoped, understandable user permission where supported.
- Never collect or log passwords, tokens, keystrokes, clipboard contents, private documents, or screenshots for routine diagnostics.
- Keep telemetry off by default. Optional diagnostics must be disclosed, minimized, redacted, revocable, and retained only as long as necessary.
- Do not silently proxy application traffic or share account identifiers and usage data with Nexa services.
- Make permission grants, revocation, diagnostic data, and stored application data understandable and manageable by the user.
- Be honest about the boundary: Nexa can protect credentials and host data within its control, but cannot guarantee a third-party publisher will never suspend or restrict an account.

## Product experience

- **Familiar desktop:** make the desktop, file handling, settings, and everyday workflows natural for people switching from Windows.
- **Simple launching:** the intended experience is double-clicking a Windows .exe in the file manager, with no command-line setup and minimal avoidable launch delay.
- **Lightweight operation:** keep idle CPU/RAM use, background services, storage footprint, and launch overhead low, especially on modest hardware—without sacrificing compatibility or security silently.
- **User choice:** keep the default desktop lightweight while letting each person customize layout, appearance, shortcuts, window behavior, and optional desktop components.
- **Original engineering:** build Nexa's own compatibility components; do not misrepresent a repackaged Wine build as the Nexa engine.
- **Linux foundation:** use the Linux kernel and its hardware-driver ecosystem as the host foundation. A user-mode compatibility engine cannot transparently replace arbitrary Windows kernel drivers.

## Current engineering status

The repository contains the NCE PE inspection, bounded image-layout planning, non-executable image population, explicit-catalogue import modelling, relocation planning/application, and transactional image-preparation foundations. These produce data only: they do not launch Windows applications, load Windows DLLs, create executable mappings, or transfer control to PE entry points.

The [NCE-005 evidence milestone](docs/milestones/NCE-005.md) remains open for broader provenance-reviewed PE corpus and compatibility evidence. The [NCE-006 desktop milestone](docs/milestones/NCE-006.md) records the current Debian/LXQt preview, safe file-browser foundation, and preview-only application entry path. The [NCE-007 security and privacy milestone](docs/milestones/NCE-007.md) defines the mandatory threat-model, isolation, credential, privacy, and update-integrity gates that must be reviewed before real Windows execution is enabled.

## Build and inspect

Requires a stable Rust toolchain.

```sh
cargo test --workspace
cargo run -p nexa-inspect -- inspect path/to/program.exe
cargo run -p nexa-inspect -- inspect --json path/to/program.exe
```

The inspector reads the selected file as data only. **Execution: NOT ATTEMPTED.**

## Development priorities

1. Close the remaining NCE-005 evidence gates with a small, provenance- and license-reviewed sample set.
2. Continue NCE-006 desktop integration and validation against the preview-only launcher; keep real PE execution disabled.
3. Complete the NCE-007 threat model and security/privacy design, then validate unprivileged isolation, credential boundaries, IPC, diagnostics, and update integrity.
4. Design and implement the smallest safe end-to-end Windows user-mode application compatibility slice only after the NCE-007 execution gate is reviewed.
5. Build the core Windows application foundation: process lifecycle, virtual memory, files, threads, synchronization, Win32 APIs, registry/COM behavior, networking, and DLL loading.
6. Expand application coverage using a versioned, reproducible test matrix. Prioritize real productivity, creative, developer, and launcher workflows as well as demanding games.
7. Pursue anti-cheat support as a first-class compatibility workstream: map each provider's requirements, seek official support and certification, validate approved configurations, and clearly report unsupported kernel-driver or publisher restrictions. No bypass or evasion.
8. Expand graphics, input, audio, controller, display, and performance support; measure on a range of hardware.
9. Engineer installation, secure updates, recovery, hardware support, accessibility, and release-quality distribution.

Use risk-based validation: spend test effort on memory safety, bounds, privilege boundaries, data integrity, failure recovery, account isolation, and regressions that could affect a user's machine. Avoid duplicative benchmarks and low-value repeated checks. Never trade away system safety or user privacy to make a compatibility test pass.

## Continuous validation

GitHub Actions checks Rust formatting, workspace tests on Linux and Windows, Clippy, bounded parser fuzzing, and Rust dependency advisories. Expand CI with targeted adversarial security tests, sandbox integration tests, and application compatibility fixtures as those components are implemented.

Milestones are complete only when their stated acceptance criteria and evidence are satisfied. No universal Windows compatibility, anti-cheat support, account protection, or performance claim should be made without supporting evidence.
