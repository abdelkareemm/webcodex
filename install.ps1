# WebCodex Windows installer for SEARCH
=======
# WebCodex Windows installer for precompiled GitHub precompiled GitHub releases.
# Rust/Cargo is not required for this installation path releases.
# Rust/Cargo is not required for this installation path.
$ErrorActionPreference = "Stop"

$root = Split-.
$ErrorActionPreference = "Stop"

$root = Split-Path -Parent $MyInvocation.MyCommand.Path
$native = Join-Path -Parent $MyInvocation.MyCommand.Path
$native = Join-Path $root "rust-core\target\release\webcodex-native.exe"
Path $root "rust-core\target\release\webcodex-native.exe"
$tui = Join-Path $root "rust-tui\target\release$tui = Join-Path $root "rust-tui\target\release\webcodex-tui.exe"
$worker\webcodex-tui.exe"
$worker = Join-Path $root "rust-core\target\release\webcodex = Join-Path $root "rust-core\target\release\webcodex-core.exe"

Write-Host ""
Write-Host "WebCod-core.exe"

Write-Host ""
Write-Host "WebCodex Windows Installer"
Write-Host "-------------------------ex Windows Installer"
Write-Host "-------------------------"

foreach ($file in"

foreach ($file in @($native, $tui, $worker)) {
    if (-not @($native, $tui, $worker)) {
    if (-not (Test-Path $file)) {
        throw "Missing compiled executable: $ (Test-Path $file)) {
        throw "Missing compiled executable: $file. Download the Windows release ZIP, not the source archive."
    }
}

if (-not (Get-Command python -Errorfile. Download the Windows release ZIP, not the source archive."
    }
}

if (-not (Get-Command python -ErrorAction SilentlyContinue)) {
    throw "Python 3.9Action SilentlyContinue)) {
    throw "Python 3.9+ is required. Install Python, reopen PowerShell and rerun this installer."
}

& python+ is required. Install Python, reopen PowerShell and rerun this installer."
}

& python -c "import sys; sys.exit(0 if sys.version_info >= -c "import sys; sys.exit(0 if sys.version_info >= (3, 9) else 1)"
if ($LASTEXITCODE - (3, 9) else 1)"
if ($LASTEXITCODE -ne 0) {
    throw "WebCodne 0) {
    throw "WebCodex requires Python 3.9 or newer."
}

Write-Host "Installingex requires Python 3.9 or newer."
}

Write-Host "Installing WebCodex Python sidecar..."
& python -m pip install - WebCodex Python sidecar..."
& python -m pip install -e $root
if ($LASTEXITCODE -ne 0) {
   e $root
if ($LASTEXITCODE -ne 0) {
    throw "Python package installation failed."
}

Write-Host "Installing Playwright Chromium throw "Python package installation failed."
}

Write-Host "Installing Playwright Chromium..."
& python -m playwright install chromium
if ($LASTEXITCODE -ne..."
& python -m playwright install chromium
if ($LASTEXITCODE -ne 0) {
    throw "Chromium 0) {
    throw "Chromium installation failed."
}

Write- installation failed."
}

Write-Host "Checking native executable..."
& $native --version
Host "Checking native executable..."
& $native --version
if ($LASTEXITCODE -ne 0) {
    throw "WebCodif ($LASTEXITCODE -ne 0) {
    throw "WebCodex native executable failed verification."
}

$binex native executable failed verification."
}

$bin = Join-Path $env:LOCALAPPDATA "WebCodex\bin = Join-Path $env:LOCALAPPDATA "WebCodex\bin"
New-Item -ItemType Directory -Force -Path $bin | Out"
New-Item -ItemType Directory -Force -Path $bin | Out-Null
Copy-Item (Join-Path $root "webcodex-Null
Copy-Item (Join-Path $root "webcodex.cmd") (Join-Path $bin "webcodex.cmd") -Force.cmd") (Join-Path $bin "webcodex.cmd") -Force

$userPath = [Environment]::GetEnvironmentVariable("Path", "User

$userPath = [Environment]::GetEnvironmentVariable("Path", "User")
$entries = @($userPath -split ";" |")
$entries = @($userPath -split ";" | Where-Object { $_ })
if (-not Where-Object { $_ })
if (-not ($entries | Where-Object { $_.TrimEnd("\") ($entries | Where-Object { $_.TrimEnd("\") -ieq $bin.TrimEnd("\") -ieq $bin.TrimEnd("\") })) {
    [Environment]::SetEnvironmentVariable })) {
    [Environment]::SetEnvironmentVariable(
        "Path",
       (
        "Path",
        (($entries + (($entries + $bin) -join ";"),
        "User"
    )
}

Write- $bin) -join ";"),
        "User"
    )
}

Write-Host ""
Write-Host "WebCodex installation complete."
Write-Host "Host ""
Write-Host "WebCodex installation complete."
Write-Host "Keep the extracted WebCodex directory in its currentKeep the extracted WebCodex directory in its current location."
Write-Host "Open a NEW Power location."
Write-Host "Open a NEW PowerShell window, navigate to your project and run:"
WriteShell window, navigate to your project and run:"
Write-Host "  webcodex"
Write-Host ""
Write-Host "  webcodex"
Write-Host ""
Write-Host "To resume a chat, run: webcodex-Host "To resume a chat, run: webcodex resume"
Write-Host "On first launch, sign in resume"
Write-Host "On first launch, sign in to ChatGPT in the browser if prompted."
=======
