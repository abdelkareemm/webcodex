# Build both native components. Run from any directory with PowerShell.
$ErrorActionPreference = 'Stop'
$root = Split-Path -Parent $MyInvocation.MyCommand.Path
if (-not (Get-Command cargo -ErrorAction SilentlyContinue)) {
    throw 'Rust/Cargo not installed. Install from https://rustup.rs/ and reopen PowerShell.'
}
& cargo test --manifest-path (Join-Path $root 'rust-core\Cargo.toml')
if ($LASTEXITCODE -ne 0) { throw 'Rust core unit tests failed.' }
& cargo build --release --manifest-path (Join-Path $root 'rust-core\Cargo.toml')
if ($LASTEXITCODE -ne 0) { throw 'Rust core build failed.' }
& (Join-Path $root 'rust-core\target\release\webcodex-native.exe') --version
if ($LASTEXITCODE -ne 0) { throw 'Rust agent startup verification failed.' }
& cargo test --manifest-path (Join-Path $root 'rust-tui\Cargo.toml')
if ($LASTEXITCODE -ne 0) { throw 'Rust TUI unit tests failed.' }
& cargo build --release --manifest-path (Join-Path $root 'rust-tui\Cargo.toml')
if ($LASTEXITCODE -ne 0) { throw 'Rust TUI build failed.' }
Write-Host 'WebCodex Rust core and terminal UI built successfully.'
