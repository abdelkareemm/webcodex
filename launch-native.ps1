param([string]$Project=".",[switch]$SelfTest)
$ErrorActionPreference="Stop"
$root=Split-Path -Parent $MyInvocation.MyCommand.Path
$native=Join-Path $root "rust-core\target-staging\release\webcodex-native.exe"
$core=Join-Path $root "rust-core\target-staging\release\webcodex-core.exe"
$tui=Join-Path $root "rust-tui\target-staging\release\webcodex-tui.exe"
foreach($file in @($native,$core,$tui)){if(!(Test-Path -LiteralPath $file)){throw "Missing executable: $file"}}
$manifest=Join-Path $root "rust-core\Cargo.toml"
$expected=([regex]::Match((Get-Content -LiteralPath $manifest -Raw),'(?m)^version\s*=\s*"([^"]+)"')).Groups[1].Value
$actual=(& $native --version) -join ""
if(!$expected -or $actual -ne "webcodex $expected (Rust)"){throw "Staged native version mismatch: expected $expected, got $actual. Run .\build-rust.ps1."}
$oldCore=$env:WEBCODEX_RUST_CORE
$oldTui=$env:WEBCODEX_RUST_TUI
try {
    $env:WEBCODEX_RUST_CORE=$core
    $env:WEBCODEX_RUST_TUI=$tui
    if($SelfTest){& $native --version}else{& $native -C $Project -i}
    if($LASTEXITCODE -ne 0){throw "Native WebCodex exited with code $LASTEXITCODE"}
} finally {
    $env:WEBCODEX_RUST_CORE=$oldCore
    $env:WEBCODEX_RUST_TUI=$oldTui
}
