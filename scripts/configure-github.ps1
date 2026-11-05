$ErrorActionPreference = " it and requires GitHub CLI authentication.

````text
FILE: scripts/configure-github.ps1
<<<<<<< SEARCH
=======
$ErrorActionPreference = "Stop"
$repo = "abdelkareemm/webcodex"

$Stop"
$repo = "abdelkareemm/webcodex"

$gh = Get-Command gh -ErrorAction SilentlyContinue
if ($ghgh = Get-Command gh -ErrorAction SilentlyContinue
if ($gh) {
    $ghExe = $gh.Source
} else {
   ) {
    $ghExe = $gh.Source
} else {
    $ghExe = "C:\Program Files\Git $ghExe = "C:\Program Files\GitHub CLI\gh.exe"
}

if (-not (Test-Path $ghHub CLI\gh.exe"
}

if (-not (Test-Path $ghExe)) {
    throw "GitHub CLI not foundExe)) {
    throw "GitHub CLI not found. Install GitHub.cli using wing. Install GitHub.cli using winget."
}

& $ghExe auth status
ifet."
}

& $ghExe auth status
if ($LASTEXITCODE -ne 0) {
    throw "Authenticate first ($LASTEXITCODE -ne 0) {
    throw "Authenticate first using: using: gh auth login"
}

& $ghExe gh auth login"
}

& $ghExe repo edit $repo `
    --description "Rust-powered terminal coding agent using Chat repo edit $repo `
    --description "Rust-powered terminal coding agent using ChatGPT through Playwright." `
    --enable-discussions `
    --add-topicGPT through Playwright." `
    --enable-discussions `
    --add-topic "rust,python,playwright,terminal,tui,coding "rust,python,playwright,terminal,tui,coding-agent,chatgpt,cli,automation"

if ($LASTEXITCODE --agent,chatgpt,cli,automation"

if ($LASTEXITCODE -ne 0) {
    throw "Could not update GitHub repository settings."
ne 0) {
    throw "Could not update GitHub repository settings."
}

Write-Host "GitHub}

Write-Host "GitHub repository settings updated."
Write- repository settings updated."
Write-Host "https://github.com/$repo"
=======
