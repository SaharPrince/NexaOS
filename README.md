# NexaOS

NexaOS is an experimental Linux-based operating system project focused on running
Windows user applications and games through an independently developed
compatibility engine, without requiring a Windows virtual machine for normal use.

**Long-term ambition:** make Nexa a practical OS choice for people who depend on
Windows software and PC gaming—including multiplayer titles with anti-cheat—while
protecting the host system, personal information, and online accounts. This is a
development target, not a claim of current compatibility or a guarantee that every
third-party application will work.

**Implementation status: early development.** Nexa currently has a Rust PE
inspection and non-executing image-preparation foundation. It is not yet a
general-purpose Windows application runtime or installable desktop OS. Compatibility,
performance, hardware support, and low-memory targets remain goals—not guarantees.

## Five co-equal product pillars

1. **Broad Windows compatibility:** run the widest practical range of modern
   Windows applications and games. Pursue anti-cheat-protected multiplayer titles
   through official provider/publisher-approved support only.
2. **Performance and efficiency:** keep idle CPU/RAM use, storage footprint,
   background activity, launch overhead, and graphics costs low, including on
   modest hardware. Measure improvements rather than assuming them.
3. **Customization and usability:** deliver a polished, Windows-familiar desktop
   with meaningful control over themes, icons, wallpaper, panels/taskbar, windows,
   shortcuts, accessibility, and optional components.
4. **Security and resilience:** use least privilege, application isolation,
   defense in depth, trustworthy updates, failure containment, and recoverable
   system operations.
5. **Privacy and account protection:** protect personal files, passwords, browser
   sessions, gaming sessions, keyrings, and private activity from ambient
   application access.

**Optimization principle: keep the features; improve the engineering.** NexaOS
must not remove useful functionality simply to make benchmark numbers look better.
Prefer efficient algorithms, bounded caches, lazy initialization, on-demand
services, hardware acceleration, and measured resource management. Never silently
trade away compatibility, security, privacy, accessibility, or user choice.

## Linux and Rust engineering approach

Linux provides the kernel, hardware-driver ecosystem, process and memory management,
filesystems, networking, and isolation primitives. Rust is the primary language for
new system-critical components because it combines low-level control with memory
safety guarantees in safe code. Use mature existing components or C/C++ interop
where that is safer or more compatible than rewriting them. Linux and Rust are a
strong foundation, not automatic Windows compatibility: the Windows APIs and
runtime behavior still need to be implemented and tested.

## Anti-cheat and account-safety requirements

Anti-cheat compatibility is a first-class, publisher-controlled workstream—not
something NexaOS may force by bypassing protections.

- Support anti-cheat only through legitimate, vendor- and publisher-approved
  compatibility paths.
- Never bypass, spoof, disable, or tamper with anti-cheat, protected processes,
  game memory, or security drivers.
- Fail closed when required anti-cheat support is absent, uncertain, or reports an
  integrity problem; do not silently force a launch.
- Clearly distinguish verified, unverified, and unsupported game configurations.
- Use vendor-approved test arrangements where available, not a user's primary
  gaming account.
- Treat account protection as a design objective, not an absolute guarantee:
  third-party publishers control detection and enforcement.

## Current engineering status

The repository contains the NCE PE inspection, bounded image-layout planning,
non-executable image population, explicit-catalogue import modelling, relocation
planning/application, and transactional image-preparation foundations. These
produce data only: they do not launch Windows applications, load Windows DLLs,
create executable mappings, or transfer control to PE entry points.

- **NCE-005** covers PE corpus, compatibility evidence, and resource measurements
  for the existing non-executing preparation pipeline. Its remaining evidence-based
  compatibility decisions are still open.
- **NCE-006** covers the desktop and application-entry experience. The repository
  has a Debian/LXQt preview, a welcome application, and a Rust preview-only desktop
  model/controller. This is not yet a fully validated interactive desktop or a
  real Windows application launcher.
- **NCE-007** defines the security and privacy gate before real Windows execution.
  Its design proposal is under review in [PR #12](https://github.com/SaharPrince/NexaOS/pull/12);
  proposed controls must not be mistaken for implemented runtime protections.

The detailed, dependency-aware plan and measurable acceptance criteria are in
[docs/ROADMAP.md](docs/ROADMAP.md).

## Build and inspect

Requires a stable Rust toolchain.

```sh
cargo test --workspace --locked
cargo run --locked -p nexa-inspect -- inspect path/to/program.exe
cargo run --locked -p nexa-inspect -- inspect --json path/to/program.exe
```

The inspector reads the selected file as data only. **Execution: NOT ATTEMPTED.**

## Continuous validation

GitHub Actions checks Rust formatting, workspace tests on Linux and Windows,
Clippy, bounded parser fuzzing, and Rust dependency advisories. Expand validation
with targeted sandbox integration, desktop session, performance regression, and
application compatibility tests as those components are implemented.

Milestones are complete only when their acceptance criteria and supporting evidence
are satisfied. Do not claim universal Windows compatibility, anti-cheat support,
perfect account protection, zero vulnerabilities, or performance gains without
reproducible evidence.
