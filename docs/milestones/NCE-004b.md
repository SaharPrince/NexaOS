# NCE-004b: Bounded Image Construction Pipeline

## Status

**Implementation in progress.** The first orchestration API and focused success/failure-path coverage are present on the feature branch. The milestone still requires final validation and review of remaining contracts. It composes the already implemented NCE-002 image-layout, allocation, population, NCE-003 symbolic import-resolution, and NCE-004a relocation primitives into one bounded, failure-atomic preparation pipeline.

NCE-004b does not enable application loading or execution. Its output is a prepared, non-executable image plus validated symbolic import information. It is not an executable process image and must not be described as launch-ready.

## Goal

Provide one narrow orchestration boundary that takes a PE32+ AMD64 input file, constructs its mapped image using existing validated stages, resolves imports only against an explicitly supplied export catalogue, applies supported base relocations to the owned image buffer, and returns a complete preparation result only if every requested stage succeeds.

The pipeline must avoid duplicate full-image allocations, keep all resource limits explicit, and ensure no partially prepared image escapes after an error.

## Existing primitives to compose

Use the existing APIs; do not reimplement their algorithms:

1. inspect(&[u8]) — parse and validate the exact input bytes.
2. plan_image_layout(&PeReport, file_len, ImageLayoutLimits) — produce bounded image copy/zero-fill ranges.
3. allocate_image_buffer(&ImageLayoutPlan, max_image_size, &mut impl ImageAllocator) — allocate zero-initialized, non-executable storage.
4. populate_image_buffer(&ImageLayoutPlan, input, ImageBuffer) — validate and populate the owned buffer.
5. resolve_imports(&[ParsedImportLibrary], &ExportCatalogue, ImportResolutionLimits) — resolve symbols using only caller-supplied exports.
6. validate_iat_patch_plan(&ImportResolutionPlan, image_size, IatPatchPlanLimits) — validate symbolic IAT destinations without writing them.
7. plan_base_relocations(...) followed by apply_base_relocations(&mut ImageBuffer, &RelocationPlan) — plan and atomically apply supported image-local DIR64 relocations.

The pipeline must use one PeReport produced from the same immutable input byte slice used for population. Do not accept an unrelated caller-constructed report as authoritative input. The caller-supplied export catalogue remains a separate, explicit dependency and must be validated by its existing constructor.

## Pipeline contract

### Inputs

The public request must make these values explicit:

- Immutable input file bytes, borrowed for the duration of preparation.
- Requested image base. The preferred image base comes from the parsed PE optional header.
- Caller-supplied ImageLayoutLimits, maximum image allocation size, and allocator.
- Caller-supplied ExportCatalogue and ImportResolutionLimits.
- Caller-supplied IatPatchPlanLimits and RelocationPlanLimits.

No implicit filesystem search, environment lookup, DLL discovery, host process inspection, or network access is permitted.

### Ordered stages

1. **Inspect:** parse the input bytes once and retain the resulting report internally for this operation.
2. **Plan layout:** validate PE32+ AMD64 identity and derive the image layout under caller limits.
3. **Resolve imports:** resolve all imports against the explicit export catalogue before image allocation. Missing modules/symbols, malformed forwarders, cycles, and hop-limit failures abort preparation.
4. **Validate imports:** validate symbolic IAT destinations against the planned image size before image allocation.
5. **Allocate:** allocate exactly one zero-initialized, non-executable image buffer, bounded by the explicit maximum.
6. **Populate:** copy validated headers/section data and zero-fill ranges using the same input slice. A population failure consumes and drops the intermediate buffer.
7. **Plan relocation:** use the report's typed relocation metadata and populated image bytes. The requested base and preferred base must be explicit. A nonzero base delta with no usable supported relocations is an error.
8. **Apply relocation:** apply the complete relocation plan through apply_base_relocations; do not add a second mutation path.
9. **Return:** expose a prepared result only after every requested stage succeeds. The result owns the single mapped image buffer and the validated symbolic import plan. Any error drops all intermediate state.

