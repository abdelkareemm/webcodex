# Update the locally installed WebCodex binaries from verified release builds.
# Run this script from the WebCodex source folder AFTER closing WebCodex.
$ErrorActionPreference = 'Stop'
$root = Split-Path -Parent $MyInvocation.MyCommand.Path

$running = @(Get-Process -Name 'webcodex-native', 'webcodex-core', 'webcodex-tui' -ErrorAction SilentlyContinue)
if ($running.Count -gt 0) {
    throw 'Close all running WebCodex sessions before updating. No files were changed.'
}

$items = @(
    @{
        Source = Join-Path $root 'rust-core\target-ui-next\release\webcodex-native.exe'
        Destination = Join-Path $root 'rust-core\target\release\webcodex-native.exe'
    },
    @{
        Source = Join-Path $root 'rust-core\target-ui-next\release\webcodex-core.exe'
        Destination = Join-Path $root 'rust-core\target\release\webcodex-core.exe'
    },
    @{
        Source = Join-Path $root 'rust-tui\target-ui-next\release\webcodex-tui.exe'
        Destination = Join-Path $root 'rust-tui\target\release\webcodex-tui.exe'
    }
)

foreach ($item in $items) {
    if (-not (Test-Path -LiteralPath $item.Source -PathType Leaf)) {
        throw "Missing release executable: $($item.Source). Build the release components first."
    }
}

$stamp = Get-Date -Format 'yyyyMMdd-HHmmss'
$backup = Join-Path $root "backups\local-update-$stamp"
New-Item -ItemType Directory -Path $backup -Force | Out-Null

foreach ($item in $items) {
    $name = Split-Path $item.Destination -Leaf
    if (Test-Path -LiteralPath $item.Destination) {
        $folder = if ($name -eq 'webcodex-tui.exe') { 'tui' } else { 'core' }
        $backupFolder = Join-Path $backup $folder
        New-Item -ItemType Directory -Path $backupFolder -Force | Out-Null
        Copy-Item -LiteralPath $item.Destination -Destination (Join-Path $backupFolder $name) -Force
    }
}

foreach ($item in $items) {
    New-Item -ItemType Directory -Path (Split-Path $item.Destination -Parent) -Force | Out-Null
    Copy-Item -LiteralPath $item.Source -Destination $item.Destination -Force
}

Write-Host "Native binaries updated. Previous executables backed up to: $backup"
Write-Host 'Existing WebCodex sessions, configuration and browser profiles were not modified.'
Write-Host 'Now run: python -m pip install -e .'
Write-Host 'Then run: python -m webcodex --version'
