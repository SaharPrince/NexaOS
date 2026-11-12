# NCE-002k: Independent LLVM/LLD PE32+ corpus sample

## Purpose

Extend image-layout compatibility evidence beyond the existing MSVC and MinGW-w64 outputs with a separately produced LLVM/LLD PE32+ AMD64 executable.

## Fixture

- Source: `engine/pe-inspector/tests/fixtures/llvm_layout.c`
- Compiler: Clang targeting `x86_64-pc-windows-msvc`
- Linker: LLVM LLD
- Generated executable: CI-only artifact; it is not committed or executed.
- The fixture includes initialized data, virtual-only zero-initialized storage, and a one-byte custom section intended to expose file-alignment padding.

The integration test reads the generated file, inspects it, plans its PE32+ AMD64 image layout, and independently checks per-section copy lengths and mapped extents. It also verifies the inspector reports `NOT_ATTEMPTED`.

## Scope and limitations

This adds one independent compiler/linker sample, not an exhaustive LLVM version matrix or broad third-party application corpus. It does not expand supported image architectures, execute the fixture, apply relocations, resolve imports, or create executable memory.

## Validation status

CI validation is pending for the commit that introduces the LLVM/LLD fixture and job. Do not treat the sample as accepted until the external compiler job passes and its generated PE is confirmed to exercise all three intended section cases.
