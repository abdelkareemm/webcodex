$ErrorActionPreference = 'Stop'
$root = Split-Path -Parent $MyInvocation.MyCommand.Path
& (Join-Path $root 'build-rust.ps1')
if ($LASTEXITCODE -ne 0) { throw 'Native build failed' }
& python -m unittest discover -s (Join-Path $root 'tests') -v
if ($LASTEXITCODE -ne 0) { throw 'Python browser-sidecar tests failed' }
& (Join-Path $root 'rust-core\target\release\webcodex-native.exe') --version
if ($LASTEXITCODE -ne 0) { throw 'Rust CLI smoke test failed' }
Write-Host 'WebCodex native smoke checks completed.'
