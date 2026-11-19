# NCE-005c: Low-End Resource Baseline

## Status

**Measurement automation added; hosted Linux and physical low-end-device evidence pending. This milestone is open.**

This milestone extends the synthetic, non-executing image-preparation measurements from NCE-005b. It does not authorize PE execution, loader behavior, planner acceptance changes, or new compatibility claims.

## Scope and measurement boundaries

The benchmark workload remains the deterministic synthetic PE32+ AMD64 fixture documented in [NCE-005b](NCE-005b.md). The benchmark inspects and prepares bytes in memory; no PE entry point or sample code is executed.

The Linux helper is `scripts/measure-image-bench-linux.sh`. It builds the harness-free benchmark once, then launches a fresh benchmark process for each repetition. For every process it records GNU `time` maximum resident set size (`%M`, reported in KiB) independently from the benchmark's phase timings.

The helper records:
- OS/kernel string and architecture;
- CPU model when exposed by `/proc/cpuinfo), logical processor count, and installed memory from `/proc/meminfo`;
- Rust and Cargo versions;
- benchmark iteration settings and repetition count;
- each run's phase output and process maximum RSS;
- min, median, nearest-rank p90, and max across repetitions.

The CI workflow invokes five fresh processes on the hosted Ubuntu runner and uploads the transcript as a seven-day artifact. These are hosted-CI observations only. The runner is not a low-end device and must not be represented as one.

## What the measurements mean

Keep these metrics separate:

1. **Phase timing:** wall-clock nanoseconds per iteration for the benchmark operations.
2. **Process maximum RSS:** GNU `time` maximum resident set size for the benchmark process, reported in KiB. This is not system-wide memory use, a private-bytes measurement, or total heap allocation.
3. **Logical image-buffer allocator counters:** reported by the Rust benchmark. These count calls and requested bytes through the image-buffer allocator interface, not every process allocation.

The external hold interval is a sampling aid and is excluded from the benchmark phase timers. Repeated results are diagnostic observations, not statistical performance guarantees.

## Physical low-end device evidence

**Not yet collected.** Hosted CI cannot substitute for a real low-end machine. Before making any low-end readiness or resource-budget decision, collect repeated measurements on a representative physical device and record:

- exact CPU model and architecture;
- installed RAM and OS version/build;
- Rust/Cargo versions and target architecture;
- power mode and whether the device was idle or under other load;
- exact repository commit and benchmark settings;
- raw transcript for each repeated run;
- min, median, nearest-rank p90, and max for every phase and process memory metric.

On the Windows lab PC, use the existing helper:

```powershell
./scripts/measure-image-bench.ps1 -PlanIterations 1000 -BufferIterations 200 -PreparationIterations 200 -Runs 5 -HoldMilliseconds 1000
```

Also record CPU model and installed RAM independently if the helper's CIM inventory reports them as unavailable. Do not infer missing hardware details.

A physical device with a clearly documented low-memory configuration is required before calling the results a low-end baseline. Do not invent a RAM floor, latency budget, or universal performance threshold from hosted-runner observations.

## Exit criteria

- [ ] Linux helper completes repeated fresh-process measurements and reports all expected phases and RSS summaries.
- [ ] Hosted Linux workflow run succeeds and its raw transcript artifact is retained.
- [ ] A physical low-end device is identified and its hardware/software inventory is recorded.
- [ ] Repeated physical-device measurements and raw output are recorded.
- [ ] Evidence is reviewed before any resource budget, compatibility decision, or readiness claim is proposed.

NCE-005c remains open until the physical-device evidence and review are complete.
