"""WebCodex native launcher. Python is a Playwright sidecar, not the agent runtime."""
from __future__ import annotations
import os
import subprocess
import sys
from pathlib import Path


def native_executable() -> Path:
    name = "webcodex-native.exe" if os.name == "nt" else "webcodex-native"
    override = os.environ.get("WEBCODEX_RUST_AGENT")
    if override:
        return Path(override)
    root = Path(__file__).resolve().parent.parent / "rust-core"
    staged = root / "target-staging" / "release" / name
    return staged if staged.is_file() else root / "target" / "release" / name


def main(argv=None) -> int:
    args = list(sys.argv[1:] if argv is None else argv)
    native = native_executable()
    if not native.is_file():
        print("WebCodex native executable not found: " + str(native), file=sys.stderr)
        print("Build WebCodex with .\\build-rust.ps1 from the extracted source directory.", file=sys.stderr)
        return 2
    try:
        from . import __version__
        actual = subprocess.check_output([str(native), "--version"], text=True, timeout=10).strip()
        if actual != f"webcodex {__version__} (Rust)":
            print(f"Runtime version mismatch: expected {__version__}, got {actual}", file=sys.stderr)
            return 2
        return subprocess.call([str(native), *args])
    except (OSError, subprocess.CalledProcessError, subprocess.TimeoutExpired) as exc:
        print("Could not launch WebCodex Rust agent: " + str(exc), file=sys.stderr)
        return 1
