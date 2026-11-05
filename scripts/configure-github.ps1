$Error GitHub authentication succeeds.

````text
FILE: scripts/configure-github.ps1
<<<<<<< SEARCH
=======
$ErrorActionPreference = "Stop"
$repo = "abdelkareActionPreference = "Stop"
$repo = "abdelkareemm/webcodex"

$ghCommand = Get-Command gh -ErrorActionemm/webcodex"

$ghCommand = Get-Command gh -ErrorAction SilentlyContinue
if ($ghCommand) {
    $ghExe = $ SilentlyContinue
if ($ghCommand) {
    $ghExe = $ghCommand.Source
} else {
   ghCommand.Source
} else {
    $ghExe = "C:\Program Files\GitHub CLI\gh.exe"
}

if (-not (Test-Path -LiteralPath $ghExe)) {
    throw "GitHub CLI $ghExe = "C:\Program Files\GitHub CLI\gh.exe"
}

if (-not (Test-Path -LiteralPath $ghExe)) {
    throw "GitHub CLI is not installed. Install it using winget."
}

& $ghExe auth is not installed. Install it using winget."
}

& $ghExe auth status
if ($LASTEXITCODE -ne 0) {
    throw " status
if ($LASTEXITCODE -ne 0) {
    throw "GitHub authentication is required. Run gh auth login in PowerGitHub authentication is required. Run gh auth login in PowerShell."
}

& $ghExe repo edit $repo --Shell."
}

& $ghExe repo edit $repo --description "Rust-powered terminal coding agent using ChatGPT through Playwright." --enabledescription "Rust-powered terminal coding agent using ChatGPT through Playwright." --enable-discussions
if ($LASTEXITCODE -ne -discussions
if ($LASTEXITCODE -ne 0) {
    throw "Could not update repository description0) {
    throw "Could not update repository description or Discussions settings."
}

& $ghExe repo edit or Discussions settings."
}

& $ghExe repo edit $repo --add-topic "rust,python,playwright,terminal,t $repo --add-topic "rust,python,playwright,terminal,tui,coding-agent,chatgpt,cli,automation"
if ($LASTui,coding-agent,chatgpt,cli,automation"
if ($LASTEXITCODE -ne 0) {
    throw "Could not update repository topicsEXITCODE -ne 0) {
    throw "Could not update repository topics."
}

Write-Host "Repository configuration completed."
}

Write-Host "Repository configuration completed: https://github.com/$repo"
