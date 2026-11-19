# NexaOS

NexaOS is an experimental Linux-based operating system project focused on running
Windows user applications and games through an independently developed
compatibility engine, without requiring a Windows virtual machine for normal use.

**Implementation status: early development.** Nexa currently has a Rust PE
inspection and non-executing image-preparation foundation. It is not yet a
general-purpose Windows application runtime or installable desktop OS. Compatibility,
performance, hardware support, and low-memory targets remain goals—not guarantees.

## Product goals

- **Familiar desktop:** make the desktop, file handling, settings, and everyday
  workflows feel natural to people switching from Windows.
- **Simple launching:** the intended experience is double-clicking a Windows .exe
  in the file manager, with no command-line setup and minimal avoidable launch delay.
- **Broad compatibility:** progressively support ordinary Windows applications,
  productivity software, and games; report compatibility by app/version rather than
  claiming every program works.
- **Lightweight operation:** keep idle CPU/RAM use, background services, storage
  footprint, and launch overhead low, especially on modest hardware.
- **Original engineering:** build Nexa's own compatibility components; do not
  misrepresent a repackaged Wine build as the Nexa engine.
- **Linux foundation:** use the Linux kernel and its hardware-driver ecosystem as
  the host foundation. Windows kernel drivers cannot be replaced by user-mode API
  compatibility.

## Anti-cheat and account-safety requirements

Anti-cheat compatibility is a high-risk, publisher-controlled area—not something
NexaOS may force by bypassing protections.

- Support anti-cheat only through legitimate, vendor- and publisher-approved
  compatibility paths.
- Never bypass, spoof, disable, or tamper with anti-cheat, protected processes,
  game memory, or security drivers.
- Fail closed when required anti-cheat support is absent, uncertain, or reports an
  integrity problem; do not repeatedly retry or silently force a launch.
- Clearly distinguish verified, unverified, and unsupported game configurations.
- Do not use a user's primary gaming account as a compatibility test account.
  Use vendor-approved test arrangements where available.
- Treat account protection as a design objective, not an absolute guarantee:
  third-party publishers control detection and enforcement, so no OS can promise
  that an account will never be flagged or restricted.

## Current engineering status

The repository contains the NCE PE inspection, bounded image-layout planning,
non-executable image population, explicit-catalogue import modelling, relocation
planning/application, and transactional image-preparation foundations. These
produce data only: they do not launch Windows applications, load Windows DLLs,
create executable mappings, or transfer control to PE entry points.

The active development branch is
nce-005c/low-end-resource-baseline. **NCE-005 remains open** while its
representative-corpus and compatibility/resource evidence gates are addressed.
See [NCE-005](docs/milestones/NCE-005.md) for exact scope, limitations, and evidence.
The [NCE-006 desktop and application-entry design](docs/milestones/NCE-006.md)
records the intended Windows-familiar desktop and safe double-click launch workflow;
it is a design baseline, not an implemented launcher.

## Build and inspect

Requires a stable Rust toolchain.

```sh
cargo test --workspace
cargo run -p nexa-inspect -- inspect path/to/program.exe
cargo run -p nexa-inspect -- inspect --json path/to/program.exe
```

The inspector reads the selected file as data only. **Execution: NOT ATTEMPTED.**

## Development priorities

To reach a useful user experience without spending time on low-value work, prioritize
in this order:

1. Close the remaining NCE-005 evidence gates with a small, provenance- and
   license-reviewed sample set. Do not expand benchmarks or corpus work without a
   decision they can inform.
2. Build the NCE-006 desktop shell and file-manager integration against a mock
   launcher, while keeping real PE execution disabled until its runtime security
   milestone is reviewed.
3. Design and implement the smallest safe end-to-end Windows user-mode application
   compatibility slice, with explicit boundaries, failure handling, and measurable
   launch/resource costs.
4. Add the common Win32, process, memory, file, thread, synchronization, and DLL
   behaviors required by real applications; expand from purpose-built test programs
   to appropriately licensed applications.
5. Expand graphics, input, audio, and game compatibility. Treat kernel anti-cheat
   support as a separate vendor-approval gate.
6. Engineer installation, updates, recovery, hardware support, and release-quality
   distribution.

Use risk-based validation: spend test effort on memory safety, bounds, privilege
boundaries, data integrity, failure recovery, and regressions that could affect a
user's machine. Avoid duplicative benchmarks and low-value repeated checks. Never
trade away system safety to make a compatibility test pass.

## Continuous validation

GitHub Actions checks Rust formatting, workspace tests on Linux and Windows,
Clippy, bounded parser fuzzing, and Rust dependency advisories. Keep required
correctness and security checks; use targeted validation for documentation-only
or isolated changes when full CI adds no meaningful protection.

Milestones are complete only when their stated acceptance criteria and evidence
are satisfied. No universal Windows compatibility, anti-cheat safety, or
performance claim should be made without supporting evidence.
