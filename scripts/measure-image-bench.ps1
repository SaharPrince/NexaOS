[CmdletBinding()]
param(
    [ValidateRange(1, 1000000)]
    [int]$PlanIterations = 1000,
    [ValidateRange(1, 1000000)]
    [int]$BufferIterations = 100,
    [ValidateRange(1, 1000000)]
    [int]$PreparationIterations = 200,
    [ValidateRange(1, 20)]
    [int]$Runs = 5,
    [ValidateRange(0, 30000)]
    [int]$HoldMilliseconds = 500
)

function Get-Median {
    param([long[]]$Values)
    $ordered = @($Values | Sort-Object)
    $middle = [int][Math]::Floor($ordered.Count / 2)
    if (($ordered.Count % 2) -eq 1) {
        return [long]$ordered[$middle]
    }
    return [long][Math]::Round(
        ([double]$ordered[$middle - 1] + [double]$ordered[$middle]) / 2.0,
        [MidpointRounding]::AwayFromZero
    )
}

function Get-NearestRankPercentile {
    param(
        [long[]]$Values,
        [ValidateRange(1, 100)]
        [int]$Percentile
    )
    $ordered = @($Values | Sort-Object)
    $index = [int][Math]::Ceiling(($Percentile / 100.0) * $ordered.Count) - 1
    return [long]$ordered[[Math]::Max(0, $index)]
}

$ErrorActionPreference = "Stop"
$repoRoot = Split-Path -Parent $PSScriptRoot
Push-Location $repoRoot
$previousLayoutIterations = $env:NEXA_BENCH_LAYOUT_ITERS
$previousBufferIterations = $env:NEXA_BENCH_BUFFER_ITERS
$previousPreparationIterations = $env:NEXA_BENCH_PREPARE_ITERS
$previousHoldMilliseconds = $env:NEXA_BENCH_HOLD_MS
$process = $null

