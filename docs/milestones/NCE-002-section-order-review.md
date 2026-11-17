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

All processing remains read-only. Never launch a corpus sample. At the time of this dataset-access triage, no Assemblage sample had been selected; metadata schema inspection and license review remain required before downloading one.

### Revised next step

Build a metadata-only shortlist first, then inspect a handful of individually license-checked binaries. Use those observations to decide whether section ordering/adjacency or the zero-`VirtualSize` fallback needs code changes. Until those results exist, retain the current planner behavior and keep NCE-003 blocked.

## Local Windows corpus checkpoint — 2026-11-17

This checkpoint records read-only measurements supplied from a Windows host and CI execution on commit `66e680a82fb797d69f048d9ebf6fdca20abcbd0e`. It does not change planner acceptance rules.

### Signed Windows system samples

Four local System32 executables were inspected and planned without execution: `where.exe`, `notepad.exe`, `cmd.exe`, and `WindowsPowerShell\\v1.0\\powershell.exe`.

All four had PE32+ AMD64 format, `SectionAlignment = 0x1000`, `FileAlignment = 0x1000`, ascending section-table RVA order, and exact adjacency between consecutive section extents after rounding the previous section's `VirtualSize` up to `SectionAlignment`. Across the four files, all 26 consecutive-section transitions were adjacent. Each file had nonzero `VirtualSize` for every listed section, so this sample set does not exercise the zero-`VirtualSize` fallback.

All four files returned a valid Authenticode signature with signer `CN=Microsoft Windows, O=Microsoft Corporation, L=Redmond, S=Washington, C=US` on the inspected host. Recorded SHA-256 values:

| Sample | Sections | SHA-256 |
|---|---:|---|
| `where.exe` | 7 | `F51FAB8041E5023D7290B540A2106F051EF0BD2BC3443C9DAF318628B560FA29` |
| `notepad.exe` | 8 | `468FFE129C395ABF6B21A09EFDF261910A95FB98AA982EAD73CAA7B2B684577E` |
| `cmd.exe` | 8 | `15CD209C995B7D3009C91CA40892D5B8F1FB162F3E5F10716642EA278E8E0D7B` |
| `powershell.exe` | 7 | `60C9B29843624DD8AF6B6A7B26147753BE6AEEF65098B9A503CC50850AFE410F` |

The `where.exe` sample was also run through the configured integration test on the user's host. It was 65,536 bytes, had 7 sections, and produced 50,416 copy bytes and 0 zero-fill bytes. Its six section transitions were individually reported as adjacent. The test result was 1 passed, 0 failed. The other three files were checked by the same local metadata-reporting workflow; their exact transition counts were 7, 7, and 6 respectively.

These four files are useful signed system samples, but they are not a diverse third-party application corpus. Their shared `fothk` section name is recorded as an observed property only; valid Authenticode signatures were verified on the inspected host. No conclusion about the origin or generality of that section name is drawn here.

### Independent compiler CI checkpoint

The GitHub Actions run for commit `66e680a` completed successfully. In particular, the `External compiler PE layout (MinGW + LLVM/LLD)` job completed both the fixture-generation step and the `Inspect and plan external compiler output` step. That workflow generates its MinGW-w64 GCC PE32+ AMD64 and PE32 x86 fixtures and Clang/LLD PE32+ AMD64 fixture from NexaOS-owned C sources; the generated executables are temporary CI artifacts and are not committed or executed.

The configured PE32+ tests check initialized data, virtual-only storage, raw-padding behavior, and successful layout planning. They also print section-transition diagnostics. The job's successful conclusion establishes that its current assertions passed; it does **not**, by itself, establish that every printed transition was adjacent, because the current integration tests do not assert section ordering or adjacency. Preserve that distinction until the CI diagnostic output is reviewed or explicit corpus assertions are added.

The Windows CI job also passed its configured `where.exe` layout test. The user's local configured `where.exe` test independently reported six adjacent transitions as recorded above.

### Updated decision

The signed Windows sample set supports ascending order and aligned adjacency for these four particular system files. It is not enough to establish universal loader acceptance rules, and the independently compiled fixture transition classifications have not yet been captured as asserted results. Keep the production planner permissive; do not reject gaps or out-of-order inputs solely on this evidence.

The zero-`VirtualSize` fallback remains untested by these four system files. Further corpus work should prioritize compiler-generated cases with zero virtual size, raw-only and empty sections, non-default alignments, and files from unrelated licensed source projects. Keep all samples read-only and outside the repository unless redistribution rights are explicit.


## File-only certificate, debug, and overlay metadata — 2026-11-17

