# NCE-005a: PE Corpus Source Audit and Initial Shortlist

## Status

**Initial source audit complete; three individually published upstream Windows binaries have now been inspected and passed the bounded image-layout/relocation integration path.** The binaries remain in local quarantine outside Git; none is committed or approved for redistribution. This small sample set is evidence of limited real-world PE coverage, not broad Windows compatibility.

NexaOS branch: `nce-001/pe-inspector`  
NexaOS baseline: `242ebb967ba232ad53071dec10f42da9dbb7ada1`

## Source review

### 1. LLVM Project COFF linker tests — approved for source-derived fixtures

- Repository: https://github.com/llvm/llvm-project
- Pinned source revision inspected: `9a18a94ec1ffb4d050d5d17d245c44b1f2d073e8`
- License: Apache License 2.0 with LLVM Exceptions, as stated in `llvm/LICENSE.TXT` at that revision.
- Relevant source directory: `lld/test/COFF`
- Method: use the repository's YAML/assembly test inputs and documented test commands to generate small PE outputs locally. The generated output is not a downloaded third-party application binary.
- Scope: deterministic PE32+ AMD64 examples for section layout, file alignment, imports, base/relocation data, and initialized/uninitialized section behavior.
- Restrictions: retain source revision and license notices; do not execute generated PE files. Only use the minimum source inputs/tooling needed to produce the test output. Generated files remain local or are represented by metadata unless redistribution is explicitly reviewed.

This source is suitable for reproducible structural fixtures. It is not a substitute for unrelated third-party application binaries.

### 2. corkami/pocs — deferred pending per-file rights review

- Repository: https://github.com/corkami/pocs
- The repository's GitHub API metadata currently reports no recognized repository license.
- Its PE directory contains hand-authored PE proof-of-concept sources and binary outputs. The project describes these as format demonstrations that can be unusual or intentionally push format limits.
- A separate Corkami PE corpus README surfaced in search and states BSD licensing for that corpus, but that is not the canonical repository's machine-readable license record. It is not sufficient to establish rights for every file in the current repository.
- Decision: do not retrieve or redistribute its binary outputs in this phase. Reconsider only after confirming the canonical repository's applicable license and the rights for each selected source/output. If later reviewed, use a tiny, specifically selected subset and never execute samples.

### 3. lief-project/samples — excluded from the initial shortlist

- Repository: https://github.com/lief-project/samples
- GitHub marks it archived; the repository describes itself as binaries used in test suites.
- Its README points to an external `lief_tests.zip` archive for the moved samples.
- GitHub API metadata reports no recognized repository license, and the archive's per-file provenance and licensing have not been established.
- Decision: do not download the archive or use its binaries unless provenance and licensing are independently confirmed for each candidate.

### 4. LLVM COFF tests versus a third-party corpus

The LLVM fixtures below are generated from source-controlled test inputs under a permissive project license. They can expand deterministic coverage, but they are not evidence that unrelated vendor-produced applications are accepted. The NCE-005 compatibility claim must remain limited until a separately reviewed third-party sample set is available.

## Initial reproducible fixture shortlist

All paths below are relative to the pinned LLVM revision above. The expected output is a PE image produced by the test's `lld-link` command, not the COFF object input itself.

| Candidate test | Intended structural evidence | Expected architecture | Status |
| --- | --- | --- | --- |
| `lld/test/COFF/sectionlayout.test` | Explicit non-default section ordering in the linked PE; checks both 1-3-2 and 3-2-1 layouts | AMD64 | Source inspected; output hash pending local generation |
| `lld/test/COFF/filealign.test` | Default 512-byte and explicit 4096-byte file alignment | AMD64 | Source inspected; output hash pending local generation |
| `lld/test/COFF/default-alignment.test` | Repeated `.rdata` input sections and linked section contents | AMD64 | Source inspected; output hash pending local generation |
| `lld/test/COFF/imports.test` | Import directory and import address/lookup table generation | AMD64 | Source inspected; output hash pending local generation |
| `lld/test/COFF/reloc-x64.test` | AMD64 relocation-bearing linked image and several COFF relocation inputs | AMD64 | Source inspected; output hash pending local generation |
| `lld/test/COFF/base.test` | Default and custom image bases; base-related linked contents | AMD64 | Source inspected; output hash pending local generation |
| `lld/test/COFF/bss-shared-no-merge.test` | Initialized `.data` and uninitialized `.bss` section behavior | AMD64 | Source inspected; output hash pending local generation |