try {
    cargo bench --locked -p nexa-inspect --bench nexa_image --no-run
    if ($LASTEXITCODE -ne 0) {
        throw "Failed to build the NCE image benchmark (exit code $LASTEXITCODE)."
    }

    $benchDirectory = Join-Path $repoRoot "target\release"
    # Cargo may place harness-free bench executables in package build output
    # directories rather than target\release\deps. Search the release tree.
    $benchmark = Get-ChildItem -Path $benchDirectory -Filter "nexa_image-*.exe" -File -Recurse |
        Sort-Object LastWriteTimeUtc -Descending |
        Select-Object -First 1
    if ($null -eq $benchmark) {
        throw "Could not find the compiled nexa_image benchmark anywhere under $benchDirectory."
    }

    $env:NEXA_BENCH_LAYOUT_ITERS = "$PlanIterations"
    $env:NEXA_BENCH_BUFFER_ITERS = "$BufferIterations"
    $env:NEXA_BENCH_PREPARE_ITERS = "$PreparationIterations"
    $env:NEXA_BENCH_HOLD_MS = "$HoldMilliseconds"

    $phaseSamples = @{}
    $workingSetSamples = New-Object 'System.Collections.Generic.List[long]'
    Write-Host "Benchmark executable: $($benchmark.FullName)"
    Write-Host "measurement_runs=$Runs"
    Write-Host "plan_iterations=$PlanIterations"
    Write-Host "buffer_iterations=$BufferIterations"
    Write-Host "preparation_iterations=$PreparationIterations"
    Write-Host "hold_milliseconds=$HoldMilliseconds"

    $cpuName = $null
    $installedMemoryBytes = $null
    try {
        $processor = Get-CimInstance Win32_Processor -ErrorAction Stop |
            Select-Object -First 1
        $computer = Get-CimInstance Win32_ComputerSystem -ErrorAction Stop
        $cpuName = $processor.Name
        $installedMemoryBytes = [long]$computer.TotalPhysicalMemory
    }
    catch {
        Write-Host "hardware_inventory=unavailable"
    }
    if ($cpuName) {
        Write-Host "cpu_name=$cpuName"
    }
    Write-Host "logical_processor_count=$([Environment]::ProcessorCount)"
    if ($installedMemoryBytes) {
        Write-Host "installed_memory_bytes=$installedMemoryBytes"
    }

    for ($runIndex = 1; $runIndex -le $Runs; $runIndex++) {
        Write-Host ""
        Write-Host "=== BENCHMARK RUN $runIndex OF $Runs ==="
        # Drain redirected output asynchronously while sampling working set,
        # avoiding a full stdout pipe from blocking the benchmark process.
        $startInfo = New-Object System.Diagnostics.ProcessStartInfo
        $startInfo.FileName = $benchmark.FullName
        $startInfo.WorkingDirectory = $repoRoot
        $startInfo.UseShellExecute = $false
        $startInfo.CreateNoWindow = $true
        $startInfo.RedirectStandardOutput = $true
        $startInfo.RedirectStandardError = $true

        $process = New-Object System.Diagnostics.Process
        $process.StartInfo = $startInfo
        if (-not $process.Start()) {
            throw "Failed to start benchmark run $runIndex."
        }
        $stdoutTask = $process.StandardOutput.ReadToEndAsync()
        $stderrTask = $process.StandardError.ReadToEndAsync()
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

        $process.WaitForExit()
        $process.Refresh()
        $benchmarkExitCode = [int]$process.ExitCode
        $stdout = $stdoutTask.Result
        $stderr = $stderrTask.Result
        if ($process.WorkingSet64 -gt $sampledPeakWorkingSet) {
            $sampledPeakWorkingSet = $process.WorkingSet64
        }
        if ($process.PeakWorkingSet64 -gt $sampledPeakWorkingSet) {
            $sampledPeakWorkingSet = $process.PeakWorkingSet64
        }
        if ($stdout) {
            Write-Host $stdout.TrimEnd()
        }
        if ($stderr) {
            Write-Host $stderr.TrimEnd()
        }
        if ($benchmarkExitCode -ne 0) {
            throw "Benchmark run $runIndex failed (exit code $benchmarkExitCode)."
        }
        if ($sampledPeakWorkingSet -le 0) {
            throw "Unable to capture a nonzero working set for run $runIndex."
        }

        $workingSetSamples.Add($sampledPeakWorkingSet)
        foreach ($line in ($stdout -split '[\r\n]+')) {
            if ($line -match '^(?<phase>[A-Za-z0-9_]+): iterations=(?<iterations>\d+), total_ms=(?<total>[0-9.]+), ns_per_iteration=(?<ns>\d+)') {
                $phase = $Matches.phase
                if (-not $phaseSamples.ContainsKey($phase)) {
                    $phaseSamples[$phase] = New-Object 'System.Collections.Generic.List[long]'
                }
                $phaseSamples[$phase].Add([long]$Matches.ns)
            }
        }

        $process.Dispose()
        $process = $null
        Write-Host ("run_{0}_peak_working_set_bytes={1}" -f $runIndex, $sampledPeakWorkingSet)
    }

    if ($phaseSamples.Count -eq 0) {
        throw "No benchmark phase measurements could be parsed from standard output."
    }

    Write-Host ""
    Write-Host "=== REPEATED-RUN PHASE SUMMARY (NANOSECONDS / ITERATION) ==="
    foreach ($phase in ($phaseSamples.Keys | Sort-Object)) {
        $values = [long[]]$phaseSamples[$phase].ToArray()
        if ($values.Count -ne $Runs) {
            throw "Phase '$phase' produced $($values.Count) samples; expected $Runs."
        }
        $minimum = ($values | Measure-Object -Minimum).Minimum
        $maximum = ($values | Measure-Object -Maximum).Maximum
        $median = Get-Median -Values $values
        $p90 = Get-NearestRankPercentile -Values $values -Percentile 90
        Write-Host (
            "phase_summary={0}, samples={1}, min_ns={2}, median_ns={3}, p90_ns={4}, max_ns={5}" -f
            $phase, $values.Count, $minimum, $median, $p90, $maximum
        )
    }

    $workingSetValues = [long[]]$workingSetSamples.ToArray()
    $workingSetMinimum = ($workingSetValues | Measure-Object -Minimum).Minimum
    $workingSetMaximum = ($workingSetValues | Measure-Object -Maximum).Maximum
    $workingSetMedian = Get-Median -Values $workingSetValues
    $workingSetP90 = Get-NearestRankPercentile -Values $workingSetValues -Percentile 90
    Write-Host ""
    Write-Host "=== WORKING-SET SUMMARY (BYTES) ==="
    Write-Host (
        "working_set_summary=samples={0}, min_bytes={1}, median_bytes={2}, p90_bytes={3}, max_bytes={4}" -f
        $workingSetValues.Count, $workingSetMinimum, $workingSetMedian, $workingSetP90, $workingSetMaximum
    )
    Write-Host "measurement=Windows process working-set high-water mark for the benchmark process only"
    Write-Host "statistics=nearest-rank p90; repeated-run diagnostic summary, not a performance guarantee"
    Write-Host "note=This is a synthetic microbenchmark, not a full application cold-load or system-wide memory measurement."
}
finally {
    if ($null -ne $process) {
        $process.Dispose()
    }
    $env:NEXA_BENCH_LAYOUT_ITERS = $previousLayoutIterations
    $env:NEXA_BENCH_BUFFER_ITERS = $previousBufferIterations
    $env:NEXA_BENCH_PREPARE_ITERS = $previousPreparationIterations
    $env:NEXA_BENCH_HOLD_MS = $previousHoldMilliseconds
    Pop-Location
}
