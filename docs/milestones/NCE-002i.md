# NCE-002i: Compiler-produced section accounting

## Purpose

Strengthen the Windows compiler-produced PE32+ integration test beyond basic parser and planner acceptance. The test reads the Cargo-built `nexa-inspect.exe` produced by the Windows MSVC toolchain and validates each nonempty section against the planner's public output.

## Assertions

For every section with a nonzero raw or virtual extent, the test independently derives the expected logical mapped size:

- Use nonzero `VirtualSize` as the mapped extent.
- Preserve the explicit raw-size fallback when `VirtualSize` is zero.
- Expect copied bytes to equal the raw size capped at the selected mapped extent.
- Expect zero-fill bytes to cover the remainder of the mapped extent.
- Verify copy destinations begin at the declared section RVA.
- Verify zero-fill begins immediately after the copied bytes.
- Sum per-section copy and zero-fill lengths and compare them with the plan's aggregate accounting.
- Assert the inspected executable remains marked `NOT_ATTEMPTED`.

The fixture is only read and inspected. It is never launched by this test.

## Validation status

The integration test passed in the Windows CI job on commit `eb51ef9`. The same revision also passed Linux tests, formatting, Clippy, the independent MinGW compiler fixtures, bounded fuzz smoke, and the workspace/fuzz dependency audit.

This remains compiler-produced sample coverage, not an exhaustive third-party PE corpus. The existing MinGW job continues to exercise independently compiled PE32+ AMD64 planning and PE32 x86 parsing.

## Safety and scope

No image execution, relocation, import resolution, executable memory, or host-process memory writes are introduced. The image planner remains PE32+ AMD64-only. No additional runtime dependency or background service is added.
