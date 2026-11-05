$ErrorActionPreference = 'Stop'
$root = Split-Path -Parent (Split-Path -Parent $MyInvocation.MyCommand.Path)

$expected = @{
    'webcodex-native' = Join-Path $root 'rust-core\target-staging\release\webcodex-native.exe'
    'webcodex-core' = Join-Path $root 'rust-core\target-staging\release\webcodex-core.exe'
    'webcodex-tui' = Join-Path $root 'rust-tui\target-validation\release\webcodex-tui.exe'
}

$stagedFound = @{
    'webcodex-native' = $false
    'webcodex-core' = $false
    'webcodex-tui' = $false
}

foreach ($name in @('webcodex-native','webcodex-core','webcodex-tui')) {
    $processes = @(Get-CimInstance Win32_Process -Filter "Name = '$name.exe'")

    if ($processes.Count -eq 0) {
        Write-Host "NOT RUNNING: $name"
        continue
    }

    foreach ($process in $processes) {
        Write-Host "  Parent PID: $($process.ParentProcessId)"
        Write-Host "  Command: $($process.CommandLine)"
        $actual = $process.ExecutablePath
        $match = $actual -and [string]::Equals(
            $actual,
            $expected[$name],
            [System.StringComparison]::OrdinalIgnoreCase
        )

        if ($match) {
            $stagedFound[$name] = $true
        }

        $status = if ($match) { 'STAGED' } else { 'OTHER' }
        Write-Host "$status PID=$($process.ProcessId) $name"
        Write-Host "  Path: $actual"
    }
}

if (@($stagedFound.Values | Where-Object { -not $_ }).Count -gt 0) {
    Write-Host 'RESULT: One or more staged components were not detected.'
    exit 1
}

Write-Host 'PASS: All three staged Rust processes are running.'
