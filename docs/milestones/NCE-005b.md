# NCE-005b: End-to-End Image Preparation Benchmark

## Status

**Initial diagnostic benchmark implemented and exercised on Windows. Evidence recorded; milestone exit criteria remain partially open.**

The benchmark implementation was committed on the local branch in commit:

`013542ec19916ab09d2d509ed85131f673a7ba01`

This report records the current synthetic-fixture benchmark, repeated diagnostic timing observations, Windows process working-set samples, and validation results. It does not claim statistical performance significance, real-world corpus coverage, loader compatibility, or completion of the broader NCE-005 resource-readiness milestone.

This report was committed and published on the NCE-005b branch in commit `09c19b078a89513aab2f33e277fa4f6458569a2f`.

## Goal

Measure the PE image preparation pipeline in distinct phases and as a complete operation, while keeping the work static and non-executing.

The benchmark covers:

1. PE parsing and inspection.
2. Image layout planning.
3. Image-buffer allocation.
4. Allocation and population together.
5. Complete `prepare_image` operation using a counting image allocator.
6. Complete `prepare_image` operation using the system image allocator.

The benchmark is diagnostic. It is intended to establish an initial reproducible workload and expose measurement gaps, not to establish production performance guarantees.

## Implementation

The new benchmark is:

`engine/pe-inspector/benches/nexa_image.rs`

It is registered as a harness-free Cargo benchmark target in:

`engine/pe-inspector/Cargo.toml`

The pre-existing layout benchmark remains available at:

`engine/pe-inspector/benches/nexa_image_layout.rs`

The original layout benchmark was restored and preserved byte-for-byte relative to its parent revision. Its workload is distinct from the new end-to-end benchmark and its timing results must not be compared as if they measured identical work.

## Synthetic fixture

The new benchmark constructs deterministic PE32+ AMD64 input bytes.

| Property | Value |
|---|---:|
| Input size | 1,024 bytes |
| Image size | 8,192 bytes |
| Sections | 1 |
| Bytes copied | 1,024 |
| Bytes zero-filled | 256 |
| Imports | 0 |
| Relocation entries | 0 |

The full preparation path uses an empty export catalogue and the configured default import, IAT, and relocation limits.

The fixture is deliberately small and deterministic. It exercises the no-import/no-relocation preparation path; it is not a representative real-world PE corpus.

The benchmark reports `execution: NOT_ATTEMPTED`. No PE entry point or sample code is executed.

## Measurement methodology

The benchmark records wall-clock elapsed time for each phase and logical image allocator calls and requested bytes.

The counting allocator measures calls to the image-buffer allocator. It does not count all process-wide heap allocations.

The full preparation measurements use the same fixture and pipeline, with two allocator configurations:

- Counting image allocator.
- System image allocator.

The phase named `image_allocation_and_population` intentionally includes allocation in its timed interval. A separate `population_only` phase now allocates a fresh destination buffer before each timed sample, then times only `populate_image_buffer`. The allocation setup is reported separately as `population_only_setup_allocator_*`; setup time is excluded from the population timing.

The optional `NEXA_BENCH_HOLD_MS` delay allows the external PowerShell helper to sample the benchmark process working set. The hold occurs after benchmark phase measurements and is excluded from the reported phase timings.

## Benchmark configuration

The new benchmark uses these environment variables:

| Variable | Default |
|---|---:|
| `NEXA_BENCH_PARSE_ITERS` | 1,000 |
| `NEXA_BENCH_LAYOUT_ITERS` | 1,000 |
| `NEXA_BENCH_BUFFER_ITERS` | 200 (also controls population-only samples) |
| `NEXA_BENCH_PREPARE_ITERS` | 200 |

Iteration values are clamped to the range 1 through 1,000,000.

`NEXA_BENCH_HOLD_MS` controls the optional external-measurement hold and is clamped to a maximum of 30,000 milliseconds.

The existing PowerShell helper, `scripts/measure-image-bench.ps1`, builds and launches the benchmark and samples the process working-set high-water mark. It reports that measurement separately from benchmark phase timings.

**Helper configuration correction:** the PowerShell helper now maps `-PlanIterations` to `NEXA_BENCH_LAYOUT_ITERS`. The corrected mapping was verified with a run using 7 layout iterations and 3 buffer iterations. The helper now also exposes `-PreparationIterations` and maps it to `NEXA_BENCH_PREPARE_ITERS`; the previous process environment value is restored when the helper exits.

## Windows environment

The environment information returned during evidence collection was:

| Property | Observed value |
|---|---|
| Operating system | Microsoft Windows 10 IoT Enterprise LTSC |
| OS version | 10.0.19044 |
| OS build | 19044 |
| Architecture | 64-bit |
| Power plan | High performance |
| Rust compiler | `rustc 1.100.0 (0ef1bc386 2026-11-09)` |
| Cargo | `cargo 1.100.0 (3acc79c0c 2026-11-05)` |

