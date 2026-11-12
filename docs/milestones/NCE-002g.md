# NCE-002g: Independent Compiler PE Compatibility Samples

## Purpose

NCE-002g broadens validation beyond the Rust/MSVC-produced executable used in NCE-002f. It adds independently generated PE32+ AMD64 and PE32 x86 samples from the MinGW-w64 GCC toolchains.

## Fixture provenance

The repository contains only the small C source at `engine/pe-inspector/tests/fixtures/mingw_layout.c`. GitHub Actions installs the Ubuntu MinGW-w64 x86-64 and i686 cross-compiler packages, records both compiler versions in the job log, and builds the PE fixtures during CI. Generated executables are temporary CI artifacts and are not committed or redistributed.

The fixture deliberately contains initialized global data, zero-initialized storage, and a one-byte variable placed in a dedicated section. That small section forces the linker to round `SizeOfRawData` up to `FileAlignment` while keeping a smaller nonzero `VirtualSize`. The PE32+ AMD64 sample therefore exercises initialized copies, virtual-only zero-fill, and trailing raw file-alignment padding. The PE32 x86 sample broadens parser coverage across the 32-bit optional-header format and x86 machine type; it is inspection-only because the current image planner intentionally supports PE32+ AMD64 only.

MinGW-w64 GCC 13 emits an import-directory Size of 0x4f0 for the x86-64 fixture (not divisible by the 20-byte IMAGE_IMPORT_DESCRIPTOR size); GNU objdump identifies the directory as covering parts of .idata. The descriptor array is null-terminated before the remainder of that bounded import-data region.

## Validation

The `external-pe-corpus` CI job:
1. Builds a PE32+ AMD64 executable with `x86_64-w64-mingw32-gcc`.
2. Builds a PE32 x86 executable with `i686-w64-mingw32-gcc`.
3. Parses both generated files using the read-only PE inspector.
4. Prints the compiler-produced section headers so `VirtualSize` and `SizeOfRawData` are visible in CI logs.
5. Verifies that the AMD64 sample contains initialized data, virtual-only storage, and a nonzero-`VirtualSize` section whose raw size includes file-alignment padding.
6. Runs the pure image-layout planner on the AMD64 sample and verifies that each nonzero virtual extent is mapped to `VirtualSize`, copy lengths are capped at `min(SizeOfRawData, VirtualSize)`, and virtual-only tails are zero-filled.
7. Verifies the x86 sample is PE32 with machine type x86; it does not pass it to the AMD64-only planner.

The initial MinGW x86-64 run exposed an overly strict parser assumption that the import-directory Size must be divisible by 20. Microsoft's PE format describes the import directory as a null-terminated array of 20-byte descriptors, but does not specify that the directory Size must end exactly on a descriptor boundary. The parser now treats Size as the bounded mapped range and walks descriptors only until the all-zero terminator. A regression test covers a directory Size that includes additional import data beyond the descriptor array. <https://learn.microsoft.com/en-us/windows/win32/debug/pe-format>

All fixtures are inspected as data only. They are never launched or executed.

The Windows MSVC CI job separately builds and inspects PE32 x86 and PE32+ AMD64 Rust compiler outputs. These are parser validation samples; the current image planner intentionally remains PE32+ AMD64-only.

## Compatibility limits

These are additional independently generated compiler samples, not a representative application corpus. They do not establish compatibility with every MinGW version, linker option, PE subsystem, packer, or third-party program. They do not test relocations, import resolution, executable memory, or runtime behavior.

Generated binaries are intentionally excluded from source control. The checked-in C fixture source is original project test material; compiler outputs exist only in CI.

## Acceptance criteria

- MinGW-generated PE32+ AMD64 is accepted by the inspector and planner.
- Initialized file-backed AMD64 sections are planned as copy ranges.
- File-alignment padding beyond a nonzero `VirtualSize` is not copied into the logical mapped image.
- Virtual-only AMD64 section storage is planned as zero-fill.
- MinGW-generated PE32 x86 is accepted by the inspector as PE32/x86.
- The planner is not used for the PE32 x86 sample.
- The planner's accounting remains bounded and internally consistent.
- No inspected fixture is executed.
