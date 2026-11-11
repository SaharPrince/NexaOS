# NexaOS

NexaOS is an experimental Linux-based operating system project focused on running
Windows user applications and games through an independently developed user-mode
compatibility engine, without requiring a Windows virtual machine for normal use.

**Implementation status: early development.** Compatibility, performance, hardware
support, and low-memory targets are goals—not achieved guarantees.

## Project principles

- Linux remains the host operating-system foundation.
- Build an original compatibility engine; do not present a repackaged Wine build as
  the Nexa engine.
- Start with x86-64 Windows PE executables and grow API coverage incrementally.
- Never claim that every Windows application, driver, or game will work.
- Windows kernel drivers and publisher-controlled kernel anti-cheat require
  appropriate native support; user-mode API compatibility cannot solve those cases.
- Security, evidence-based compatibility reporting, and low-resource measurements
  are first-class requirements.
- Keep optional runtime components demand-driven; avoid background services and
  unbounded caches that consume resources when no Windows application is running.

## Current milestone: NCE-001 — Nexa PE Inspector

A read-only Rust tool that identifies and inspects Windows Portable Executable
(PE) files. It does not execute files or load DLLs.

Initial scope:
- DOS, COFF, and PE optional headers
- PE32 / PE32+ and machine architecture identification
- Section table and data-directory inspection
- Import and export metadata
- Base-relocation directory summary
- Human-readable and JSON output
- Bounds-checked parsing and malformed-input tests

## Next milestone design: NCE-002 — PE Image Layout

The [NCE-002 design specification](docs/milestones/NCE-002.md) defines the staged,
bounded design for laying out PE32+ AMD64 images in non-executable memory. It is a
design document, not an implemented loader. The first implementation stages will
plan and verify image layouts before any memory population; no Windows code will
be executed by NCE-002.

## Build

Requires a stable Rust toolchain.

```sh
cargo test --workspace
cargo run -p nexa-inspect -- inspect path/to/program.exe
cargo run -p nexa-inspect -- inspect --json path/to/program.exe
```

The inspector only reads the selected file. **Execution: NOT ATTEMPTED.**

## Continuous validation

GitHub Actions automatically checks changes on the development branch and pull
requests targeting `main`:

- Rust formatting, including the isolated fuzz target
- Workspace tests and CLI startup on Linux and Windows
- Clippy with warnings denied
- A bounded PE parser fuzz run
- Rust dependency advisory audits for both the main workspace and fuzz harness

Dependency audits also run weekly. Dependabot proposes weekly updates for Cargo
and GitHub Actions dependencies; proposed updates must pass the same checks
before they are accepted.

## Roadmap

- [ ] NCE-001: PE inspector
- [ ] NCE-002: PE image layout and memory-loader design
- [ ] NCE-003: Import/export resolution model
- [ ] NCE-004: Windows x64 ABI bridge
- [ ] NCE-005: First purpose-built Windows test executable
- [ ] NCE-006: Initial GUI compatibility surface
- [ ] NCE-007: Broader user-mode application compatibility
- [ ] NCE-008: Graphics, input, audio, and publisher-supported game compatibility
- [ ] NCE-009: NexaOS distribution engineering

Milestones are not complete until their acceptance tests and documentation pass.
