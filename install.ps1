, and registers a command without requiring Rust or Cargo.

````text
FILE: install.ps1
<<<<<<< SEARCH
=======
$ErrorActionPreference = "Stop"

$root = Split-Path -Parent$ErrorActionPreference = "Stop"

$root = Split-Path -Parent $MyInvocation.MyCommand.Path
$native = Join $MyInvocation.MyCommand.Path
$native = Join-Path $root "rust-core\target\release\webcodex-native.exe-Path $root "rust-core\target\release\webcodex-native.exe"
$core = Join-Path $root "rust"
$core = Join-Path $root "rust-core\target\release\webcodex-core.exe"
$tui = Join--core\target\release\webcodex-core.exe"
$tui = Join-Path $root "rust-tui\target\release\webcodex-tuiPath $root "rust-tui\target\release\webcodex-tui.exe"

Write-Host "WebCodex Windows Installer"

.exe"

Write-Host "WebCodex Windows Installer"

foreach ($file in @($native, $core, $tui))foreach ($file in @($native, $core, $tui)) {
    if (-not (Test-Path -LiteralPath $file)) {
    if (-not (Test-Path -LiteralPath $file)) {
        throw "Missing binary {
        throw "Missing binary: $file. Download the compiled Windows release ZIP instead: $file. Download the compiled Windows release ZIP instead of the source archive."
    }
}

if (-not (Get-Command python of the source archive."
    }
}

if (-not (Get-Command python -ErrorAction SilentlyContinue)) {
    throw "Python is -ErrorAction SilentlyContinue)) {
    throw "Python is not available. Install Python 3.9 not available. Install Python 3.9 or newer and reopen PowerShell."
}

& or newer and reopen PowerShell."
}

& python -c "import sys; sys.exit(0 if sys.version_info >= python -c "import sys; sys.exit(0 if sys.version_info >= (3, 9) else 1)"
if ($LASTEXITCODE - (3, 9) else 1)"
if ($LASTEXITCODE -ne 0) {
    throw "Python 3.9 or newer isne 0) {
    throw "Python 3.9 or newer is required."
}

Write-Host "Installing Python browser required."
}

Write-Host "Installing Python browser-sidecar dependencies..."
& python -m pip-sidecar dependencies..."
& python -m pip install -e $root
if ($LASTEXITCODE -ne 0) install -e $root
if ($LASTEXITCODE -ne 0) {
    throw "Python dependency installation failed."
}

Write-Host {
    throw "Python dependency installation failed."
}

Write-Host "Installing Chromium for Playwright..."
& python -m "Installing Chromium for Playwright..."
& python -m playwright install chromium
if ($LASTEXITCODE -ne 0) {
    playwright install chromium
if ($LASTEXITCODE -ne 0) {
    throw "Playwright Chromium installation failed."
}

Write-Host " throw "Playwright Chromium installation failed."
}

Write-Host "Checking the native application..."
& $native --version
if ($LASTEXITCODEChecking the native application..."
& $native --version
if ($LASTEXITCODE -ne 0) {
    throw "Native executable verification failed."
}

$ -ne 0) {
    throw "Native executable verification failed."
}

$bin = Join-Path $env:LOCALAPPDATA "WebCodex\bin = Join-Path $env:LOCALAPPDATA "WebCodex\bin"
New-Item -ItemType Directory -Force -Path $bin |bin"
New-Item -ItemType Directory -Force -Path $bin | Out-Null

$launcher = Join-Path $bin "webcodex Out-Null

$launcher = Join-Path $bin "webcodex.cmd"
$python = (& python -c "import sys.cmd"
$python = (& python -c "import sys; print(sys.executable)").Trim()
if (-not $python); print(sys.executable)").Trim()
if (-not $python) {
    throw "Could not determine Python executable."
}

$script = '@ {
    throw "Could not determine Python executable."
}

$script = '@echo off' + "`r`n" +
   echo off' + "`r`n" +
    '"' + $python + '" -m web '"' + $python + '" -m webcodex %*' + "`r`n" +
    'exit /bcodex %*' + "`r`n" +
    'exit /b %errorlevel%' + "`r`n"
 %errorlevel%' + "`r`n"
[System.IO.File]::WriteAllText($launcher, $script, [[System.IO.File]::WriteAllText($launcher, $script, [System.Text.Encoding]::ASCIISystem.Text.Encoding]::ASCII)

$userPath = [Environment]::GetEnvironmentVariable("Path", "User)

$userPath = [Environment]::GetEnvironmentVariable("Path", "User")
if (-not $userPath) { $userPath = "" }
$entries")
if (-not $userPath) { $userPath = "" }
$entries = @($userPath -split ";" | Where-Object { $_ })
$existing = @($userPath -split ";" | Where-Object { $_ })
$existing = @($entries | Where-Object { $_.TrimEnd("\") = @($entries | Where-Object { $_.TrimEnd("\") -ieq $bin.TrimEnd("\") })

if ($existing.Count -eq 0) {
    [Environment -ieq $bin.TrimEnd("\") })

if ($existing.Count -eq 0) {
    [Environment]::SetEnvironmentVariable(
        "Path]::SetEnvironmentVariable(
        "Path",
        (($entries + $bin) -join ";",
        (($entries + $bin) -join ";"),
        "User"
    )
}

Write"),
        "User"
    )
}

Write-Host ""
Write-Host "WebCodex installation completed."
Write-Host-Host ""
Write-Host "WebCodex installation completed."
Write-Host "Keep the extracted WebCodex folder at "Keep the extracted WebCodex folder at: $root"
Write-Host "Open: $root"
Write-Host "Open a NEW PowerShell session and a NEW PowerShell session and run webcodex from your project directory."
Write-Host "Sign in to run webcodex from your project directory."
Write-Host "Sign in to ChatGPT in the visible browser if prompted."
