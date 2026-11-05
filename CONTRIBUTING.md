# Contributing to WebCodex

Thanks for your interest in improving WebCodex.

## Architecture

WebCodex uses a Rust-native agent and terminal UI, with a Python Playwright sidecar for browser interaction.

- `rust-core/` — CLI, agent orchestration, project scanning, file operations, and session management.
- `rust-tui/` — terminal interface.
- `webcodex/` — Python browser transport and launcher.
- `tests/` — Python integration and contract tests.

## Development setup

Install a stable Rust toolchain and Python 3.9 or newer.

```powershell
python -m pip install -e . pytest
python -m playwright install chromium
```

Build and test both Rust components:

```powershell
powershell.exe -NoProfile -ExecutionPolicy Bypass -File .\verify-native.ps1
```

On other platforms, run:

```sh
cargo test --manifest-path rust-core/Cargo.toml
cargo test --manifest-path rust-tui/Cargo.toml
python -m pytest -q
```

## Pull requests

Keep changes focused and explain the problem being addressed.

For behavior changes:

1. Add or update tests.
2. Preserve existing session and browser-profile compatibility where possible.
3. Avoid adding unnecessary runtime dependencies.
4. Verify that terminal rendering and exit behavior remain reliable.
5. Document user-visible changes.

Rust code should follow `cargo fmt`. Prefer descriptive error handling and explicit protocol contracts between Rust and Python.

## Security

Do not commit browser sessions, credentials, ChatGPT cookies, private conversations, or generated project data.

Treat AI-generated shell commands and file edits as untrusted input. Permission settings must not silently escalate.

Never include sensitive information in bug reports, screenshots, or terminal logs.

See `SECURITY.md` for vulnerability reporting guidance.