These are candidate test cases, not claims that every resulting PE is accepted by NexaOS. Run the generated output through the existing inspector and layout planner before adding it to a passing corpus manifest. Record a failure as evidence; do not silently discard it.

### Cases not covered by this shortlist

- A real-world third-party vendor application set.
- A verified PE executable with `VirtualSize == 0` in one or more image sections.
- Certificate-table and overlay diversity.
- Current Windows loader acceptance behavior for disputed section layouts.
- A representative mix of MSVC, MinGW, and other independent vendor/toolchain releases beyond the fixtures already present in NexaOS CI.

Do not infer these properties from filenames or COFF object inputs. Inspect the generated PE headers and record exact observed values.

## Required metadata for each generated or third-party candidate

Record:

- Stable sample ID and source repository.
- Exact source commit/tag and relative source path.
- License and applicable notice location.
- Whether the PE was generated from source or obtained as a binary.
- Binary SHA-256 and byte length.
- PE signature, optional-header magic, machine, subsystem, image base, section/file alignment, header size, image size, and directory counts.
- For every section: table index, name, virtual address, virtual size, raw pointer, raw size, characteristics, and computed planner destination range.
- Certificate/debug directory observations and overlay length where present.
- Inspector result, planner result, preparation result if applicable, and tool commit.
- Any exclusion/rejection reason.

A hash is recorded only after the exact output bytes exist. Never invent a hash from a source file, repository commit, or expected build.

## Next execution steps

1. Generate only the first three small AMD64 fixtures from the pinned LLVM source tests: section layout, file alignment, and default alignment.
2. Inspect generated PE metadata without executing the output.
3. Compute SHA-256 and record byte sizes and observed headers/sections.
4. Run the existing NexaOS inspector and layout planner against each generated PE.
5. Expand to imports, relocations, image-base, and BSS fixtures if the first set is reproducible and resource-bounded.
6. Continue independent third-party source/license review separately. Do not call this source-derived fixture set a third-party corpus.

## Safety and redistribution

- No PE sample is to be executed, loaded, or launched.
- No third-party binaries are to be committed during this source-audit stage.
- Keep generated PE files outside the repository unless a later explicit redistribution review approves the exact files and notices.
- Do not retrieve large archives merely to inspect a handful of cases.
- Do not use samples with unclear provenance or licensing as evidence in the approved corpus.


### 4. Assemblage Windows GitHub dataset — primary third-party candidate source; metadata-first only

- Dataset documentation: https://assemblagedocs.readthedocs.io/en/latest/dataset.html
- Dataset card: https://huggingface.co/datasets/changliu8541/Assemblage_PE
- The current Assemblage dataset documentation lists approximately 91,000 Windows PE binaries built with MSVC, with a Windows GitHub dataset update dated 2025-05.
- The documented database schema exposes per-binary platform, build mode, toolset version, optimization, repository URL, source commit, license, file size, and hash. A separate archive stores the binaries.
- The dataset documentation explicitly says each binary retains the license of its originating repository; the Assemblage project's MIT license does not relicense the binaries.
- Decision: **approved for metadata-only candidate discovery, not blanket binary approval**. Query the metadata database first, then inspect the originating repository at the recorded commit and verify the exact license/notice before considering an individual binary. Do not fetch the multi-file binary archive or PDB archive as a first step.
- Selection preference: small PE32+ AMD64 executables from unrelated repositories, distinct MSVC toolset generations where metadata permits, and samples likely to add section/layout/directory diversity beyond the existing compiler fixtures. Avoid selecting many builds of the same project as if they were independent applications.
- Current status: no individual Assemblage binary has been selected, downloaded, hashed, or accepted by NexaOS.

### 5. Assemblage Windows vcpkg dataset — secondary candidate source; DLL-focused

- Dataset documentation: https://assemblagedocs.readthedocs.io/en/latest/dataset.html
- The documentation lists a Windows vcpkg dataset of DLLs built from vcpkg ports, with SQLite metadata and a separate binary archive.
- Decision: retain as a **secondary metadata-only source** if the Windows GitHub dataset does not provide enough independent DLL samples or useful section/directory diversity. Check the originating port and upstream project license for every candidate; a package-manager entry is not itself a redistribution grant for the compiled binary.
- Do not download the full archive. Select at most a few small AMD64 DLL candidates after inspecting metadata and rights.

### 6. Winbindex / Microsoft public symbol server — host-baseline reference only; not a redistributable corpus

