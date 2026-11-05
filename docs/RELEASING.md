# Publishing WebCodex

WebING.md
<<<<<<< SEARCH
=======
# Publishing WebCodex

WebCodex uses semantic versions and GitHub Actions for WindowsCodex uses semantic versions and GitHub Actions for Windows releases.

## Before releasing

- Update the version releases.

## Before releasing

- Update the version consistently in both Rust manifests, consistently in both Rust manifests, the Python package, and version checks.
- Update CHANGELOG.md with the Python package, and version checks.
- Update CHANGELOG.md with user-visible user-visible changes.
- Run the complete test suite.
- Check that the working tree contains changes.
- Run the complete test suite.
- Check that the working tree contains no sensitive or generated files.
- Confirm the Windows installer no sensitive or generated files.
- Confirm the Windows installer and package layout.

## Validate locally and package layout.

## Validate locally

From the repository root:

    powershell

From the repository root:

    powershell.exe -NoProfile -ExecutionPolicy Bypass -File ".\.exe -NoProfile -ExecutionPolicy Bypass -File ".\verify-native.ps1"

## Publish a release

Commit andverify-native.ps1"

## Publish a release

Commit and push the release-ready source before creating push the release-ready source before creating its tag.

For WebCod its tag.

For WebCodex v1.2.0, the tagex v1.2.0, the tag is:

    v1.2.0

The GitHub Actions workflow `. is:

    v1.2.0

The GitHub Actions workflow `.github/workflows/release.yml` runs whengithub/workflows/release.yml` runs when a matching version tag is pushed.

It will:

 a matching version tag is pushed.

It will:

1. Verify the tag matches the1. Verify the tag matches the package version.
2. Run Python and Rust tests on Windows package version.
2. Run Python and Rust tests on Windows.
3. Build Rust release executables.
4. Create the Windows.
3. Build Rust release executables.
4. Create the Windows release archive.
5. Generate SHA256SUM release archive.
5. Generate SHA256SUMS.txt.
6. Publish a GitHub Release with theS.txt.
6. Publish a GitHub Release with the downloadable archive and checksums.

If compilation or testing fails, the release publishing job downloadable archive and checksums.

If compilation or testing fails, the release publishing job will not run.

The workflow also supports manual artifact will not run.

The workflow also supports manual artifact builds using builds using `workflow_dispatch`. Manual runs do not automatically `workflow_dispatch`. Manual runs do not automatically publish a GitHub Release.

## Validate the publish a GitHub Release.

## Validate the published release published release

Download the Windows archive from Git

Download the Windows archive from GitHub Releases and extractHub Releases and extract it into a fresh directory.

Run install.ps1 and verify that it into a fresh directory.

Run install.ps1 and verify that:

- The native executable reports the expected version.
- The `:

- The native executable reports the expected version.
- The `webwebcodex` command works in acodex` command works in a new PowerShell session.
- ChatGPT can be opened new PowerShell session.
- ChatGPT can be opened using the Playwright using the Playwright browser.
- A message can be sent and answered.
 browser.
- A message can be sent and answered.
- An example project can be edited.
-- An example project can be edited.
- Existing sessions can be resumed.

Do not claim a release is fully verified until the Existing sessions can be resumed.

Do not claim a release is fully verified until the fresh-install test passes.

## Distribution security

Do not include fresh-install test passes.

## Distribution security

Do not include browser profiles, session databases, secrets, cookies, personal files, or preauthenticated browser profiles, session databases, secrets, cookies, personal files, or preauthenticated accounts.

 accounts.

Do not modify or replace a published release tag silently.Do not modify or replace a published release tag silently. If an already-published If an already-published version needs a fix, version needs a fix, issue a new patch version.

WebCod issue a new patch version.

WebCodex is created by abdelkareemm. Preserveex is created by abdelkareemm. Preserve any required third-party copyright and license notices.
=======