The PowerShell queries for CPU and installed memory returned no property values in the captured output. CPU model, core counts, logical processor count, and installed RAM are therefore **not recorded**.

The exact benchmark host hardware cannot be fully characterized from this evidence.

## End-to-end timing observations

The following are observed per-iteration averages from separate runs of the new benchmark. Values are nanoseconds per iteration.

| Run | Counting allocator | System allocator |
|---|---:|---:|
| Initial default run | 13,675 | 12,040 |
| Windows sample run A | 17,664 | 10,696 |
| Windows sample run B | 10,109 | 9,941 |
| Validation run A | 30,125 | 14,257 |
| Windows sample run C | 10,519 | 9,861 |
| Validation run B | 13,146 | 13,731 |

These are individual diagnostic observations, not statistical estimates. The variation demonstrates why a single run must not be used to claim that one allocator is consistently faster.

The benchmark uses 200 iterations for each preparation allocator measurement in these runs.

## Additional phase observations

One captured Windows sample run reported:

| Phase | Iterations | Total time | Average |
|---|---:|---:|---:|
| Parse and inspect | 1,000 | 7.847 ms | 7,846 ns |
| Layout planning | 1,000 | 0.643 ms | 643 ns |
| Image-buffer allocation | 200 | 0.170 ms | 852 ns |
| Allocation and population | 200 | 0.269 ms | 1,345 ns |

Another validation run reported:

| Phase | Iterations | Total time | Average |
|---|---:|---:|---:|
| Parse and inspect | 1,000 | 8.195 ms | 8,195 ns |
| Layout planning | 1,000 | 0.659 ms | 658 ns |
| Image-buffer allocation | 200 | 0.242 ms | 1,212 ns |
| Allocation and population | 200 | 0.324 ms | 1,622 ns |

These phase timings are also diagnostic observations and should not be treated as stable performance guarantees.

## Corrected helper verification

The corrected PowerShell helper was run with:

```powershell
.\scripts\measure-image-bench.ps1 `
    -PlanIterations 7 `
    -BufferIterations 3 `
    -HoldMilliseconds 0
