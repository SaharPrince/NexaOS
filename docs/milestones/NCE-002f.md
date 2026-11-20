# NCE-002f: Compiler-Produced PE Planner Validation

## Purpose

NCE-002f connects the image planner to a real PE32+ AMD64 executable produced by the Windows CI toolchain. This closes a gap between synthetic planner metadata tests and parser-only compiler fixture inspection.

## Test

On Windows, `engine/pe-inspector/tests/compiler_pe_layout.rs` reads Cargo's compiler-produced `nexa-inspect.exe` integration-test fixture, parses it with the read-only inspector, and passes the resulting report and exact file length to `plan_image_layout`.

The test asserts the architecture and format, nonempty sections, header mapping, image size, and basic copy/zero-fill accounting. The test never launches the inspected executable; it only reads its bytes.

The test is Windows-gated because a Windows target produces the PE fixture. Linux continues to run the portable synthetic compatibility suite.

## Limits

This is one real compiler-produced fixture, not a broad third-party corpus. The CI build profile and Rust toolchain are not representative of every compiler, linker, packing mode, subsystem, or application. It does not validate relocations, imports, executable memory, or program behavior.

## Validation status

The test passed on Windows CI for commit `29eb19b5ca7d2ebefc68b5a28984d6b25d361034`. The test only reads and parses the compiler-produced executable; it does not execute it. The current Windows run establishes acceptance of this specific toolchain-produced PE32+ AMD64 sample by both the inspector and pure image planner.

The local Windows workspace validation on the same commit also passed: 79 library tests, 6 CLI tests, 1 compiler-produced PE integration test, and 5 synthetic image-layout compatibility tests (91 total).

Further corpus work should add independently generated, redistributable fixtures with recorded compiler/linker versions and licensing. Do not add proprietary binaries or samples with unclear redistribution rights.