- Index: https://winbindex.m417z.com/
- Index implementation and provenance notes: https://github.com/m417z/winbindex
- Winbindex indexes Windows binaries hosted on Microsoft's public symbol server and exposes SHA-256, PE timestamp, image size, version, architecture, Windows release/update association, and signing metadata.
- This provides strong version/hash provenance for Windows system binaries, but does not establish a general redistribution license for those Microsoft binaries.
- Decision: use only as a **reference for identifying/versioning Windows system files already present on a test host**, or for metadata comparison. Do not add symbol-server binaries to the repository or treat them as third-party application coverage. Any local inspection remains read-only and must record the host OS build and exact file hash.

### Metadata-first selection procedure

1. Obtain only the Assemblage Windows GitHub metadata database or a documented metadata export; do not download `binaries.tar.xz`, PDB archives, or the vcpkg binary archive at this stage.
2. Inspect the database schema and record the exact dataset snapshot, source URL, retrieval date, and metadata-file hash.
3. Filter metadata to small PE32+ AMD64 executable candidates, then deduplicate by source repository/project and binary hash. Prefer unrelated projects and distinct toolset generations.
4. For each proposed row, record its binary hash, byte size, repository URL, source commit, toolset, build configuration, and declared license from the dataset metadata.
5. Independently inspect the exact source repository revision and its license/notice. Exclude missing, ambiguous, incompatible, or untraceable rights; do not infer a binary's license from the dataset host or Assemblage generator.
6. Only after approval, retrieve one individually selected binary to a local quarantine directory outside the repository. Verify its SHA-256 and byte length against metadata before parsing it.
7. Run only NexaOS's bounded, non-executing inspection and planning paths. Record both accepted and rejected results. Do not load, launch, map as executable, or otherwise execute the sample.
8. Keep the binary local unless a separate review explicitly approves redistribution of that exact file and its notices. Commit only the metadata manifest and reproducible scripts where appropriate.

The metadata-first step is intentionally separate from sample acceptance: a database row is a candidate, not evidence that NexaOS parses or plans the corresponding PE successfully.


### 7. Individually published upstream Windows releases — practical small-sample shortlist

The Assemblage Windows PE dataset is not a practical first download for this phase: its current Hugging Face file listing shows a 33.8 GB binary archive and a 9.32 GB compressed SQLite archive (43.1 GB total), with no small standalone metadata file in the repository root. The dataset remains a potentially valuable future source if a compact metadata export or query service becomes available. Do not download either large archive merely to build this initial shortlist.

The following upstream release assets are more practical candidates because each is a small, versioned Windows release with an identifiable upstream project and published licensing information. These are **candidates only**; no binary has been downloaded or accepted yet.

| Candidate | Upstream release asset | Architecture | Source/license review | Candidate value | Status |
| --- | --- | --- | --- | --- | --- |
| ripgrep 15.2.0 | `ripgrep-15.2.0-x86_64-pc-windows-msvc.zip` — https://github.com/BurntSushi/ripgrep/releases/download/15.2.0/ripgrep-15.2.0-x86_64-pc-windows-msvc.zip | AMD64 | Release tag `15.2.0`; upstream identifies the project as dual-licensed MIT/Unlicense and provides both notices in the repository. | Compact Rust/MSVC CLI executable; useful real application PE distinct from compiler-generated fixtures. | **Locally inspected; image layout and relocation integration test passed.** |
| bat 0.26.1 | `bat-v0.26.1-x86_64-pc-windows-msvc.zip` — https://github.com/sharkdp/bat/releases/download/v0.26.1/bat-v0.26.1-x86_64-pc-windows-msvc.zip | AMD64 | Release tag `v0.26.1`; upstream identifies MIT OR Apache-2.0 licensing and provides corresponding license files. | Second independent Rust/MSVC application; useful comparison with ripgrep while not pretending to add a new compiler family. | **Locally inspected; image layout and relocation integration test passed.** |
| jq 1.8.2 | `jq-windows-amd64.exe` — https://github.com/jqlang/jq/releases/download/jq-1.8.2/jq-windows-amd64.exe | AMD64 | Release tag `jq-1.8.2`; upstream identifies MIT licensing and notes the included decNumber component's ICU license. Preserve all applicable notices. | C application binary, adding language/toolchain diversity beyond the Rust/MSVC candidates. | **Locally inspected; image layout and relocation integration test passed.** |