The read-only inspector now reports the file length, trailing overlay range, and parsed IMAGE_DEBUG_DIRECTORY entries without changing image-layout planning or executing the input.

### Address-space handling

- The Security / Certificate Table directory remains a file-offset range. Its WIN_CERTIFICATE records are validated as file data and are not mapped into the image.
- The Debug data-directory entry itself is an RVA and is resolved through the existing RVA-to-file mapping.
- Each 28-byte IMAGE_DEBUG_DIRECTORY record retains both AddressOfRawData (RVA) and PointerToRawData (file offset) as separate fields.
- A nonzero PointerToRawData plus SizeOfData is bounds-checked directly against the input file. A nonzero AddressOfRawData plus SizeOfData is independently resolved as an RVA. A zero RVA is retained as file-only debug data; its file pointer is not passed to the RVA mapper.
- The report includes the file offset obtained from mapping AddressOfRawData, when present. It does not silently replace either declared address with the other or impose an equality rule between them.

Microsoft documents that debug-directory entries are 28 bytes, that AddressOfRawData can be zero for data not covered by a section header, and that PointerToRawData is a file pointer. The certificate table is likewise file-offset based and is not loaded as part of the image.

Reference: https://learn.microsoft.com/en-us/windows/win32/debug/pe-format

### Overlay reporting policy

The reported trailing overlay begins at the greater of SizeOfHeaders and the end of the furthest section raw-data range, and ends at the physical end of the input file. It is a file-layout observation only. It can include certificate-table bytes and unmapped debug data; those separately reported ranges may therefore overlap the overlay range. This field does not classify gaps between earlier file ranges as overlay and does not imply that all trailing bytes have the same purpose.

### Regression coverage and limits

Synthetic tests cover:
- A mapped debug payload, preserving its RVA-derived file offset separately from the declared raw file pointer.
- An unmapped debug payload referenced by file offset in the trailing overlay.
- A generic trailing overlay after the last section raw-data range.
- An out-of-bounds debug file pointer.
- A debug-directory size that is not a whole number of 28-byte records.

The parser caps debug-directory entry processing at 4,096 records. No debug payload is decoded, interpreted, executed, or copied into the image buffer. No production planner acceptance rules changed. These are synthetic metadata tests, not evidence of broad third-party corpus compatibility.


## Local Windows debug, overlay, and signature-type checkpoint — 2026-11-17

A follow-up read-only inspection was performed on the same four Windows System32 files using the inspector JSON report and PowerShell `Get-AuthenticodeSignature`. This resolves the apparent mismatch between a zero Security directory and a valid Authenticode result.

| Sample | File size | Debug records | Security directory | Trailing overlay |
|---|---:|---:|---|---|
| `where.exe` | 65,536 bytes | 4 | offset 0, size 0 | None |
| `notepad.exe` | 360,448 bytes | 4 | offset 0, size 0 | None |
| `cmd.exe` | 339,968 bytes | 4 | offset 0, size 0 | None |
| `powershell.exe` | 495,616 bytes | 4 | offset 0, size 0 | None |

Across all 16 debug-directory records, the reported `AddressOfRawData`, `PointerToRawData`, and RVA-mapped file offset were equal. The four records in each file had type codes 2, 13, 16, and 20. All declared debug payload file ranges were within the input files. No trailing overlay was reported for these samples.

PowerShell reported the following for every file:

- `Status = Valid`
- `SignatureType = Catalog`
- `IsOSBinary = True`
- Signer: `CN=Microsoft Windows, O=Microsoft Corporation, L=Redmond, S=Washington, C=US`
- Status message: `Signature verified.`

### Interpretation

There is no contradiction between the valid Authenticode results and the zero PE Security directory entries. These four files are catalog-signed on the inspected Windows host; the trust signature is supplied through the Windows catalog mechanism rather than an embedded PE Certificate Table. The zero Security directory therefore does not indicate a parser failure and must not be interpreted as proof that a file is unsigned.

This observation also demonstrates why PE structural metadata and host trust verification must be kept separate:

- The PE inspector reports whether the file declares an embedded Certificate Table and validates that file range when present.
- Windows Authenticode verification may report a valid catalog signature even when the PE Security directory is empty.
- The inspector does not perform catalog lookup, certificate-chain validation, revocation checking, or a general trust decision.

### Decision

No parser code change is justified by these four results. Preserve the current distinction between embedded certificate-table metadata and external catalog-signature trust. Future documentation and user-facing output must avoid equating an empty Security directory with an unsigned or untrusted executable.

These four files provide useful examples of catalog-signed Windows system binaries with four debug records and no reported trailing overlay. They remain a small, same-host system sample set, not a diverse third-party application corpus or proof of broad compatibility.
