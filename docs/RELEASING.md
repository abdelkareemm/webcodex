# Publishing Web two documentation files

````text
FILE: docs/RELEASING.md
<<<<<<< SEARCH
=======
# Publishing WebCodex

WebCodex uses semantic versioning, Git tags, and GitCodex

WebCodex uses semantic versioning, Git tags, and GitHub Actions.

## Before tagging

Hub Actions.

## Before tagging

1. Update all application version strings1. Update all application version strings and release tests.
2. Update and release tests.
2. Update CHANGELOG.md.
3. Run `verify-native.ps1` on Windows.
 CHANGELOG.md.
3. Run `verify-native.ps1` on Windows.
4. Review changes for secrets and generated files.
5. Commit and4. Review changes for secrets and generated files.
5. Commit and push the release-ready source.
 push the release-ready source.
6. Check that GitHub Actions tests6. Check that GitHub Actions tests are passing.

## Build Windows are passing.

## Build Windows artifacts without publishing

Run the ** artifacts without publishing

Run the **Publish WebCodex** workflow manually using the GitHub Actions tab.

ThePublish WebCodex** workflow manually using the GitHub Actions tab.

The manual run creates a downloadable workflow artifact but manual run creates a downloadable workflow artifact but does not create a GitHub Release.

Use does not create a GitHub Release.

Use it to verify the ZIP and it to verify the ZIP and installer before publishing a versioned tag.

## Publish a new version

Create a installer before publishing a versioned tag.

## Publish a new version

Create a new version new version tag pointing to the verified release commit, then push the tag.

For example, tag pointing to the verified release commit, then push the tag.

For example, after updating every version reference to  after updating every version reference to 1.2.1:

    git tag v1.2.1
1.2.1:

    git tag v1.2.1
    git push origin v1.2.1

The release workflow tests the source, builds Windows    git push origin v1.2.1

The release workflow tests the source, builds Windows executables, packages a ZIP, generates executables, packages a ZIP, generates SHA256SUMS.txt, and publishes the GitHub Release.

Do not reuse SHA256SUMS.txt, and publishes the GitHub Release.

Do not reuse or force-update an existing published tag.

## Validate the or force-update an existing published tag.

## Validate the release

Download the ZIP from GitHub Releases into a clean directory.

Run release

Download the ZIP from GitHub Releases into a clean directory.

Run `install.ps1` and confirm that:

- The Rust `install.ps1` and confirm that:

- The Rust agent reports the expected version.
- The `webcodex` command works in agent reports the expected version.
- The `webcodex` command works in a new terminal.
- The browser starts and can access a new terminal.
- The browser starts and can access ChatGPT.
- Messages can be sent and received.
- File editing ChatGPT.
- Messages can be sent and received.
- File editing and session resume and session resume work in a disposable test work in a disposable test project.

The compiled Windows ZIP requires Python and project.

The compiled Windows ZIP requires Python and Playwright, but not Rust or Cargo.

Do not publish browser profiles, Chat Playwright, but not Rust or Cargo.

Do not publish browser profiles, ChatGPT cookies, private conversations, local session databases, or project credentialsGPT cookies, private conversations, local session databases, or project credentials.

WebCodex is created by abdelkareemm; third-party license.

WebCodex is created by abdelkareemm; third-party license requirements must be preserved.
