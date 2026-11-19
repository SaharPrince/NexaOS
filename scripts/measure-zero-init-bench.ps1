[CmdletBinding()]
param(
    [ValidateRange(1, 20)]
    [int]$Runs = 5,
    [ValidateRange(1, 1000000)]
    [int]$Iterations = 2000,
    [ValidateRange(1, 67108864)]
    [int]$ImageBytes = 8192
)

$ErrorActionPreference = "Stop"
$repoRoot = Split-Path -Parent $PSScriptRoot
Push-Location $repoRoot
$process = $null

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
    param([long[]]$Values, [int]$Percentile)
    $ordered = @($Values | Sort-Object)
    $index = [int][Math]::Ceiling(($Percentile / 100.0) * $ordered.Count) - 1
    return [long]$ordered[[Math]::Max(0, $index)]
}

try {
    Write-Host "=== BUILD DUPLICATE ZERO-INITIALIZATION DIAGNOSTIC ==="
    cargo bench --locked -p nexa-inspect --bench nexa_zero_init --no-run
    if ($LASTEXITCODE -ne 0) {
        throw "Failed to build nexa_zero_init (exit code $LASTEXITCODE)."
    }

    $benchDirectory = Join-Path $repoRoot "target\release"
    $benchmark = Get-ChildItem -Path $benchDirectory -Filter "nexa_zero_init-*.exe" -File -Recurse |
        Sort-Object LastWriteTimeUtc -Descending |
        Select-Object -First 1
    if ($null -eq $benchmark) {
        throw "Could not find the compiled nexa_zero_init executable under $benchDirectory."
    }

    Write-Host "benchmark_executable=$($benchmark.FullName)"
    Write-Host "fresh_process_runs=$Runs"
    Write-Host "iterations_per_phase=$Iterations"
    Write-Host "image_bytes=$ImageBytes"
    Write-Host "execution_expected=NOT_ATTEMPTED"

    $expectedPhases = @("allocator_allocate_initialized", "isolated_second_clear")
    $samples = @{
        allocator_allocate_initialized = New-Object 'System.Collections.Generic.List[long]'
        isolated_second_clear = New-Object 'System.Collections.Generic.List[long]'
    }

    for ($runIndex = 1; $runIndex -le $Runs; $runIndex++) {
        Write-Host ""
        Write-Host "=== FRESH PROCESS RUN $runIndex OF $Runs ==="

        $startInfo = New-Object System.Diagnostics.ProcessStartInfo
        $startInfo.FileName = $benchmark.FullName
        $startInfo.WorkingDirectory = $repoRoot
        $startInfo.UseShellExecute = $false
        $startInfo.CreateNoWindow = $true
        $startInfo.RedirectStandardOutput = $true
        $startInfo.RedirectStandardError = $true
        $startInfo.EnvironmentVariables["NEXA_ZERO_INIT_ITERS"] = "$Iterations"
        $startInfo.EnvironmentVariables["NEXA_ZERO_INIT_BYTES"] = "$ImageBytes"

        $process = New-Object System.Diagnostics.Process
        $process.StartInfo = $startInfo
        if (-not $process.Start()) {
            throw "Failed to start diagnostic process $runIndex."
        }

        $stdoutTask = $process.StandardOutput.ReadToEndAsync()
        $stderrTask = $process.StandardError.ReadToEndAsync()
        $process.WaitForExit()
        $exitCode = [int]$process.ExitCode
        $stdout = $stdoutTask.Result
        $stderr = $stderrTask.Result

        if ($stdout) {
            Write-Host $stdout.TrimEnd()
        }
        if ($stderr) {
            Write-Host $stderr.TrimEnd()
        }
        if ($exitCode -ne 0) {
            throw "Diagnostic process $runIndex failed (exit code $exitCode)."
        }

        $lines = @($stdout -split '[\r\n]+')
        if (@($lines | Where-Object { $_ -eq "execution: NOT_ATTEMPTED" }).Count -ne 1) {
            throw "Run $runIndex did not emit exactly one execution: NOT_ATTEMPTED marker."
        }
        if (@($lines | Where-Object { $_ -eq "image_bytes: $ImageBytes" }).Count -ne 1) {
            throw "Run $runIndex did not report the requested image size."
        }
        if (@($lines | Where-Object { $_ -eq "iterations: $Iterations" }).Count -ne 1) {
            throw "Run $runIndex did not report the requested iteration count."
        }

        foreach ($phase in $expectedPhases) {
            $phaseLines = @($lines | Where-Object {
                $_ -match ("^{0}: iterations={1}, total_ms=[0-9.]+, ns_per_iteration=(?<ns>\d+)$" -f
                    [regex]::Escape($phase), $Iterations)
            })
            if ($phaseLines.Count -ne 1) {
                throw "Run $runIndex emitted $($phaseLines.Count) valid samples for '$phase'; expected exactly one."
            }
            if ($phaseLines[0] -match "ns_per_iteration=(\d+)$") {
                $samples[$phase].Add([long]$Matches[1])
            }
        }

        $process.Dispose()
        $process = $null
    }

    Write-Host ""
    Write-Host "=== REPEATED FRESH-PROCESS SUMMARY (NANOSECONDS / ITERATION) ==="
    foreach ($phase in $expectedPhases) {
        $values = [long[]]$samples[$phase].ToArray()
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

    Write-Host ""
    Write-Host "measurement=timed allocator initialization and isolated subsequent fill"
    Write-Host "not_measured=process-wide heap allocations, peak RSS, loader behavior, application startup"
    Write-Host "interpretation=isolated_second_clear is not an end-to-end speedup estimate"
    Write-Host "execution=NOT_ATTEMPTED"
}
finally {
    if ($null -ne $process) {
        $process.Dispose()
    }
    Pop-Location
}
