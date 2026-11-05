**Version: 1.2.0 (release #22).** See `CHANGELOG.md` for the history.

# Rust migration status

This package is **Rust-primary at runtime**: no legacy Python agent/CLI fallback is
included. The Python files left in the runtime are solely the browser sidecar and its
configuration, streaming, and diagnostics helpers.

**Unverified:** Native Rust compilation, Windows rendering, interaction with a live
ChatGPT browser session, and end-to-end parity with the historical Python implementation.
Do not assume existing scripts using the removed Python modules remain compatible.
Source-language percentages are calculated separately and are not claimed to meet 90/10.

## Rust terminal QoL update

Added prompt history, viewport scrollbar, mouse/keyboard jump-to-bottom, follow-tail behavior, startup spinner and browser status events, and stronger assistant code/edit block suppression. Python sidecar and source-contract checks passed. Rust build and interactive terminal behavior must be verified with Cargo on Windows.

## Codex-to-WebCodex session migration

The new native importer reads Codex CLI JSONL history but does not reuse a Codex
backend, execution state, or remote conversation ID. A chosen session creates
a NEW ChatGPT chat with a bounded context handoff on the first task.
Interactive session selection is implemented before browser startup.
Rust compilation and Windows interactive picker behavior require on-device validation.
