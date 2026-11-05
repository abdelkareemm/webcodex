$ErrorActionPreference = 'Stop'
$root = Split-Path -Parent $MyInvocation.MyCommand.Path

$pairs = @(
    @('rust-core\target-staging\release\webcodex-native.exe', 'rust-core\target\release\webcodex-native.exe'),
    @('rust-core\target-staging\release\webcodex-core.exe', 'rust-core\target\release\webcodex-core.exe'),
    @('rust-tui\target-staging\release\webcodex-tui.exe', 'rust-tui\target\release\webcodex-tui.exe')
)

$allMatch = $true

foreach ($pair in $pairs) {
    $source = Join-Path $root $pair[0]
    $installed = Join-Path $root $pair[1]

    if (-not (Test-Path -LiteralPath $source -PathType Leaf)) {
        Write-Host "MISSING BUILD: $source"
        $allMatch = $false
        continue
    }

    if (-not (Test-Path -LiteralPath $installed -PathType Leaf)) {
        Write-Host "MISSING INSTALLED: $installed"
        $allMatch = $false
        continue
    }

    $sourceHash = (Get-FileHash -LiteralPath $source -Algorithm SHA256).Hash
    $installedHash = (Get-FileHash -LiteralPath $installed -Algorithm SHA256).Hash

    if ($sourceHash -eq $installedHash) {
        Write-Host "MATCH: $installed"
    } else {
        Write-Host "OUTDATED OR DIFFERENT: $installed"
        $allMatch = $false
    }
}

$running = @(Get-CimInstance Win32_Process | Where-Object {
    $_.Name -in @('webcodex-native.exe', 'webcodex-tui.exe', 'webcodex-core.exe')
})

foreach ($process in $running) {
    Write-Host "RUNNING: $($process.Name) PID=$($process.ProcessId)"
}

if ($allMatch) {
    Write-Host 'DONE: All production executables match the verified release builds.'
} else {
    Write-Host 'BLOCKED: Production executables are missing or differ from the verified builds.'
}