The order is deliberate: import resolution and IAT validation depend only on parsed metadata, the caller-owned catalogue, and the planned image size. Performing them before allocation avoids consuming image memory for an image whose imports cannot be resolved or whose IAT plan is invalid. The IAT plan remains symbolic and is not written. Relocation is applied only to the populated image buffer after all non-mutating import validation succeeds.

## Output contract

A successful result may expose:

- A read-only view of the populated, relocated image bytes.
- The image size, preferred base, and requested base as validated metadata.
- The validated symbolic IatPatchPlan, whose targets remain module names and RVAs.
- Minimal diagnostic metadata needed by callers.

The result must not expose mutable image bytes, raw pointers, executable-memory handles, process addresses, or a generic write interface. Avoid retaining the original input bytes or duplicating the complete image. Do not retain a second full image snapshot for rollback; transactionality is achieved by keeping the intermediate buffer private and returning it only on total success.

## Failure and resource guarantees

- Every stage returns a structured error identifying the failing stage and preserving the underlying error category.
- No partially prepared result is returned on any error.
- Image allocation is bounded by the caller's maximum image size; section, import, export, relocation, and patch counts remain bounded by existing caller-supplied limits.
- Do not introduce unbounded retries, background work, global caches, or per-image threads.
- Avoid cloning the complete image. Any additional vectors must be sized from bounded metadata and use fallible reservation where they allocate.
- The input is borrowed, not copied wholesale. The output owns one image buffer.
- The allocator remains injectable for deterministic allocation-failure tests.
- The pipeline must not trust public plan structures without the existing independent validators.

## Import and relocation semantics

The import resolver and IAT validator produce symbolic results only. They do not calculate loaded target-module addresses and do not patch IAT slots. This milestone must preserve that boundary.

Relocation changes only the validated DIR64 targets in the owned image buffer. The current supported types remain ABSOLUTE (no-op) and DIR64 (eight-byte patch) for PE32+ AMD64. Unsupported relocation types fail closed.

If the caller supplies an export catalogue that does not contain a required module or symbol, preparation fails. The pipeline must not silently omit imports or search the host for a DLL.

## Security and compatibility boundaries

This stage must not:

- Execute the input or mapped image, call its entry point, or create a thread.
- Load or map Windows DLLs, discover modules from the host, or perform filesystem search.
- Write IAT values or convert symbolic RVAs into process virtual addresses.
- Allocate executable or writable-plus-executable memory or change page protections.
- Write to another process, kernel memory, or arbitrary host addresses.
- Invoke Windows APIs or run constructors/TLS callbacks.
- Claim compatibility with arbitrary Windows applications.

The image remains ordinary non-executable process-heap data. A successful result means only that the supported preparation stages completed.

## Required tests

### Success path

1. Builds an image from a valid PE32+ AMD64 fixture using the exact input bytes.
2. Confirms headers, copied section bytes, and zero-fill bytes match the existing population contract.
3. Resolves imports from a caller-supplied export catalogue and returns a validated symbolic IAT plan.
4. Applies a valid DIR64 relocation when requested base differs from preferred base.
5. Produces an empty relocation plan when the bases are equal and relocation metadata is absent.
6. Handles an image with no imports when the import table is genuinely empty.
7. Confirms output image bytes are exposed read-only and the result contains no process addresses.

### Failure atomicity

8. Rejects malformed PE input before allocation.
9. Rejects invalid layout, image-size budget, and allocator failure.
10. Rejects population source/destination range failures without returning an image.
11. Rejects missing import modules/symbols, malformed forwarders, cycles, and hop-limit failures.
12. Rejects forged/invalid IAT plans, out-of-bounds slots, and overlapping destinations.
13. Rejects missing relocation data for a required rebase, unsupported relocation types, invalid targets, and arithmetic overflow.
14. Confirms no error path returns a prepared result or exposes intermediate image bytes.

### Resource behavior

15. Uses an allocator spy to prove at most one image-sized allocation occurs in the pipeline.
16. Exercises configured low image-size, section, import, export, and relocation limits.
17. Confirms allocation failure is surfaced without panic or partial result.
18. Keeps all existing workspace, fuzz, Windows, and compiler-fixture tests passing.