Candidate references:
- ripgrep release and signed tag: https://github.com/BurntSushi/ripgrep/releases/tag/15.2.0
- ripgrep license explanation: https://github.com/BurntSushi/ripgrep/blob/master/FAQ.md#how-is-ripgrep-licensed
- bat release: https://github.com/sharkdp/bat/releases/tag/v0.26.1
- bat license statement: https://github.com/sharkdp/bat
- jq release: https://github.com/jqlang/jq/releases/tag/jq-1.8.2
- jq license and decNumber notice: https://github.com/jqlang/jq

For any additional candidate considered for the corpus:
1. Download only the named release archive or executable from the upstream release URL into a local quarantine directory outside the repository.
2. Verify the exact release tag and asset name against the upstream release page; compute and record SHA-256 and byte length locally. Do not treat a third-party package-manager manifest hash as the final verified hash.
3. Extract only the intended executable from an archive without launching it. Record archive hash and executable hash separately.
4. Preserve the exact license and third-party notice files that accompany the release. Review the license terms for the binary and included dependencies; do not assume a source repository's top-level license automatically covers every bundled component.
5. Inspect PE headers and run the NexaOS inspector/planner only. Record architecture, section table, directories, overlay, planner outcome, and NexaOS commit.
6. Keep all binaries outside Git. Commit only metadata and license/provenance notes unless redistribution of the exact binary is separately approved.


## Real-world upstream sample validation — 2026-11-18

Three individually published upstream Windows AMD64 binaries were downloaded to user-local quarantine directories outside the repository and inspected as data only. Each temporary integration test exercised parsing, image-layout planning, non-executable buffer allocation/population, DIR64 relocation planning, and relocation application. Each temporary test file was removed after its run. No sample was launched.

These results are local validation evidence, not a redistributable corpus. The exact binaries remain outside Git. The hashes below were verified locally before and after their respective tests.

| Sample | Release asset / local binary | Verified SHA-256 | Size evidence | PE and relocation observations | Result |
| --- | --- | --- | --- | --- | --- |
| ripgrep 15.2.0 | `ripgrep-15.2.0-x86_64-pc-windows-msvc.zip`; extracted `rg.exe` | Archive: `71b2fef860abe467217a538ff31de02f5258807c0129f771846f87bd029aafc5`; executable: `14231169855EC5205CF5A1B6F1DB358FF4AED4247C86B69CE8AAE647C77F6680` | Archive: 1,789,611 bytes; planned image: 4,239,360 bytes | PE32+ AMD64; 7 sections; 6 import descriptors / 127 imported symbols; 118 relocation blocks; 12,852 entries (53 ABSOLUTE, 12,799 DIR64); no unsupported types reported. | **PASS** — 12,799 DIR64 patches planned and applied; original patch values and unchanged input verified. |
| bat 0.26.1 | `bat-v0.26.1-x86_64-pc-windows-msvc.zip`; extracted `bat.exe` | Archive: `0f729b4b6f5f28d395c641eacc2e9ff68d0096b85aa0eec344aa62425144b69b`; executable: `9EA9F7AB1C27AA5C9390E91543F9F47C6BD17BC53F3E30AC9DB762F28F55C4FD` | Archive: 3,613,318 bytes; executable: 7,342,592 bytes; planned image: 7,376,896 bytes | PE32+ AMD64; 5 sections; 4 import descriptors; 124 relocation blocks; 11,374 entries (52 ABSOLUTE, 11,322 DIR64); no unsupported types reported. | **PASS** — 11,322 DIR64 patches planned and applied; original patch values and unchanged input verified. |
| jq 1.8.2 | `jq-windows-amd64.exe` | Executable: `A6FC67FEDAF9128A3309A1E2EBB8B986AECCF70122EE46D2CB4849E423F0C627` | Executable: 1,035,264 bytes; planned image: 1,060,864 bytes | PE32+ AMD64; 10 sections including `.bss` and `.tls`; 14 import descriptors; 16 relocation blocks; 1,358 entries (4 ABSOLUTE, 1,354 DIR64); no unsupported types reported. Planner recorded 4,064 zero-fill bytes. | **PASS** — 1,354 DIR64 patches planned and applied; original patch values and unchanged input verified. |

### Aggregate evidence

- Three independently published upstream application binaries passed the same bounded image-layout and relocation integration path.
- Across the three samples: **25,584 relocation entries**, comprising **109 ABSOLUTE** entries and **25,475 DIR64** entries; **25,475 DIR64 patches** were planned and applied successfully.
- Each test checked every patch's original value before application and relocated value afterward. Each test confirmed the source input bytes remained unchanged.
- All samples were PE32+ AMD64 and all observed relocation entries were supported ABSOLUTE or DIR64 types.
- Execution was **not attempted**. No test resolved imports or exercised the full `prepare_image()` path; no fabricated export catalogue was used.
- These three command-line tools do not establish broad Windows application compatibility, compatibility with every compiler/linker, or Windows loader acceptance.

