$ErrorActionPreference = 'Stop'
$root = Split-Path -Parent $MyInvocation.MyCommand.Path
$installer = Join-Path $root 'install-production.ps1'

if (-not (Test-Path -LiteralPath $installer -PathType Leaf)) {
    throw "Production installer not found: $installer"
}

$arguments = @(
    '-NoProfile'
    '-ExecutionPolicy'
    'Bypass'
    '-File'
    "`"$installer`""
    '-WaitSeconds'
    '3600'
)

$process = Start-Process `
    -FilePath 'powershell.exe' `
    -ArgumentList $arguments `
    -WorkingDirectory $root `
    -PassThru

Write-Host "Production installer started in a separate process (PID $($process.Id))."
Write-Host 'Exit this WebCodex session with /quit to unlock the production executables.'
Write-Host 'The installer will then install and verify the production binaries.'
