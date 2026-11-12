[CmdletBinding()]
param(
    [ValidateRange(1, 1000000)]
    [int]$PlanIterations = 1000,
    [ValidateRange(1, 1000000)]
    [int]$BufferIterations = 100
)

$ErrorActionPreference = "Stop"
$repoRoot = Split-Path -Parent $PSScriptRoot
Push-Location $repoRoot

$previousPlanIterations = $env:NEXA_BENCH_PLAN_ITERS
$previousBufferIterations = $env:NEXA_BENCH_BUFFER_ITERS

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

    Write-Host "Benchmark executable: $($benchmark.FullName)"
    $process = Start-Process -FilePath $benchmark.FullName -PassThru -NoNewWindow
    [long]$sampledPeakWorkingSet = 0

    while (-not $process.HasExited) {
        $process.Refresh()
        if ($process.PeakWorkingSet64 -gt $sampledPeakWorkingSet) {
            $sampledPeakWorkingSet = $process.PeakWorkingSet64
        }
        Start-Sleep -Milliseconds 20
    }

    $process.Refresh()
    if ($process.PeakWorkingSet64 -gt $sampledPeakWorkingSet) {
        $sampledPeakWorkingSet = $process.PeakWorkingSet64
    }

    if ($process.ExitCode -ne 0) {
        throw "NCE image benchmark failed (exit code $($process.ExitCode))."
    }

    Write-Host ("peak_working_set_bytes={0}" -f $sampledPeakWorkingSet)
    Write-Host ("peak_working_set_mib={0:N2}" -f ($sampledPeakWorkingSet / 1MB))
    Write-Host "measurement=Windows process PeakWorkingSet64 for the benchmark process only"
    Write-Host "note=This is a synthetic microbenchmark, not a full application cold-load or system-wide memory measurement."
}
finally {
    $env:NEXA_BENCH_PLAN_ITERS = $previousPlanIterations
    $env:NEXA_BENCH_BUFFER_ITERS = $previousBufferIterations
    Pop-Location
}
