# NCE-002: Section Ordering and File-Only Data Review

**Review date:** 2026-11-16  
**Scope:** Independent specification and implementation comparison. This is a validation note, not a loader behavior change or an NCE-002 acceptance decision.

## Findings

### Microsoft PE/COFF format

Microsoft's PE Format documentation states that section-table entries are in linker-chosen order, and that for an image the section virtual addresses must be assigned in ascending order and adjacent, with each address aligned to `SectionAlignment`.

The same section-header documentation defines `VirtualSize` as the section's in-memory size. `SizeOfRawData` is file-aligned and can exceed a nonzero `VirtualSize`; a section containing only uninitialized data should have zero raw size. The Certificate Table data-directory entry is file-offset based and is not loaded into memory as part of the image.

Reference: https://learn.microsoft.com/en-us/windows/win32/debug/pe-format

### Independent implementation comparison: ReactOS

The ReactOS kernel's PE image-section code iterates the section headers in table order. It checks that each section begins at the previous aligned virtual end, and rounds each section's chosen extent up to `SectionAlignment`. It uses `VirtualSize` when nonzero and `SizeOfRawData` when `VirtualSize` is zero.

Reference: https://github.com/reactos/reactos/blob/master/ntoskrnl/mm/section.c

ReactOS's boot loader also documents the zero-`VirtualSize` fallback, suppresses raw copying when `PointerToRawData` is zero, and caps copied bytes at the virtual extent.

Reference: https://github.com/mirror/reactos/reactos/blob/master/reactos/boot/freeldr/freeldr/windows/peloader.c

These are useful independent implementation observations, **not proof of Windows loader behavior**. ReactOS is not Windows, and its kernel validation policy must not be presented as a Windows compatibility guarantee.

## Consequences for NexaOS

1. The current planner validates alignment and rejects overlapping section destinations, but it does not enforce ascending section-table RVA order or adjacency. That is less strict than the documented PE image-generation rule.
2. Any future adjacency check must compare the next section RVA against the previous section's **SectionAlignment-rounded extent**. Comparing directly against the current unrounded `mapped_size` would be incorrect for this rule.
3. The zero-`VirtualSize` fallback has corroboration in ReactOS implementation code, but this does not establish behavior across supported Windows versions. Keep it an explicit compatibility policy pending corpus evidence.
4. The certificate table must remain file-only and must not be copied into the mapped image. Debug-directory records need a separate field-by-field review because their directory RVA and file pointers describe different address spaces; do not classify every debug-related byte as unmapped.

## Decision

**Do not change planner acceptance rules in this review.** The specification gives a strong reason to test ordering and adjacency, but tightening validation before checking a broader set of independently produced images could reject files that existing Windows systems tolerate. The current planner's permissive behavior remains a known validation gap, not a claim that gaps or out-of-order sections are valid PE images.

## Low-cost corpus plan

Avoid downloading the full Assemblage archive. Its Windows PE corpus is a large, compiler-produced MSVC collection, and each binary retains the license of its originating source repository.

Dataset documentation: https://assemblagedocs.readthedocs.io/en/latest/dataset.html  
Dataset page: https://huggingface.co/datasets/changliu8541/Assemblage_PE

For any candidate sample selected later, record at minimum:

- Source project URL and exact source commit/version.
- Original project license and whether redistribution is permitted.
- SHA-256, file size, PE architecture, and compiler/linker version when known.
- Section count, section-table order, RVAs, virtual/raw sizes, alignment values, and overlay/certificate/debug metadata.
- Inspection/planning result and the exact test or tool version used.

Prefer a small, diverse, license-checked sample set over a bulk download. Keep third-party binaries out of the repository unless their licenses explicitly permit redistribution. All corpus checks remain read-only: inspect and plan files, but never execute the samples.

## Remaining validation questions

- Do real, redistributable PE32+ AMD64 samples exhibit gaps or out-of-order section headers that Windows accepts?
- How do compiler/linker versions vary in their use of zero-`VirtualSize`, raw-only, virtual-only, and empty sections?
- Which samples exercise section alignments around the 4 KiB boundary, certificate tables, debug records, and overlays?
- Can the selected samples be tested reproducibly in CI without committing binaries or relying on fragile external downloads?

NCE-002 remains under validation. NCE-003 remains blocked until the corpus review and representative low-end hardware measurements are complete.


## Dataset access triage — 2026-11-17

A second review of the published Assemblage Windows PE dataset clarifies the practical selection path:

- The dataset card describes approximately 91,000 Windows PE binaries, with metadata in `binaries.csv` and `functions.csv`, plus a separate binary archive.
- The Hugging Face listing reports approximately 43.1 GB total; the binary archive alone is listed at approximately 33.8 GB.
- The dataset documentation says each binary retains its originating source repository's license. The dataset's own hosting/license label must not be mistaken for blanket permission to redistribute every underlying binary.
- The Assemblage documentation describes per-binary metadata including source repository and license information; the Windows PE dataset card also identifies SHA-256 and `binary_path` as ways to identify files.

References:
- https://huggingface.co/datasets/changliu8541/Assemblage_PE
- https://huggingface.co/datasets/changliu8541/Assemblage_PE/tree/main
- https://assemblagedocs.readthedocs.io/en/latest/dataset.html

### Selection decision

Do **not** download the 33.8 GB binary archive for this milestone. If the metadata database or CSVs can be queried independently, first inspect their actual schema and select a small candidate list using metadata only. For each candidate, verify the source repository, exact version/commit where available, recorded license, architecture, and SHA-256 before retrieving any binary. A license label recorded in metadata is a lead for review, not legal advice or proof that NexaOS may redistribute the file.

The initial candidate set should aim for diversity rather than volume: several unrelated source projects, more than one compiler/linker configuration if available, and samples with different section counts and alignment patterns. Keep downloaded samples outside the repository unless redistribution rights are clear. Store only provenance and measured structural metadata in the repository; do not commit third-party binaries.

### Required per-sample record

For every selected sample, record:

- Dataset record identifier and binary path.
- Source repository URL, source revision/version, and recorded license.
- SHA-256 and exact file size.
- PE format and machine type.
- `SectionAlignment`, `FileAlignment`, `SizeOfImage`, and `SizeOfHeaders`.
- Section-table order and each section's RVA, `VirtualSize`, `SizeOfRawData`, and raw file offset.
- Certificate-table presence, debug-directory presence, and overlay byte count where safely measurable.
- Inspector result, planner result, tool commit, and any rejection reason.

All processing remains read-only. Never launch a corpus sample. No sample has been selected or validated by this documentation update; metadata schema inspection and license review are still required before any download.

### Revised next step

Build a metadata-only shortlist first, then inspect a handful of individually license-checked binaries. Use those observations to decide whether section ordering/adjacency or the zero-`VirtualSize` fallback needs code changes. Until those results exist, retain the current planner behavior and keep NCE-003 blocked.
