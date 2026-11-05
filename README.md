# WebCodex 1.2.0 — Release #22

**WebCodex v1.2.0 — Release #22**

WebCodex is a Rust-native terminal coding agent that uses a local Playwright-controlled browser to interact with ChatGPT.

It provides a Codex-inspired coding experience with a native terminal interface, project-aware file editing, shell execution, conversation history, and session management.

> **Disclaimer:** WebCodex is an independent project. It is not the official OpenAI Codex CLI and is not affiliated with or endorsed by OpenAI.

## Features

- **Rust-native architecture:** Terminal UI, agent orchestration, file operations, and session management.
- **ChatGPT integration:** Uses a signed-in browser session through Python Playwright.
- **Project-aware coding:** Reads project files, applies edits, runs commands, and maintains backups.
- **Session management:** Resume existing WebCodex conversations or select from previous sessions.
- **Codex CLI importing:** Transfer conversation context from locally saved Codex sessions.
- **ChatGPT conversation resume:** Open accessible conversations using their UUID or URL.
- **Terminal experience:** Prompt history, smooth scrolling, automatic follow, compact Markdown rendering, and hidden code blocks.
- **Headless operation:** Run browser automation without opening a visible browser window.

## Repository documentation

- [Changelog](CHANGELOG.md) — Release history and versioning.
- [Contributing](CONTRIBUTING.md) — Development setup and contribution guidelines.
- [Security](SECURITY.md) — Security considerations and vulnerability reporting.

## Security notice

WebCodex can execute generated shell commands and modify local files.

Using `-s danger-full-access -a never` disables local approval prompts and permits commands to run with your Windows account permissions. It does not provide an operating-system sandbox.

Never publish browser profiles, authentication cookies, local conversation databases, or private project files.

WebCodex remains subject to applicable ChatGPT usage limits.

---

## Technical documentation

The CLI, conversation management, prompt planning, project scanning, patch application,
file backups, undo, shell execution and terminal interface are implemented in **Rust**.
The only Python runtime is the **Playwright browser sidecar**. This is a native-first
source distribution; the original Python fallback CLI is intentionally excluded.

## Windows quick start

1. Install Rust/Cargo via `winget install --id Rustlang.Rustup -e` and MSVC C++ Build Tools if required.
2. In the extracted WebCodex directory:

   ```powershell
   $env:Path = "$env:USERPROFILE\.cargo\bin;$env:Path"
   powershell.exe -NoProfile -ExecutionPolicy Bypass -File .\build-rust.ps1
   python -m pip install -e .
   ```
3. From a project directory:

   ```powershell
   webcodex resume -s danger-full-access -a never
   ```

`-s danger-full-access` lets the Rust core run commands with your local user's privileges;
`-a never` skips local confirmations. **There is no OS sandbox in this mode.**

## Architecture

* `rust-core/src/bin/webcodex-native.rs`: Rust command-line entry point and task loop
* `rust-core/src/{main,store,project,planner,reply,backup}.rs`: Rust worker operations
* `rust-tui`: native Rust terminal UI
* `webcodex/browser_bridge.py`: JSON-lines browser IPC
* `webcodex/transports.py`: Python Playwright page interaction only

The distribution uses the original `~/.webcodex/workspace.sqlite3` session store and
browser profile. Make a backup before running this native-first release on important projects.

## Verification

The Python sidecar can be tested without Rust via `python -m compileall -q webcodex`.
Full native functionality requires successful `cargo test` and `cargo build` for both
Rust crates on your target OS. This source was not compiled in the packaging environment.
The 90/10 source-language target should be verified by GitHub Linguist; language percentages
alone are not a substitute for behavioral tests.

### Terminal display preferences

WebCodex keeps model-generated fenced code/edit blocks out of the terminal transcript (including live updates and resumed messages). File edits and shell commands are still parsed and applied by the agent; their activity summaries remain visible. User messages have a subtle gray background, including wrapped lines, to distinguish them from assistant responses.

### Terminal quality-of-life controls (November 2026)

* **Up / Down** in the single-line composer recalls previous submitted prompts; Down restores any draft you were composing. Prompts from the restored conversation are included.
* **Mouse wheel / Page Up / Page Down / Ctrl+Up / Ctrl+Down** scroll the transcript. Manually scrolling up pauses automatic following of incoming replies. Scrolling back to the bottom resumes it.
* **End** with an empty composer, or click **↓ Jump to latest (End)** when it appears, resumes automatic scrolling. A thin scrollbar shows transcript position.
* **Ctrl+L** clears only the visible transcript; session history remains saved. **Ctrl+C** clears the current unsent draft, and **Ctrl+Q** exits cleanly.
* Model-generated fenced blocks, inline edit-block syntax, HTML preformatted blocks, and execution directives are hidden from the transcript in both native TUI and plain text mode. The agent still receives the original unfiltered reply for edit execution.
* Browser startup opens the TUI first and shows a moving WebCodex loading indicator and browser connection stages. It may still require visible login if ChatGPT requests authentication.