## Exit criteria

- The orchestration API composes the existing primitives without duplicating their validation logic.
- The exact same input slice is used for parsing and population.
- One bounded image buffer is allocated; no full-image clones or rollback snapshots are introduced.
- Import resolution remains explicit-catalogue-only and IAT results remain symbolic.
- Relocations are applied only through the existing all-or-nothing image-local API.
- Failure paths never return a partial image.
- Linux and Windows CI, formatting, Clippy, workspace tests, fuzz smoke, and dependency audit pass.
- Documentation and API naming clearly state that the result is prepared data, not an executable or launch-ready image.

## Deferred work

IAT value application, loaded-module base assignment, DLL loading, page permissions, TLS handling, entry-point execution, process creation, and application startup each require separate design and security review. NCE-002's broader third-party PE corpus and representative low-end hardware measurements remain mandatory release gates before real application loading is considered.


## Implementation progress — NCE-004b.1

The first orchestration increment adds `prepare_image` and the `ImagePreparationRequest` / `PreparedImage` API in `engine/pe-inspector/src/image_preparation.rs`.

- Inspects the borrowed input once and uses that same slice for image population.
- Plans layout, resolves imports only against the caller-supplied `ExportCatalogue`, and validates symbolic IAT destinations before allocating image memory.
- Allocates one bounded non-executable image buffer and consumes it during population.
- Plans and applies relocation through the existing pure planner and all-or-nothing image-local API.
- Returns the image only after every stage succeeds; the image bytes are exposed read-only and the IAT plan remains symbolic.
- Reports structured stage identity and the underlying error text.
- Tests cover import-bearing success without IAT writes, successful DIR64 rebasing, unresolved imports rejected before allocation, invalid IAT plans rejected before allocation, image-budget rejection before allocator invocation, malformed input rejected before allocation, allocator failure, and missing relocation data on rebase.
- The focused preparation tests also exercise layout-limit rejection before allocation and unsupported relocation rejection after allocation, without returning a prepared image.

This is an initial implementation increment, not milestone completion. The population-failure path and broader final contract review remain outstanding; additional tests should be added only where a failure can be induced meaningfully through the public API. No IAT writes, DLL loading, executable memory, or image execution are introduced.


## Implementation progress — NCE-004b.2

The preparation error contract now retains the original typed subsystem error instead of flattening it into a formatted string.

- Added public `ImagePreparationCause` variants for PE inspection, layout, import resolution, IAT validation, allocation/population, relocation planning, and relocation application failures.
- `ImagePreparationError` retains both the orchestration stage and typed cause; its standard error source chain exposes the cause and underlying subsystem error.
- PE inspection errors now implement `Clone`, `PartialEq`, and `Eq` so the complete preparation error remains inspectable and comparable.
- Focused tests assert concrete cause variants for inspection, import resolution, IAT validation, allocation, and relocation planning, and verify the standard error source chain.
- This closes the typed-diagnostics gap; it does not change image preparation, memory allocation, import resolution, relocation, or execution behavior.

## Implementation progress — NCE-004b.3

The image-population validator now reserves its bounded temporary range-tracking storage fallibly before validation begins.

- The maximum number of tracked ranges is derived with checked arithmetic from the section count: one header range plus at most two ranges per section.
- Both destination and source range vectors use `try_reserve_exact`; allocation failure is returned as `ImageAllocationError::AllocationFailed` instead of relying on infallible growth during `push`.
- Existing population tests exercise malformed source ranges, overlapping destinations, and inconsistent plan accounting; these failures occur before image bytes are copied and the consumed intermediate buffer is not returned.
- A malformed population plan cannot be injected through `prepare_image`: that API creates its layout internally from the inspected input and checks the allocator's output length before population. The primitive-level tests therefore cover malformed-plan rejection without adding a public test hook or weakening the preparation API.
- This is a bounded-allocation hardening change; it does not alter image contents, import resolution, relocation, or execution behavior.
