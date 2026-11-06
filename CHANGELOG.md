# WebCodex release history

The current development version is defined in `pyproject.toml` and the Rust Cargo manifests. This document records historical releases and is not automatically updated for every development version increment.

## 1.2.1 - Release #23 - 2026-11-05

Maintenance release focused on response integrity and safe automatic editing.

- Preserve captured ChatGPT response streams instead of replacing them with DOM text.
- Reject incomplete response streams and unverified DOM edit instructions.
- Reject malformed, unfenced, incomplete, and nested edit blocks.
- Roll back all edits to a file when any edit in its batch fails.
- Add regression tests for transport integrity, parser validation, and atomic patching.
- Preserve existing resume, Codex import, and terminal functionality.


**Current development version: 1.4.67 (November 6, 2026)**

## Changes since 1.3.0

### 1.4.0 — Native agent and skills integration

- Updated native-agent and skills integration code.
- Synchronized Python, Rust core, and Rust TUI version metadata.

### 1.4.1–1.4.67 — Reliability and recovery improvements

- Added native filesystem backup, undo, recovery journals, and rollback safeguards.
- Added `/recover-files` for recovering from interrupted undo operations.
- Retire completed backups before deleting recovery snapshots, preventing accidental repeat undo.
- Reject unsafe paths, symlink escapes, duplicate manifest entries, and backup-retirement collisions.
- Improved persistent task queue recovery and interrupted-task handling.
- Added command timeout handling and bounded stdout/stderr capture.
- Strengthened Rust response parsing and regression coverage.
- Fixed browser cancellation state handling at operation execution time.
- Removed unsafe fallback Send clicks and keyboard submissions that could duplicate prompts.
- Added native-launcher version checks and Windows launch support.
- Expanded Rust core, terminal UI, and Python regression tests.
- Added staged release builds and isolated native-worker startup verification.

The v1.4.66 source release passed 165 automated tests and staged runtime verification. Version 1.4.67 includes this documentation update and requires a new build before its binaries can be considered verified.

The 1.1.0 update adds interactive session selection and Codex CLI import to the 1.0.0 Rust-native milestone, which consolidates the prior 0.3.3 development snapshots into a Rust-native agent and Rust terminal interface, with Python retained for Playwright browser communication. It includes the latest terminal redraw improvements to address stale text in Windows Terminal and VS Code terminals.

Version policy: `MAJOR.MINOR.PATCH` (SemVer). Future breaking compatibility changes increment MAJOR, new backward-compatible functionality increments MINOR, and fixes increment PATCH.

**Counting method:** Every distinct WebCodex ZIP generated in this conversation is counted as one historical package, including the initial baseline. Those packages were snapshots and did not previously receive distinct numeric versions; the entries below enumerate them retrospectively, without inventing prior version numbers or claiming public GitHub releases.

## Historical package timeline

| # | Labeled version | Package milestone |
|---:|:---:|---|
| 1 | 0.3.3 | Initial WebCodex baseline (`webcodex-0.3.3.zip`) |
| 2 | 0.3.3 | Codex-style CLI flags (`webcodex-0.3.3-codex-flags.zip`) |
| 3 | 0.3.3 | Prompt-style TUI (`webcodex-0.3.3-codex-tui.zip`) |
| 4 | 0.3.3 | Prompt event-loop fix (`webcodex-0.3.3-codex-tui-fixed.zip`) |
| 5 | 0.3.3 | Full-screen terminal UI (`webcodex-0.3.3-fullscreen-tui.zip`) |
| 6 | 0.3.3 | Clean Codex-style layout and scrolling (`webcodex-0.3.3-codex-clean-scroll-fixed.zip`) |
| 7 | 0.3.3 | Rust TUI source (`webcodex-0.3.3-rust-tui-source.zip`) |
| 8 | 0.3.3 | Rust core migration (`webcodex-0.3.3-rust-core-migration.zip`) |
| 9 | 0.3.3 | Expanded Rust migration (`webcodex-0.3.3-rust-migration-expanded.zip`) |
| 10 | 0.3.3 | Rust-primary controller source (`webcodex-0.3.3-rust-primary-source.zip`) |
| 11 | 0.3.3 | Rust-primary continuation (`webcodex-0.3.3-rust-primary-continued.zip`) |
| 12 | 0.3.3 | Native-first Rust distribution (`webcodex-0.3.3-native-first-rust.zip`) |
| 13 | 0.3.3 | Windows socket handling fix (`webcodex-0.3.3-windows-socket-fix.zip`) |
| 14 | 0.3.3 | Windows runtime fix v2 (`webcodex-0.3.3-win-runtime-fix-v2.zip`) |
| 15 | 0.3.3 | Hidden code blocks and gray user messages (`webcodex-0.3.3-clean-transcript-gray-user.zip`) |
| 16 | 0.3.3 | Rust terminal QoL improvements (`webcodex-0.3.3-rust-tui-qol.zip`) |
| 17 | 0.3.3 | Compact Markdown transcript (`webcodex-0.3.3-compact-transcript.zip`) |
| 18 | 0.3.3 | Fast scrolling and batched resume (`webcodex-0.3.3-fast-scroll-resume.zip`) |
| 19 | 0.3.3 | Terminal redraw fix (`webcodex-0.3.3-terminal-redraw-fix.zip`) |
| 20 | **1.0.0** | Major Rust-native milestone, incorporating terminal redraw fix (`webcodex-1.0.0-release-20.zip`) |
| 21 | **1.1.0** | Conversation picker and local Codex session importer (`webcodex-1.1.0-release-21.zip`) |

| 22 | **1.2.0** | ChatGPT URL/UUID direct resume, integrated with project session picker (`webcodex-1.2.0-release-22.zip`) |

## Verification status

The previous 0.3.3 snapshot passed user-run Windows Rust builds and tests before the latest redraw changes. This 1.0.0 package changes the version metadata and release documentation on top of that redraw-fix snapshot; Rust compilation and live terminal behavior must be reverified on Windows. The major version denotes the magnitude of architecture changes, **not a claim of production stability**, perfect Codex UI parity, or a 90% Rust source ratio.

## 1.1.0 — Release #21 — 2026-11-05

- Searchable arrow-key session picker on `webcodex resume` before browser startup.
- `resume --last` keeps the previous quick-resume mode; `--thread-id` stays supported.
- Read-only local Codex CLI JSONL importer (`import-codex --list`, interactive
  selection, and explicit `--session <id>`).
- Filtered Codex message transfer without tool output, with bounded first-task
  handoff and restored local transcript. Imported sessions start a **new**
  ChatGPT conversation and remain resumable in WebCodex.
- Metadata-first discovery for large Codex session directories.

## 1.2.0 — Release #22 — 2026-11-05

- Directly resume any accessible ChatGPT conversation by its UUID or exact `https://chatgpt.com/c/<UUID>` URL.
- Reuse local WebCodex threads linked to the same ChatGPT URL within a project.
- Fail clearly on inaccessible remote chats rather than silently creating a new one.
- Preserve interactive WebCodex resume selection, Codex CLI import picker, and `--last`.
