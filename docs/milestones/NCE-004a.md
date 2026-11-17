# NCE-004a: Bounded AMD64 Base Relocations

## Status

**NCE-004a.1 implemented and validated.** The parser now retains bounded typed relocation metadata. Pure patch planning and all-or-nothing application remain unimplemented. This is a loader prerequisite between the accepted NCE-002 image foundation and any future application-startup/runtime work. The existing high-level NCE-004 Windows x64 ABI/runtime work area is not renumbered by this document.

The parser now validates relocation-directory block framing and retains bounded typed relocation entries. The existing counters remain for compatibility; relocation planning consumes the typed metadata and independently validates its inputs.

## Goal

For PE32+ AMD64 images only, produce and apply a bounded, image-local base-relocation patch plan when an image is mapped at a base different from its preferred image base.

Relocation is a data transformation only. It must not load DLLs, resolve imports, execute image code, change page protections, write another process's memory, or create executable memory.

## Scope and staged implementation

### NCE-004a.1 — typed relocation metadata

Extend `PeReport` with bounded structured records while preserving the existing `relocation_blocks` and `relocation_entries` counters for compatibility.

Retain:
- Each block's 4-KiB-aligned page RVA.
- Each encoded 16-bit relocation entry, including its type and 12-bit page offset.
- Stable block/entry ordering and enough indices to report the exact malformed entry.
- The relocation directory's declared RVA and size, or equivalent validated directory metadata.

Parser requirements:
- Validate the complete directory range before iterating.
- Require each block header to fit, `SizeOfBlock >= 8`, block size to be even, and each block to remain inside the directory.
- Require the directory to be consumed exactly; reject truncated trailing bytes and malformed blocks.
- Apply explicit configurable or fixed conservative ceilings to total blocks and entries; no unbounded allocation or iteration.
- Use checked arithmetic for block offsets and page-RVA calculations.
- Preserve unknown relocation type values as metadata; the planner, not the parser, decides whether a type is supported.
- Continue to inspect files as data only.

### NCE-004a.2 — pure relocation patch planning

Add a pure planning API for a populated, non-executable PE32+ AMD64 image.

Inputs must make the preferred image base, requested load base, image size, and validated relocation metadata explicit. Reject unsupported machine/format combinations.

Initial supported relocation types:
- `IMAGE_REL_BASED_ABSOLUTE (0)`: padding/no-op; it creates no patch.
- `IMAGE_REL_BASED_DIR64 (10)`: one 64-bit relocation target.

Reject every other type explicitly, including `HIGHLOW (3)` in this AMD64-only stage. Do not silently ignore unknown types.

For each DIR64 entry:
- Calculate target RVA as block page RVA plus the 12-bit entry offset using checked arithmetic.
- Validate the entire eight-byte target range against the image before reading or planning a patch.
- Read the original little-endian 64-bit value from the mapped image.
- Apply the base delta using checked addition or subtraction; reject arithmetic overflow/underflow.
- Record target RVA, original value, and relocated value in the plan.
- Reject duplicate or overlapping relocation targets.

If preferred and requested bases are equal, produce an empty patch plan without requiring relocation entries. If the bases differ and the image has no usable relocation directory, return an explicit error; do not guess or silently leave absolute addresses unchanged.

Bound total relocation blocks, entries, and patches. Keep allocations proportional to the bounded relocation metadata, not to the full image size.

### NCE-004a.3 — all-or-nothing image-local application

Apply a validated relocation plan through a narrowly scoped mutable `ImageBuffer` API only.

Before the first write:
1. Validate every patch target against the current image length.
2. Validate every target is eight bytes and targets remain unique/non-overlapping.
3. Verify the current bytes still equal each patch's recorded original value, preventing stale plans from overwriting intervening changes.
4. Complete all fallible validation and required allocation before mutation.

Only after all checks succeed may the implementation write the precomputed 64-bit values. The write loop must have no fallible operations. Any error must leave the image byte-for-byte unchanged.

Do not expose raw pointers, arbitrary address writes, or a generic process-memory write API.

## Error model

