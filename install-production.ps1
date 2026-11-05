param(
    [int]$WaitSeconds = 300
)

$ErrorActionPreference = 'Stop'
$root = Split-Path -Parent $MyInvocation.MyCommand.Path

$files = @(
    @{
        Source = Join-Path $root 'rust-core\target-staging\release\webcodex-native.exe'
        Destination = Join-Path $root 'rust-core\target\release\webcodex-native.exe'
    },
    @{
        Source = Join-Path $root 'rust-core\target-staging\release\webcodex-core.exe'
        Destination = Join-Path $root 'rust-core\target\release\webcodex-core.exe'
    },
    @{
        Source = Join-Path $root 'rust-tui\target-staging\release\webcodex-tui.exe'
        Destination = Join-Path $root 'rust-tui\target\release\webcodex-tui.exe'
    }
)

foreach ($file in $files) {
    if (-not (Test-Path -LiteralPath $file.Source -PathType Leaf)) {
        throw "Missing compiled release executable: $($file.Source)"
    }
}

$deadline = (Get-Date).AddSeconds($WaitSeconds)
Write-Host 'Waiting for running WebCodex processes to exit...'

while ($true) {
    $running = @(
        Get-CimInstance Win32_Process |
            Where-Object {
                $_.Name -in @(
                    'webcodex-native.exe',
                    'webcodex-tui.exe',
                    'webcodex-core.exe'
                )
            }
    )

    if ($running.Count -eq 0) {
        break
    }

    if ((Get-Date) -ge $deadline) {
        throw 'Timed out waiting for WebCodex to close. Production files were not replaced.'
    }

    Start-Sleep -Seconds 2
}

foreach ($file in $files) {
    $directory = Split-Path -Parent $file.Destination
    New-Item -ItemType Directory -Force -Path $directory | Out-Null
}

foreach ($file in $files) {
    Copy-Item -LiteralPath $file.Source -Destination $file.Destination -Force

    $sourceHash = (Get-FileHash -LiteralPath $file.Source -Algorithm SHA256).Hash
    $installedHash = (Get-FileHash -LiteralPath $file.Destination -Algorithm SHA256).Hash

    if ($sourceHash -ne $installedHash) {
        throw "Binary verification failed: $($file.Destination)"
    }

    Write-Host "INSTALLED: $($file.Destination)"
}

& $files[0].Destination --version

if ($LASTEXITCODE -ne 0) {
    throw 'Production native executable failed startup verification.'
}

Write-Host 'DONE: Production WebCodex executables installed and verified.'