```

The output confirmed:

| Phase | Iterations | Total time | Average |
|---|---:|---:|---:|
| Layout planning | 7 | 0.025 ms | 3,500 ns |
| Image-buffer allocation | 3 | 0.031 ms | 10,433 ns |
| Allocation and population | 3 | 0.053 ms | 17,533 ns |
| Full preparation, counting allocator | 200 | 3.547 ms | 17,736 ns |
| Full preparation, system allocator | 200 | 2.798 ms | 13,988 ns |

The helper reported a process working-set high-water mark of 3,170,304 bytes (3.02 MiB). The external measurement hold was zero milliseconds. These historical measurements predate the isolated population-only phase.

This verifies the layout and buffer iteration mappings. The preparation iteration count was previously fixed at the benchmark default of 200. The helper now exposes `-PreparationIterations` (default 200), allowing the complete preparation phases to be configured independently.

This was a configuration verification run with deliberately small iteration counts, not a directly comparable performance run.
## Logical allocation accounting

For the 8,192-byte image and 200 iterations, the expected logical image-buffer request is:

`200 × 8,192 = 1,638,400 bytes`

The benchmark reported:

| Measurement | Calls | Requested bytes |
|---|---:|---:|
| Allocation-only phase | 200 | 1,638,400 |
| Allocation and population | 200 | 1,638,400 |
| Full preparation with counting allocator | 200 | 1,638,400 |

These counters describe the image-buffer allocator interface. They do not represent process-wide allocation totals or peak memory consumption.

## Windows working-set observations

The existing PowerShell helper was run with:

`-BufferIterations 200 -HoldMilliseconds 1000`

Three captured observations were:

| Observation | Peak working-set bytes | Peak working-set MiB |
|---|---:|---:|
| A | 3,178,496 | 3.03 |
| B | 3,186,688 | 3.04 |
| C | 3,178,496 | 3.03 |

These values are the Windows process working-set high-water marks reported by the helper for the benchmark process.

They are not measurements of:

- The complete NexaOS application.
- A cold application startup.
- System-wide memory consumption.
- Peak process private bytes.
- A low-memory machine under system pressure.

The one-second external hold is a sampling aid, not a guarantee that every transient working-set peak was observed.

## Preserved layout benchmark

The original layout benchmark remains a separate diagnostic workload.

Its reported fixture is:

| Property | Value |
|---|---:|
| Sections | 4 |
| Image size | 524,288 bytes |
| Input size | 262,656 bytes |
| Copy bytes | 262,656 |
| Zero-fill bytes | 131,072 |

A captured run reported:

| Phase | Iterations | Average |
|---|---:|---:|
| Planning | 1,000 | 1,358 ns |
| Image-buffer allocation | 100 | 52,944 ns |
| Population only | 100 | 29,461 ns |

A further run reported 1,104 ns planning, 63,795 ns allocation, and 30,010 ns population-only.

The older benchmark constructs its own layout workload. Its population-only timing is not an isolated population-only measurement for the new benchmark's deterministic PE fixture. These values must not be combined with the new benchmark's timings as if the fixtures and measured operations were equivalent.

## Validation evidence

The following commands completed successfully during the recorded validation runs:

```powershell
cargo fmt --all
cargo check -p nexa-inspect --benches --locked
cargo bench -p nexa-inspect --bench nexa_image_layout --locked
cargo bench -p nexa-inspect --bench nexa_image --locked
cargo test --workspace --locked
cargo clippy --workspace --all-targets --locked -- -D warnings
cargo fmt --all -- --check
git diff --check
```

The workspace test result was:

| Test group | Passed |
|---|---:|
| Library unit tests | 146 |
| Binary unit tests | 6 |
| Compiler PE layout integration test | 1 |
| External compiler PE integration tests | 7 |
| Image layout compatibility integration tests | 16 |
| Image preparation public API integration test | 1 |
| **Total** | **177** |

Result: **177 passed, 0 failed**.

The external compiler integration tests reported success in the captured run. Their names indicate optional fixture-dependent coverage; this report does not infer that every optional external compiler was installed or exercised.

## Limitations and outstanding work

The current evidence is an initial baseline, not completion of all NCE-005b requirements.

1. **Population-only measurement:** an isolated population-only phase is now included for the same deterministic fixture. It allocates each destination before timing and reports setup allocator calls separately. The first measurements from this new phase still need to be collected and recorded.
2. **Imports:** the new full-preparation fixture has no imports. An import-bearing deterministic preparation case remains outstanding.
3. **Relocations:** the new full-preparation fixture has no relocation entries. A relocation-bearing deterministic preparation case remains outstanding.
4. **Environment completeness:** CPU identity, core counts, and installed RAM were not captured successfully.
5. **Statistical repeatability:** the observations show run-to-run variation. A controlled repeated-run procedure with summarized distributions remains outstanding.
6. **Memory coverage:** only Windows process working-set high-water observations were collected. Process-wide allocation counts, peak RSS on other platforms, and broader application memory behavior remain unmeasured.
7. **Low-end hardware:** no 4-GB-class Windows or Linux baseline is included here.
8. **Real-world corpus:** this benchmark uses a synthetic fixture and does not replace the separately documented NCE-005a real-world sample evidence.
9. **Compatibility:** benchmark completion does not establish Windows loader compatibility or broad PE compatibility.
10. **Helper configuration:** the layout-iteration mapping has been corrected and tested. The helper does not currently expose a separate preparation-iteration parameter.

## Follow-up work

- Consider exposing a separate preparation-iteration parameter in the PowerShell helper.
- Capture CPU model, core and logical processor counts, and installed memory.
- Define and run a controlled repeated benchmark procedure; report distributions rather than selecting the best run.
- Add deterministic import-bearing and relocation-bearing preparation fixtures where feasible.
- Collect and record population-only measurements from the new same-fixture phase.
- Collect low-end Windows and Linux resource baselines under NCE-005c.
- Update the parent NCE-005 milestone with the evidence and remaining gaps before making compatibility or cost decisions under NCE-005d.

## Security and execution boundary

All benchmark work is static preparation and measurement.

- The fixture is deterministic synthetic PE data.
- The benchmark does not execute the input or prepared image.
- No PE entry point is invoked.
- No process is created to run a PE sample.
- No executable memory or loader behavior is exercised.
- The prepared image remains data for inspection and measurement.

The `execution: NOT_ATTEMPTED` output is an explicit part of the benchmark's safety boundary.

## Reproduction

From the repository root:

```powershell
cargo bench -p nexa-inspect --bench nexa_image --locked
cargo bench -p nexa-inspect --bench nexa_image_layout --locked
```

For a Windows working-set observation:

```powershell
.\scripts\measure-image-bench.ps1 `
    -BufferIterations 200 `
    -PreparationIterations 200 `
    -HoldMilliseconds 1000
```

The helper's `-PlanIterations` parameter controls layout-planning iterations, `-BufferIterations` controls allocation and population-only iterations, and `-PreparationIterations` controls both complete preparation allocator phases.

## References

- `docs/milestones/NCE-005.md`
- `docs/milestones/NCE-005a.md`
- `engine/pe-inspector/benches/nexa_image.rs`
- `engine/pe-inspector/benches/nexa_image_layout.rs`
- `engine/pe-inspector/Cargo.toml`
- `scripts/measure-image-bench.ps1`