### Scope and remaining work

This evidence expands NCE-005a beyond source-generated fixtures to three real-world applications. It does not close the compatibility milestone. Remaining work includes:

1. **Completed 2026-11-18 on documentation commit `2618cbeae464486e0a175c8fdfe4caad5f33eca8`:** local Windows formatting, Clippy with warnings denied, all 177 workspace tests, and `git diff --check` passed; the working tree was clean. GitHub Actions on the same commit passed Rust checks (run `44335373829`), dependency security audit (run `44335373718`), and PE parser fuzz smoke test (run `44335373670`).
2. Continue compatibility testing with structurally distinct, individually licensed samples (including DLLs and additional toolchains where appropriate).
3. Exercise import resolution and the full preparation path only when genuine, complete export catalogues are available.
4. Measure time and peak memory on the project's defined low-end hardware before setting product resource defaults.
5. Keep all binaries in local quarantine. Do not commit binaries or imply redistribution approval from source-level license identification alone.


This shortlist provides a practical starting set, not a representative universal corpus. Three command-line tools cannot establish broad Windows application compatibility. Expand only after these samples are fully verified and their structural observations show what additional categories are still missing.
### SQLite 3.54.1 — Windows x64 DLL

**Source and provenance**

- Source: Official SQLite precompiled Windows x64 DLL archive.
- Version: SQLite 3.54.1.
- Official archive: `sqlite-dll-win-x64-3540100.zip`.
- Archive SHA3-256:
  `c2d09c8babb9399277055c057c97a53f1ffd7c5f11d0c1f2118ffdfa39f2daa4`
- DLL: `sqlite3.dll`.
- DLL SHA-256:
  `4B59D3CDED9DC1608544160F3B4B10AC974C6AC0D6FFC23064E5F4C8DBD05AEA`
- SQLite is public domain, according to its official copyright information.
- The archive and extracted DLL were kept outside the repository in the local quarantine directory.
- The DLL was inspected statically and was never executed.

**Inspected image**

- Format: PE32+ AMD64.
- Sections: 9.
- Preferred image base: `0x180000000`.
- Image size: 3,555,328 bytes.
- Header size: 1,024 bytes.
- Section alignment: 4,096 bytes.
- File alignment: 512 bytes.
- Entry-point RVA: 14,201.
- Imports: 1 descriptor (`KERNEL32.dll`), 127 symbols.
- Exports: 379 named exports.
- Relocation blocks: 30.
- Encoded relocation entries: 2,910.
- Writable-and-executable sections: 0.
- Execution status: `NOT_ATTEMPTED`.

**Static image and relocation integration test**

A temporary external integration test exercised the public `nexa_inspect` API against the quarantined DLL:

1. Parsed the original DLL with `inspect`.
2. Planned its image layout with `plan_image_layout`.
3. Allocated a non-executable image buffer.
4. Populated the image buffer from the input file.
5. Planned base relocations using the parsed relocation metadata.
6. Applied the resulting relocation plan to the image buffer.
7. Verified every original and relocated value.
8. Verified that the mapped image contained exactly the planned changes.
9. Verified that the original input bytes remained unchanged.

Test configuration and results:

- Preferred image base: `0x180000000`.
- Requested image base: `0x180100000`.
- Relocation delta: `0x100000`.
- ABSOLUTE entries: 16.
- DIR64 entries: 2,894.
- Unsupported relocation entries: 0.
- DIR64 patches planned and applied: 2,894.
- Image size: 3,555,328 bytes.
- Test result: **PASS** — 1 test passed, 0 failed.
- Temporary test source was removed after the run.
- Git working tree was clean after cleanup.

The test used static parsing, copying, planning, and byte-buffer modification only. It did not load or execute the DLL.

**Limitations**

- This test validates image layout and relocation planning/application for this DLL; it does not establish Windows loader compatibility.
- The DLL imports `KERNEL32.dll`. An authentic, trusted dependency export catalogue was not available for full import resolution.
- Full `prepare_image` with authentic dependency export data was not tested.
- This sample does not establish broad Windows DLL compatibility.
- Low-end hardware performance and memory measurements remain outstanding.
