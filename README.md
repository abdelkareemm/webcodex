# WebCodex

**A Rust-powered coding agent that runs through ChatGPT.**

WebCodex brings a Codex-style terminal coding experience to your existing ChatGPT account. It uses Rust for the terminal interface and agent operations, with Python and Playwright handling browser communication.

Built by **[abdelkareemm](https://github.com/abdelkareemm)**.

[GitHub Repository](https://github.com/abdelkareemm/webcodex) · [Releases](https://github.com/abdelkareemm/webcodex/releases) · [Changelog](CHANGELOG.md)

## Features

- **Native Rust terminal UI** — Interactive conversation, scrolling, prompt history, and live responses.
- **Project-aware coding** — Inspect project files, modify code, and execute shell commands.
- **ChatGPT browser integration** — Use your signed-in ChatGPT account without configuring an API key.
- **Resume conversations** — Continue WebCodex sessions or open accessible ChatGPT conversations by ID.
- **Codex session importing** — Transfer context from locally saved Codex CLI conversations.
- **Automatic file editing** — Apply generated edits with backups and undo support.
- **Headless mode** — Run browser automation without displaying the browser window.
- **Configurable permissions** — Choose when WebCodex asks before executing operations.

## Requirements

- Windows 10 or 11
- Python 3.9 or newer
- Rust and Cargo
- Microsoft C++ Build Tools (MSVC)
- A ChatGPT account

Rust and Python are required for installation from source. WebCodex uses ordinary ChatGPT usage limits.

## Installation

### Windows ZIP release (no Rust compiler required)

Download the Windows ZIP from the [GitHub Releases page](https://github.com/abdelkareemm/webcodex/releases) and extract it to a permanent folder.

Install Python 3.9 or newer. Open PowerShell in the extracted WebCodex directory and run:

```powershell
python -m pip install -e .
python -m playwright install chromium
python -m webcodex --version
```

To start WebCodex in your project directory:

```powershell
cd "C:\path\to\your\project"
webcodex
```

The ZIP includes the compiled Rust executables. Keep the extracted directory intact. You do not need Rust, Cargo, or MSVC to use the prebuilt release.

### Build from source

### 1. Install prerequisites

Install Python and Rust from PowerShell:

```powershell
winget install --id Python.Python.3.12 -e
winget install --id Rustlang.Rustup -e
```

If you do not already have Microsoft C++ Build Tools:

```powershell
winget install --id Microsoft.VisualStudio.2022.BuildTools -e --override "--wait --passive --add Microsoft.VisualStudio.Workload.VCTools --includeRecommended"
```

Restart PowerShell after installation.

Confirm the tools are available:

```powershell
python --version
cargo --version
```

### 2. Clone WebCodex

```powershell
git clone https://github.com/abdelkareemm/webcodex.git
cd webcodex
```

### 3. Install Python dependencies and Playwright

```powershell
python -m pip install -e .
python -m playwright install chromium
```

### 4. Build WebCodex

```powershell
powershell.exe -NoProfile -ExecutionPolicy Bypass -File ".\verify-native.ps1"
```

This runs the available tests and builds the release executables for the Rust core and terminal UI.

### 5. Launch WebCodex

Navigate to the project you want WebCodex to work on:

```powershell
cd "C:\path\to\your\project"
webcodex
```

On first use, complete any required ChatGPT sign-in through the browser.

You can now interact with WebCodex directly from your terminal.

## Usage

### Resume a previous WebCodex conversation

```powershell
webcodex resume
```

Select a conversation from the interactive picker.

To immediately resume the most recent session:

```powershell
webcodex resume --last
```

### Resume a specific ChatGPT conversation

Use the conversation ID from its ChatGPT URL:

```powershell
webcodex resume 6aebe9ae-0a9c-83ea-8e91-da222befc0c1
```

Or provide the complete URL:

```powershell
webcodex resume https://chatgpt.com/c/6aebe9ae-0a9c-83ea-8e91-da222befc0c1
```

The conversation must be accessible through your signed-in ChatGPT account.

### Import a Codex CLI conversation

```powershell
webcodex import-codex
```

Select a locally saved Codex session to transfer its conversational context into WebCodex.

To list available Codex sessions:

```powershell
webcodex import-codex --list
```

Imported sessions create new ChatGPT conversations. They do not transfer the original Codex execution environment.

### Run with automatic approvals

```powershell
webcodex resume -s danger-full-access -a never
```

This allows WebCodex to execute generated shell commands and modify files without local approval prompts.

**Warning:** These options grant broad access to your files and computer. This mode does not provide operating-system sandboxing. Only use it when you accept those risks.

### Run without a visible browser

```powershell
webcodex resume -s danger-full-access -a never --headless
```

Headless mode hides the Playwright-controlled browser, not the WebCodex terminal interface. Browser authentication challenges may require visible mode.

## Terminal shortcuts

| Shortcut | Action |
|---|---|
| Enter | Send message |
| Alt+Enter | Insert newline |
| Up / Down | Recall previous messages |
| Page Up / Page Down | Scroll conversation |
| Mouse wheel | Scroll conversation |
| End | Jump to latest message |
| Ctrl+C | Clear current draft |
| Ctrl+L | Clear visible transcript |
| Ctrl+Q | Exit WebCodex |

## Development

WebCodex uses two Rust crates:

- `rust-core/` — Agent orchestration, CLI, sessions, project operations, patches, and commands.
- `rust-tui/` — Native terminal interface.

The `webcodex/` Python package provides the browser transport, Playwright integration, and launcher.

Run the tests:

```powershell
powershell.exe -NoProfile -ExecutionPolicy Bypass -File ".\verify-native.ps1"
```

See [CONTRIBUTING.md](CONTRIBUTING.md) for development information and [SECURITY.md](SECURITY.md) for security considerations.

## Credits

**Created and developed by [abdelkareemm](https://github.com/abdelkareemm).**

WebCodex's terminal experience is inspired by [OpenAI Codex](https://github.com/openai/codex).

WebCodex is an independent project and is not an official OpenAI product.

Any third-party source code incorporated into WebCodex remains subject to its original licenses and required attribution notices.

## License

The Rust package manifests declare Apache-2.0. See the repository's LICENSE file once the full license text has been added.
