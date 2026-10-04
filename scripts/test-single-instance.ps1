param(
    [string]$Executable = (Join-Path $PSScriptRoot '../src-tauri/target/debug/examples/single_instance_probe.exe')
)

$ErrorActionPreference = 'Stop'
$probeExecutable = (Resolve-Path -LiteralPath $Executable).Path
$probeDirectory = Join-Path ([System.IO.Path]::GetTempPath()) ('aurora-instance-probe-' + [guid]::NewGuid())
$launchDirectory = Join-Path $probeDirectory 'launch Jørn'
$null = New-Item -ItemType Directory -Path $launchDirectory
$probeProcesses = [System.Collections.Generic.List[System.Diagnostics.Process]]::new()
$probeVariables = @('AURORA_INSTANCE_PROBE_ID', 'AURORA_INSTANCE_PROBE_OUTPUT', 'AURORA_INSTANCE_PROBE_MODE')
$previousEnvironment = @{}
foreach ($name in $probeVariables) { $previousEnvironment[$name] = [Environment]::GetEnvironmentVariable($name) }

function Read-ProbeRecords {
    if (Test-Path -LiteralPath $env:AURORA_INSTANCE_PROBE_OUTPUT) {
        try { Get-Content -LiteralPath $env:AURORA_INSTANCE_PROBE_OUTPUT -Encoding UTF8 | ForEach-Object { $_ | ConvertFrom-Json } }
        catch { return }
    }
}

function Wait-ProbeRecord([string]$Kind, [int]$Count) {
    $deadline = [DateTime]::UtcNow.AddSeconds(20)
    do {
        $records = @(Read-ProbeRecords | Where-Object kind -eq $Kind)
        if ($records.Count -ge $Count) { return $records[-1] }
        Start-Sleep -Milliseconds 50
    } while ([DateTime]::UtcNow -lt $deadline)
    throw "Timed out waiting for $Kind record $Count. Evidence: $probeDirectory"
}

function Start-Probe([string]$Argument) {
    $parameters = @{ FilePath = $probeExecutable; WorkingDirectory = $launchDirectory; WindowStyle = 'Hidden'; PassThru = $true }
    if ($Argument) { $parameters.ArgumentList = '"' + $Argument + '"' }
    $process = Start-Process @parameters
    $probeProcesses.Add($process)
    return $process
}

function Assert-Probe([bool]$Condition, [string]$Message) {
    if (-not $Condition) { throw "$Message. Evidence: $probeDirectory" }
}

function Wait-ProbeExit([System.Diagnostics.Process]$Process) {
    Assert-Probe ($Process.WaitForExit(20000)) "Probe $($Process.Id) did not exit"
    Assert-Probe ($Process.ExitCode -eq 0) "Probe $($Process.Id) exited with $($Process.ExitCode)"
}

try {
    foreach ($mode in @('hidden', 'minimized', 'maximized')) {
        $env:AURORA_INSTANCE_PROBE_ID = [guid]::NewGuid().ToString('N')
        $env:AURORA_INSTANCE_PROBE_OUTPUT = Join-Path $probeDirectory "$mode.jsonl"
        $env:AURORA_INSTANCE_PROBE_MODE = $mode
        $primary = Start-Probe ''
        $ready = Wait-ProbeRecord 'ready' 1
        Assert-Probe ($ready.pid -eq $primary.Id) 'Readiness belongs to the wrong process'
        if ($mode -eq 'hidden') { Assert-Probe (-not $ready.visible) 'Probe did not start hidden' }
        else { Assert-Probe $ready.minimized 'Probe did not start minimized' }

        $argument = 'C:\Music\Jørn Tillnes\Track 01.mp3'
        $secondary = Start-Probe $argument
        Wait-ProbeExit $secondary
        $launch = Wait-ProbeRecord 'launch' 1
        Assert-Probe ($launch.pid -eq $primary.Id) 'Launch was not delivered to the original process'
        Assert-Probe ($launch.visible -and -not $launch.minimized -and $launch.focused) 'Existing window was not restored and focused'
        if ($mode -eq 'maximized') { Assert-Probe $launch.maximized 'Maximized state was lost' }
        Assert-Probe ($launch.payload.args.Count -eq 2 -and $launch.payload.args[1] -ceq $argument) 'Spaces or Unicode in launch arguments were changed'
        Assert-Probe ($launch.payload.cwd -ceq $launchDirectory) 'Launching directory was changed'
        Assert-Probe (@(Read-ProbeRecords | Where-Object kind -eq 'setup').Count -eq 1) 'Duplicate process reached application setup'
        Assert-Probe (-not $primary.HasExited) 'Original process exited unexpectedly'

        $exitRequest = Start-Probe '--probe-exit'
        Wait-ProbeExit $exitRequest
        Wait-ProbeExit $primary
        $restarted = Start-Probe ''
        $null = Wait-ProbeRecord 'ready' 2
        Assert-Probe (@(Read-ProbeRecords | Where-Object kind -eq 'setup').Count -eq 2) 'Normal exit did not release the instance guard'

        # Only terminate processes started by this isolated harness.
        $restarted.Kill()
        $restarted.WaitForExit()
        $recovered = Start-Probe ''
        $null = Wait-ProbeRecord 'ready' 3
        Assert-Probe (@(Read-ProbeRecords | Where-Object kind -eq 'setup').Count -eq 3) 'Forced exit did not release the instance guard'
        $exitRequest = Start-Probe '--probe-exit'
        Wait-ProbeExit $exitRequest
        Wait-ProbeExit $recovered
        Write-Output "PASS: $mode activation, argument forwarding, duplicate setup prevention, normal restart, crash recovery"
    }
    $env:AURORA_INSTANCE_PROBE_ID = [guid]::NewGuid().ToString('N')
    $env:AURORA_INSTANCE_PROBE_OUTPUT = Join-Path $probeDirectory 'startup.jsonl'
    $env:AURORA_INSTANCE_PROBE_MODE = 'hidden'
    # Launch both without waiting for the primary's window or application setup.
    $first = Start-Probe ''
    $second = Start-Probe ''
    $ready = Wait-ProbeRecord 'ready' 1
    if ($ready.pid -eq $first.Id) { $primary = $first; $duplicate = $second }
    else { $primary = $second; $duplicate = $first }
    Wait-ProbeExit $duplicate
    Assert-Probe (@(Read-ProbeRecords | Where-Object kind -eq 'setup').Count -eq 1) 'Launches during startup reached duplicate application setup'
    $exitRequest = Start-Probe '--probe-exit'
    Wait-ProbeExit $exitRequest
    Wait-ProbeExit $primary
    Write-Output 'PASS: repeated launch during startup'
    Write-Output "Native single-instance evidence: $probeDirectory"
}
finally {
    foreach ($process in $probeProcesses) {
        if (-not $process.HasExited) { $process.Kill(); $process.WaitForExit() }
        $process.Dispose()
    }
    foreach ($name in $probeVariables) { [Environment]::SetEnvironmentVariable($name, $previousEnvironment[$name]) }
}
