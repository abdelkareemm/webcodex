# Runtime source language measurements

Counted UTF-8 source file byte sizes in this native-only distribution, excluding tests, generated files, and third-party libraries:

- Rust (.rs): **79,971 bytes**, **60.5%**
- Python runtime (.py): **52,182 bytes**, **39.5%**

These are reproducible file-byte measurements, not GitHub Linguist output.

This is **not** the 90/10 target by source bytes; the Python Playwright transport
accounts for most of the remaining Python. The process ownership is Rust-first.

The preferred improvement is to migrate browser-independent DOM extraction, response
parsing and diagnostics out of the Python sidecar *without sacrificing reliability*.
A language-percent target should never be met by padding Rust code or discarding
required browser functionality.
