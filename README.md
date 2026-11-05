# WebCodex

[![
FILE: README.md
<<<<<<< SEARCH
=======
# WebCodex

[![Build](https://github.com/abdelkBuild](https://github.com/abdelkareemm/webcodex/actions/workflows/build.yml/badge.svg)](httpsareemm/webcodex/actions/workflows/build.yml/badge.svg)](https://github.com/abdelkareemm/webcodex/actions/workflows/build://github.com/abdelkareemm/webcodex/actions/workflows/build.yml)
[![Apache-2.0](https://img.yml)
[![Apache-2.0](https://img.shields.io/badge/license-Apache--.shields.io/badge/license-Apache--2.0-blue)](LICENSE)
[![Release2.0-blue)](LICENSE)
[![Release](https://img.shields.io/github/v/release/abdel](https://img.shields.io/github/v/release/abdelkareemm/webcodex)](https://github.com/abdelkkareemm/webcodex)](https://github.com/abdelkareemm/webcodex/releases)

**A Rust-powered terminalareemm/webcodex/releases)

**A Rust-powered terminal coding agent that coding agent that works through works through Chat ChatGPT.**

Created by **[abdelkareemm](https://GPT.**

Created by **[abdelkareemm](https://github.com/abdelkareemm)**.

[Download releasesgithub.com/abdelkareemm)**.

[Download releases](https://github.com/abdelk](https://github.com/abdelkareemm/webcodex/releases) | [areemm/webcodex/releases) | [Report anReport an issue](https://github.com/abdelkareemm/webcodex/issues issue](https://github.com/abdelkareemm/webcodex/issues) | [Changelog](CHANGELOG.md)

Web) | [Changelog](CHANGELOG.md)

WebCodex combines a native Rust terminal interface andCodex combines a native Rust terminal interface and coding agent with a Python Playwright browser side coding agent with a Python Playwright browser sidecar. It can work withcar. It can work with project files, run commands, apply edits project files, run commands, apply edits,, and continue conversations using an accessible ChatGPT account.

## Features

- Native Rust terminal interface with streaming and continue conversations using an accessible ChatGPT account.

## Features

- Native Rust terminal interface with streaming responses.
- Project-aware file editing, command execution, backups, and undo.
 responses.
- Project-aware file editing, command execution, backups, and undo.
- Conversation picker, prompt history, scrolling, and search- Conversation picker, prompt history, scrolling, and search.
- Resume accessible.
- Resume accessible ChatGPT conversations by URL or conversation ID.
- Import conversation ChatGPT conversations by URL or conversation ID.
- Import conversation context from local Codex CLI sessions.
- Optional head context from local Codex CLI sessions.
- Optional headless Chromium browserless Chromium browser.
- Configurable command approvals and execution permissions.

## Installation.
- Configurable command approvals and execution permissions.

## Installation

### Windows release (recommended)

**

### Windows release (recommended)

**Requirements:** Windows 10/11, Python 3.9 orRequirements:** Windows 10/11, Python 3.9 or newer, and a ChatGPT account. newer, and a ChatGPT account. Rust and Cargo are not required for the compiled Windows package.

1. Download Rust and Cargo are not required for the compiled Windows package.

1. Download `webcodex-v1.2.0-windows-x64.zip` `webcodex-v1.2.0-windows-x64.zip` from [GitHub Releases](https:// from [GitHub Releases](https://github.com/abdelkareemm/webcodex/releases) once the release is available.
2. Extract the ZIP togithub.com/abdelkareemm/webcodex/releases) once the release is available.
2. Extract the ZIP to a permanent folder, such as `C:\Tools\WebCodex`.
3 a permanent folder, such as `C:\Tools\WebCodex`.
3. Open PowerShell inside the extracted `webcodex` folder.
4.. Open PowerShell inside the extracted `webcodex` folder.
4. Run:

   ```powershell
   powershell.exe -NoProfile -Execution Run:

   ```powershell
   powershell.exe -NoProfile -ExecutionPolicy Bypass -File ".\install.ps1"
   ```

5.Policy Bypass -File ".\install.ps1"
   ```

5. Open a **new** PowerShell window and Open a **new** PowerShell window and navigate to your coding project:

   ```powershell
   navigate to your coding project:

   ```powershell
   cd "C:\path\to\your cd "C:\path\to\your\project"
   webcodex
   ```

Complete\project"
   webcodex
   ```

Complete the ChatGPT browser sign the ChatGPT browser sign-in if prompted. Keep the extracted WebCodex directory in place after installation.

If a-in if prompted. Keep the extracted WebCodex directory in place after installation.

If a compiled release is not available yet, follow the source installation below.

### Build from source

Install Python, compiled release is not available yet, follow the source installation below.

### Build from source

Install Python, Rust, and Microsoft C++ Build Tools:

```powershell
winget install -- Rust, and Microsoft C++ Build Tools:

```powershell
winget install --id Python.Python.3.12 -e
winget install --idid Python.Python.3.12 -e
winget install --id Rustlang.Rustup -e
 Rustlang.Rustup -e
wingwinget install --id Microsoft.VisualStudio.2022.BuildTools -e --override "--wait --passive --add Microsoft.VisualStudio.Workload.VCToolset install --id Microsoft.VisualStudio.2022.BuildTools -e --override "--wait --passive --add Microsoft.VisualStudio.Workload.VCTools --includeRecommended"
```

Reopen PowerShell, then:

```powershell
git clone --includeRecommended"
```

Reopen PowerShell, then:

```powershell
git clone https://github.com/abdelkareemm/webcodex.git
cd web https://github.com/abdelkareemm/webcodex.git
cd webcodex
python -m pip install -e .
python -m playwright installcodex
python -m pip install -e .
python -m playwright install chromium
powershell.exe -NoProfile -ExecutionPolicy Bypass -File chromium
powershell.exe -NoProfile -ExecutionPolicy Bypass -File ".\verify-native.ps1"
```

Launch WebCodex from your project ".\verify-native.ps1"
```

Launch WebCodex from your project directory:

```powershell
cd "C:\path\to\your\ directory:

```powershell
cd "C:\path\to\your\project"
webcodex
```

## Commonproject"
webcodex
```

## Common commands

| Command | Description |
|---|---|
| `webcod commands

| Command | Description |
|---|---|
| `webcodex` | Start a new session |
| `webcodex resume` |ex` | Start a new session |
| `webcodex resume` | Select an existing WebCodex conversation |
| `webcodex resume -- Select an existing WebCodex conversation |
| `webcodex resume --last` | Resume the most recent session |
| `webcodex resume <last` | Resume the most recent session |
| `webcodex resume <conversation-id>` | Open an accessible ChatGPT conversation |
| `conversation-id>` | Open an accessible ChatGPT conversation |
| `webcodex import-codex` | Import a local Codex CLI conversationwebcodex import-codex` | Import a local Codex CLI conversation |
| `webcodex import-codex --list` | List local |
| `webcodex import-codex --list` | List local Codex sessions |
| `webcodex resume --headless` | Run Codex sessions |
| `webcodex resume --headless` | Run without displaying the browser window without displaying the browser window |
| `webcodex --version` | Print the installed version |

For |
| `webcodex --version` | Print the installed version |

For example:

```powershell
webcodex resume - example:

```powershell
webcodex resume -s danger-full-access -a never --headless
```

**Security warnings danger-full-access -a never --headless
```

**Security warning:** `danger-full-access` and `-a never` allow model-requested:** `danger-full-access` and `-a never` allow model-requested commands and edits without local approval prompts. There is no operating-system sandbox in this commands and edits without local approval prompts. There is no operating-system sandbox in this mode. Only use it when you accept the risks mode. Only use it when you accept the risks.

WebCodex uses normal ChatGPT account access.

WebCodex uses normal ChatGPT account access and remains subject to applicable usage limits.

## Terminal and remains subject to applicable usage limits.

## Terminal shortcuts

| Key | Action |
|---|---|
| Enter | Send shortcuts

| Key | Action |
|---|---|
| Enter | Send message |
| Alt+Enter | Insert message |
| Alt+Enter | Insert newline |
| Up / Down | Recall sent newline |
| Up / Down | Recall sent prompts |
| Page Up / Page Down | Scroll conversation |
| Mouse wheel | prompts |
| Page Up / Page Down | Scroll conversation |
| Mouse wheel | Scroll conversation |
| End | Jump to latest |
 Scroll conversation |
| End | Jump to latest |
| Ctrl+C | Clear draft |
| Ctrl+L | Clear visible transcript |
| Ctrl+C | Clear draft |
| Ctrl+L | Clear visible transcript |
| Ctrl+Q | Exit |

## Development

-| Ctrl+Q | Exit |

## Development

- `rust-core/` - Native `rust-core/` - Native CLI, agent, session CLI, agent, session storage, file operations, and command execution.
- `rust-tui/` storage, file operations, and command execution.
- `rust-tui/` - Terminal interface.
- `webcodex/` - Python Playwright browser - Terminal interface.
- `webcodex/` - Python Playwright browser sidecar and launcher.
- `tests/` - Integration sidecar and launcher.
- `tests/` - Integration and regression tests.

Run Windows verification:

```powershell
powershell and regression tests.

Run Windows verification:

```powershell
powershell.exe -NoProfile -ExecutionPolicy Bypass -File ".\verify-native.ps.exe -NoProfile -ExecutionPolicy Bypass -File ".\verify-native.ps1"
```

See [CONTRIBUTING.md](CONTRIBUTING1"
```

See [CONTRIBUTING.md](CONTRIBUTING.md), [SECURITY.md](SECURITY.md), and [release.md), [SECURITY.md](SECURITY.md), and [release instructions](docs/RELEASING.md).

## Credits

**Web instructions](docs/RELEASING.md).

## Credits

**WebCodex is created andCodex is created and developed by [abdelkareemm]( developed by [abdelkareemm](https://github.com/abdelkareemm).**

Its terminal experiencehttps://github.com/abdelkareemm).**

Its terminal experience is inspired by [OpenAI Codex](https://github.com/openai is inspired by [OpenAI Codex](https://github.com/openai/codex). WebCodex is independent/codex). WebCodex is independent and is not an official OpenAI product.

Third-party and is not an official OpenAI product.

Third-party code retains any required original copyright and code retains any required original copyright and license notices.

## License

Licensed under [Apache License 2.0 license notices.

## License

Licensed under [Apache License 2.0](LICENSE).