These changes are provided as Rust source. Run `verify-native.ps1` to compile and test them on Windows before launching; the packaging environment did not have Cargo installed.

### Compact Markdown transcript (formatting update)

The Rust TUI now formats assistant text instead of displaying raw Markdown:
headings appear as bold section labels, list items are grouped, inline Markdown
markers and link URLs are not shown, and repeated blank lines or `---` section
separators do not waste screen height. Assistant code fences remain hidden, and
incoming streamed replies use the same formatter as completed responses.
Your prompt history, scrolling controls, gray user messages, and browser agent
are unchanged.

### Transcript performance improvements

The Rust terminal caches transcript formatting and terminal-width wrapping. Mouse-wheel,
Page Up, Page Down, and scrollbar movement reuse that cache and render only visible
rows instead of reprocessing the entire conversation on every frame. The UI also
redraws on events, rather than continuously while idle. A resize or new message
invalidates the cached layout.

On resume, the last **50 messages** from the existing SQLite session are loaded
in one batch. Older messages are **not deleted**; they remain in the database and
in the original ChatGPT conversation. This limits terminal restoration time and
memory use. Playwright still needs to reopen the corresponding ChatGPT page;
network or login delays are separate from terminal transcript performance.

### Windows terminal repaint hardening

The Rust TUI clears the ratatui back buffer when transcript content, scrolling,
resizing, or layout changes invalidate the physical screen, while normal typing
continues to use cheap incremental redraws. It also reserves the final terminal
column to avoid ConPTY auto-wrap and strips ANSI cursor-control sequences from
assistant/tool output. These changes target stale overlapping text in Windows
Terminal and VS Code integrated terminals. Rebuild on Windows using
`powershell.exe -NoProfile -ExecutionPolicy Bypass -File .\verify-native.ps1`.

## Session selection and Codex CLI import (WebCodex 1.1.0)

When run in an interactive terminal, **`webcodex resume` opens a searchable
WebCodex session picker before starting the browser**. Use Up/Down to select,
type to filter, Enter to continue, and Esc to start a new session. Sessions are
scoped to the current project. An exact ID bypasses the picker:

```powershell
webcodex resume -s danger-full-access -a never --headless
webcodex resume --last -s danger-full-access -a never --headless
webcodex --thread-id <WebCodex-thread-id>
```

`--last` preserves the previous immediate resume-latest behavior. Noninteractive
terminals also choose the latest session. The existing `--list-threads` command
prints stored WebCodex threads without starting a browser.

**Import a local Codex CLI conversation** using a separate picker:

```powershell
webcodex import-codex --list
webcodex import-codex -s danger-full-access -a never --headless
webcodex import-codex --session <Codex-session-id> --headless
```

Sessions are discovered under `CODEX_HOME` (or `~/.codex/sessions` and
`~/.codex/archived_sessions`). `--codex-home <folder>` overrides that path.
The importer **never changes the source Codex history**, and never imports tool
outputs, tool calls, or internal reasoning. It saves up to the 100 most recent
user/assistant messages in WebCodex's SQLite database and creates a bounded
handoff for your first task in a *new* ChatGPT conversation. It does not migrate
Codex tool state, authentication, or the original conversation ID. Make sure
you launch WebCodex from the intended project directory; source session paths
are shown in the picker as a reminder. This uses ordinary ChatGPT usage limits,
not an unlimited Codex bypass.

Picker keys operate before the Rust TUI is launched, so the browser does not
need to load just to choose a conversation. On noninteractive terminals, pass
`--session <id>` when importing to avoid inadvertently selecting the latest.

### Open any existing ChatGPT conversation by ID (WebCodex 1.2.0)

```powershell
webcodex resume 6aebe9ae-0a9c-83ea-8e91-da222befc0c1 -s danger-full-access -a never
webcodex resume https://chatgpt.com/c/6aebe9ae-0a9c-83ea-8e91-da222befc0c1 -s danger-full-access -a never
```

This opens the **same existing ChatGPT conversation** in the Playwright browser
profile. You must be logged in and have access to it. On first use, WebCodex
links it to a new local SQLite thread; subsequent launches reuse that thread
when invoked from the same project. If navigation fails, WebCodex reports an
error rather than creating an unrelated chat. The remote conversation remains
on ChatGPT; WebCodex cannot import its full historical messages into its local
terminal transcript until they are subsequently saved locally. Session picker
and `--last` remain available for WebCodex-local conversations.
