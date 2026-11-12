# NCE-002g: Independent Compiler PE Compatibility Sample

## Purpose

NCE-002g broadens image-layout validation beyond the Rust/MSVC-produced executable used in NCE-002f. It adds a separately generated PE32+ AMD64 sample from the MinGW-w64 GCC toolchain.

## Fixture provenance

The repository contains only the small C source at `engine/pe-inspector/tests/fixtures/mingw_layout.c`. GitHub Actions installs the Ubuntu MinGW-w64 x86-64 cross-compiler package, records the compiler version in the job log, and builds the PE fixture during CI. The generated executable is a temporary CI artifact and is not committed or redistributed.

The fixture deliberately contains initialized global data and zero-initialized storage so the resulting image exercises both file-backed section copies and virtual-only zero-fill planning. MinGW-w64 GCC 13 emits an import-directory Size of 0x4f0 (not divisible by the 20-byte IMAGE_IMPORT_DESCRIPTOR size); GNU objdump identifies the directory as covering parts of .idata. The descriptor array is null-terminated before the remainder of that bounded import-data region.

## Validation

The `external-pe-corpus` CI job:
1. Builds the PE32+ AMD64 executable with `x86_64-w64-mingw32-gcc`.
2. Parses the generated file using the read-only PE inspector.
3. Verifies that initialized and virtual-only section storage are present.
4. Runs the pure image-layout planner and checks copy/zero-fill accounting.

The initial MinGW run exposed an overly strict parser assumption that the import-directory Size must be divisible by 20. Microsoft's PE format describes the import directory as a null-terminated array of 20-byte descriptors, but does not specify that the directory Size must end exactly on a descriptor boundary. The parser now treats Size as the bounded mapped range and walks descriptors only until the all-zero terminator. A regression test covers a directory Size that includes additional import data beyond the descriptor array. <https://learn.microsoft.com/en-us/windows/win32/debug/pe-format>

The fixture is inspected as data only. It is never launched or executed.

The Windows MSVC CI job separately builds and inspects PE32 x86 and PE32+ AMD64 Rust compiler outputs. The PE32 x86 sample is parser validation only; the current image planner intentionally remains PE32+ AMD64-only.

## Compatibility limits

This is an additional independently generated sample, not a representative application corpus. It does not establish compatibility with every MinGW version, linker option, PE subsystem, packer, or third-party program. It does not test relocations, import resolution, executable memory, or runtime behavior.

Generated binaries are intentionally excluded from source control. The checked-in C fixture source is original project test material; the compiler output exists only in CI.

## Acceptance criteria

- MinGW-generated PE32+ AMD64 is accepted by the inspector.
- Initialized file-backed sections are planned as copy ranges.
- Virtual-only section storage is planned as zero-fill.
- The planner's accounting remains bounded and internally consistent.
- No inspected fixture is executed.
