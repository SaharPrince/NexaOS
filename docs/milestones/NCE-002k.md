# NCE-002k: Independent LLVM/LLD PE32+ corpus sample

## Purpose

Extend image-layout compatibility evidence beyond the existing MSVC and MinGW-w64 outputs with a separately produced LLVM/LLD PE32+ AMD64 executable.

## Fixture

- Source: `engine/pe-inspector/tests/fixtures/llvm_layout.c`
- Compiler: Clang 21.1.8 targeting `x86_64-pc-windows-msvc`
- Linker: LLVM LLD 21.1.8
- Generated executable: CI-only artifact; it is not committed or executed.
- The fixture includes initialized data, virtual-only zero-initialized storage, and a one-byte custom section that exposes file-alignment padding.

The integration test reads the generated file, inspects it, plans its PE32+ AMD64 image layout, and independently checks per-section copy lengths and mapped extents. It also verifies the inspector reports `NOT_ATTEMPTED`.

## Validation

The external compiler CI job passed on commit `9129f79`:

- Generated PE32+ AMD64 image: 6,144 bytes, 3 sections.
- Planned copy bytes: 5,168.
- Planned zero-fill bytes: 16,384.
- Initialized data, the custom raw-padding section, and virtual-only storage were all exercised.
- The executable was inspected and planned only; it was never executed.

## Scope and limitations

This adds one independent compiler/linker sample, not an exhaustive LLVM version matrix or broad third-party application corpus. It does not expand supported image architectures, execute the fixture, apply relocations, resolve imports, or create executable memory.
