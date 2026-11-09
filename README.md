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

## Build

Requires a stable Rust toolchain.

```sh
cargo test --workspace
cargo run -p nexa-inspect -- inspect path/to/program.exe
cargo run -p nexa-inspect -- inspect --json path/to/program.exe
```

The inspector only reads the selected file. **Execution: NOT ATTEMPTED.**

## Roadmap

- [ ] NCE-001: PE inspector
- [ ] NCE-002: PE image layout and memory-loader design
- [ ] NCE-003: Import/export resolution model
- [ ] NCE-004: Windows x64 ABI bridge
- [ ] NCE-005: First purpose-built Windows test program
- [ ] NCE-006: Initial GUI compatibility surface
- [ ] NCE-007: Broader user-mode application compatibility
- [ ] NCE-008: Graphics, input, audio, and publisher-supported game compatibility
- [ ] NCE-009: NexaOS distribution engineering

Milestones are not complete until their acceptance tests and documentation pass.
