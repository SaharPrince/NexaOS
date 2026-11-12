[CmdletBinding()]
param(
    [ValidateRange(1, 1000000)]
    [int]$PlanIterations = 1000,
    [ValidateRange(1, 1000000)]
    [int]$BufferIterations = 100,
    [ValidateRange(0, 30000)]
    [int]$HoldMilliseconds = 500
)

$ErrorActionPreference = "Stop"
$repoRoot = Split-Path -Parent $PSScriptRoot
Push-Location $repoRoot

$previousPlanIterations = $env:NEXA_BENCH_PLAN_ITERS
$previousBufferIterations = $env:NEXA_BENCH_BUFFER_ITERS
$previousHoldMilliseconds = $env:NEXA_BENCH_HOLD_MS

try {
    cargo bench --locked -p nexa-inspect --bench nexa_image --no-run
    if ($LASTEXITCODE -ne 0) {
        throw "Failed to build the NCE image benchmark (exit code $LASTEXITCODE)."
    }

    $benchDirectory = Join-Path $repoRoot "target\release\deps"
    $benchmark = Get-ChildItem -Path $benchDirectory -Filter "nexa_image-*.exe" -File |
        Sort-Object LastWriteTimeUtc -Descending |
        Select-Object -First 1

    if ($null -eq $benchmark) {
        throw "Could not find the compiled nexa_image benchmark in $benchDirectory."
    }

    $env:NEXA_BENCH_PLAN_ITERS = "$PlanIterations"
    $env:NEXA_BENCH_BUFFER_ITERS = "$BufferIterations"
    $env:NEXA_BENCH_HOLD_MS = "$HoldMilliseconds"

    Write-Host "Benchmark executable: $($benchmark.FullName)"
    $process = Start-Process -FilePath $benchmark.FullName -PassThru -NoNewWindow
    [long]$sampledPeakWorkingSet = 0

    while (-not $process.HasExited) {
        $process.Refresh()
        if ($process.WorkingSet64 -gt $sampledPeakWorkingSet) {
            $sampledPeakWorkingSet = $process.WorkingSet64
        }
        if ($process.PeakWorkingSet64 -gt $sampledPeakWorkingSet) {
            $sampledPeakWorkingSet = $process.PeakWorkingSet64
        }
        Start-Sleep -Milliseconds 5
    }

    # Ensure the process handle has been waited on before reading ExitCode.
    # HasExited can become true before Process.ExitCode is populated by PowerShell.
    $process.WaitForExit()
    $process.Refresh()
    $benchmarkExitCode = $process.ExitCode

    if ($process.WorkingSet64 -gt $sampledPeakWorkingSet) {
        $sampledPeakWorkingSet = $process.WorkingSet64
    }
    if ($process.PeakWorkingSet64 -gt $sampledPeakWorkingSet) {
        $sampledPeakWorkingSet = $process.PeakWorkingSet64
    }

    if ($benchmarkExitCode -ne 0) {
        throw "NCE image benchmark failed (exit code $benchmarkExitCode)."
    }

    if ($sampledPeakWorkingSet -le 0) {
        throw "Unable to capture a nonzero peak working set; increase -HoldMilliseconds and retry."
    }

    Write-Host ("peak_working_set_bytes={0}" -f $sampledPeakWorkingSet)
    Write-Host ("peak_working_set_mib={0:N2}" -f ($sampledPeakWorkingSet / 1MB))
    Write-Host "measurement=Windows process working-set high-water mark for the benchmark process only"
    Write-Host ("external_measurement_hold_ms={0} (excluded from benchmark phase timings)" -f $HoldMilliseconds)
    Write-Host "note=This is a synthetic microbenchmark, not a full application cold-load or system-wide memory measurement."
}
finally {
    $env:NEXA_BENCH_PLAN_ITERS = $previousPlanIterations
    $env:NEXA_BENCH_BUFFER_ITERS = $previousBufferIterations
    $env:NEXA_BENCH_HOLD_MS = $previousHoldMilliseconds
    Pop-Location
}
