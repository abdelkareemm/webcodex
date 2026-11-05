param(
    [string]$Project = "",
    [switch]$Help,
    [switch]$SelfTest
)

$ErrorActionPreference = 'Stop'
$root = Split-Path -Parent $MyInvocation.MyCommand.Path
$native = Join-Path $root 'rust-core\target-staging\release\webcodex-native.exe'
$tui = Join-Path $root 'rust-tui\target-staging\release\webcodex-tui.exe'
$core = Join-Path $root 'rust-core\target-staging\release\webcodex-core.exe'

if ($Help) {
    Write-Host 'Usage: .\launch-staged.ps1 [-Project <folder>]'
    Write-Host 'Without -Project, uses a disposable test project and isolated session database.'
    Write-Host 'The browser uses a temporary profile.'
    Write-Host 'Use -SelfTest to verify the isolated native runtime without opening a browser.'
    exit 0
}

foreach ($file in @($native, $tui, $core)) {
    if (-not (Test-Path -LiteralPath $file -PathType Leaf)) {
        throw "Missing staged executable: $file"
    }
}

$smokeRoot = Join-Path $env:TEMP 'WebCodex-Staged-Smoke'
$smokeHome = Join-Path $smokeRoot 'home'

if ([string]::IsNullOrWhiteSpace($Project)) {
    $Project = Join-Path $smokeRoot 'project'
    New-Item -ItemType Directory -Path $Project -Force | Out-Null
}

New-Item -ItemType Directory -Path $smokeHome -Force | Out-Null

$resolvedProject = (Resolve-Path -LiteralPath $Project).Path
$previousCore = $env:WEBCODEX_RUST_CORE
$previousTui = $env:WEBCODEX_RUST_TUI
$previousHome = $env:WEBCODEX_HOME

try {
    $env:WEBCODEX_RUST_CORE = $core
    $env:WEBCODEX_RUST_TUI = $tui
    $env:WEBCODEX_HOME = $smokeHome
    Write-Host "Using TUI: $tui"
    if ($SelfTest) {
        & $native -C $resolvedProject --list-threads
        if ($LASTEXITCODE -eq 0) {
            Write-Host 'PASS: staged native executable and isolated Rust worker started successfully.'
        }
    }
    else {
        & $native -C $resolvedProject -i --temporary
    }
    if ($LASTEXITCODE -ne 0) {
        throw "Staged WebCodex exited with code $LASTEXITCODE"
    }
}
finally {
    $env:WEBCODEX_RUST_CORE = $previousCore
    $env:WEBCODEX_RUST_TUI = $previousTui
    $env:WEBCODEX_HOME = $previousHome
}
