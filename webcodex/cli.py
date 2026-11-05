"""WebCodex native launcher. Python is a Playwright sidecar, not the agent runtime."""
from __future__ import annotations
import os
import subprocess
import sys
from pathlib import Path


def native_executable() -> Path:
    name = "webcodex-native.exe" if os.name == "nt" else "webcodex-native"
    override = os.environ.get("WEBCODEX_RUST_AGENT")
    return Path(override) if override else Path(__file__).resolve().parent.parent / "rust-core" / "target" / "release" / name


def main(argv=None) -> int:
    args = list(sys.argv[1:] if argv is None else argv)
    native = native_executable()
    if not native.is_file():
        print("WebCodex native executable not found: " + str(native), file=sys.stderr)
        print("Build WebCodex with .\\build-rust.ps1 from the extracted source directory.", file=sys.stderr)
        return 2
    try:
        return subprocess.call([str(native), *args])
    except OSError as exc:
        print("Could not launch WebCodex Rust agent: " + str(exc), file=sys.stderr)
        return 1
