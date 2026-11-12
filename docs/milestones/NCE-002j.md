# NCE-002j: Low-section-alignment file mapping

## Purpose

Validate the PE image-layout rule for images whose `SectionAlignment` is smaller than the normal page size.

## Rule

For low-alignment images, the PE format requires section data to be laid out at matching file and image offsets. The planner therefore rejects a section with nonempty raw data when `PointerToRawData` differs from `VirtualAddress`.

The existing low-alignment validation remains in force:

- `SectionAlignment` must be a valid power of two.
- `FileAlignment` must equal `SectionAlignment` when `SectionAlignment` is below 4 KiB.
- Section image ranges must remain within `SizeOfImage`, must not overlap headers or other sections, and raw file ranges must remain within the input and not overlap.
- The low-alignment section fixture uses a valid entry-point RVA.

## Tests

The synthetic compatibility integration tests cover:

- Accepting a valid low-alignment section when file offset equals RVA and file alignment matches section alignment.
- Rejecting a low-alignment section when raw file offset differs from its RVA.
- Rejecting low alignment when file and section alignment differ.

These tests exercise metadata planning only. They do not allocate executable memory, apply relocations, resolve imports, or execute the input.

## Validation

The NCE-002j change is included in commit `eb51ef9`. Its GitHub Actions validation passed:

- Rust formatting and fuzz-target formatting
- Clippy with warnings denied
- Workspace tests on Ubuntu and Windows
- Independently compiled MinGW-w64 PE32+ AMD64 planning and PE32 x86 inspection
- Bounded PE parser fuzz smoke test
- Dependency security audit for workspace and fuzz dependencies

## Reference

- Microsoft, [PE Format](https://learn.microsoft.com/en-us/windows/win32/debug/pe-format).

This milestone adds a narrowly scoped layout constraint; it does not claim broad third-party PE compatibility.
