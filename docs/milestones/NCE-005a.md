# NCE-005a: PE Corpus Source Audit and Initial Shortlist

## Status

**Initial source audit complete; no third-party binary corpus approved or downloaded.** This document records candidate sources and a reproducible source-fixture shortlist. It does not claim third-party compatibility coverage.

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