Use structured errors that identify the block/entry or patch index where practical. At minimum distinguish:
- Unsupported image format or machine.
- Missing relocation data when a nonzero base delta is required.
- Too many blocks, entries, or patches.
- Truncated or malformed relocation block.
- Unsupported relocation type.
- Target RVA arithmetic overflow.
- Target range outside the image.
- Duplicate or overlapping targets.
- Base-delta arithmetic overflow/underflow.
- Stale patch plan / original bytes changed.
- Allocation failure.

Malformed input and forged public plans must return errors, never panic.

## Required tests

### Parser
1. Parses ABSOLUTE and DIR64 entry encodings without losing raw type/offset.
2. Preserves unknown relocation types for planner rejection.
3. Rejects incomplete headers, undersized blocks, odd block sizes, blocks extending beyond the directory, and trailing incomplete bytes.
4. Rejects unaligned page RVAs and checked-arithmetic overflow.
5. Enforces block/entry limits and preserves deterministic ordering.

### Planner
6. Produces no patches when preferred and requested bases are equal.
7. Produces the expected DIR64 result for positive and negative base deltas.
8. Treats ABSOLUTE entries as no-ops.
9. Rejects unsupported types, including HIGHLOW on AMD64.
10. Rejects missing relocation data when rebasing is required.
11. Rejects target-range overflow and targets that extend beyond the image.
12. Rejects duplicate/overlapping targets and base-delta arithmetic overflow.
13. Rejects excessive patch counts and malformed/forged metadata.

### Application
14. Applies a valid multi-patch plan correctly.
15. Rejects a stale plan if any original target bytes changed.
16. Rejects any out-of-bounds, duplicate, or overlapping forged patch before writing.
17. Proves byte-for-byte that every rejected plan leaves the image unchanged.
18. Existing parser, image-layout/population, import-resolution, CLI, fuzz, and workspace tests remain green.

## Non-goals and safety boundaries

- No PE32 x86, ARM64, ARM, or other machine support in this increment.
- No HIGH, LOW, HIGHLOW, HIGHADJ, or architecture-specific relocation support.
- No DLL discovery/loading, import resolution, IAT application, or forwarder resolution.
- No entry-point execution, thread creation, executable page permissions, or writable-plus-executable memory.
- No arbitrary host-process memory writes or kernel-mode code.
- No claim that a relocated image is ready to execute or compatible with Windows applications.

## Exit criteria

- Typed relocation metadata is bounded, lossless for encoded entries, and fuzz-tested.
- The planner supports only ABSOLUTE and DIR64 for PE32+ AMD64 and rejects all unsupported types.
- All targets and arithmetic are validated before any mutation.
- Application is image-local and all-or-nothing, including stale-plan rejection.
- Linux and Windows CI, formatting, Clippy, unit tests, and bounded fuzz smoke pass.
- Documentation continues to state that relocation does not enable image execution.

## Release gates retained from NCE-002

The broader third-party PE corpus and representative low-end hardware measurements remain mandatory before enabling real application loading. Passing this isolated relocation milestone does not waive those gates or establish production readiness.


## Implementation progress — NCE-004a.1

The parser now exposes `ParsedRelocationDirectory`, `ParsedRelocationBlock`, and `ParsedRelocationEntry` through `PeReport.relocation_metadata`. Each entry retains its original 16-bit value plus the decoded four-bit type and twelve-bit offset. Unknown type values are preserved without parser-side interpretation. The declared directory RVA and size, page RVAs, block order, and entry order are retained.

The existing `relocation_blocks` and `relocation_entries` counters remain available. Parsing enforces fixed ceilings of 4,096 blocks and 65,536 entries, validates block framing and 4-KiB page alignment, uses checked count/offset arithmetic, and rejects incomplete trailing directory bytes. Absent directories produce `None` metadata and zero counters.

Focused tests cover lossless DIR64/unknown-type decoding, missing metadata, incomplete trailing bytes, and both metadata ceilings. Local Windows validation passed formatting, Clippy, and all 144 workspace tests. GitHub Actions on commit `6672374968b60102558a4a13adba652ae0d59e76` passed formatting, Clippy, Ubuntu and Windows workspace tests, external compiler fixture validation, dependency audit, and fuzz smoke. No relocation patch planning or image mutation has been added yet.
