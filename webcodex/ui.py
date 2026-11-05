from __future__ import annotations

import difflib
import os
import sys
from typing import Optional

if os.name == "nt":
    os.system("")  # enables ANSI escape codes in Windows 10+ terminals


class _Colors:
    def __init__(self) -> None:
        on = sys.stdout.isatty() and not os.environ.get("NO_COLOR")
        self.reset = "\033[0m" if on else ""
        self.bold = "\033[1m" if on else ""
        self.dim = "\033[2m" if on else ""
        self.red = "\033[31m" if on else ""
        self.green = "\033[32m" if on else ""
        self.yellow = "\033[33m" if on else ""
        self.cyan = "\033[36m" if on else ""


C = _Colors()


class AbortTurn(Exception):
    """User chose to quit the current task."""


def info(msg: str) -> None:
    print(f"{C.cyan}{msg}{C.reset}")


def dim(msg: str) -> None:
    print(f"{C.dim}{msg}{C.reset}")


def warn(msg: str) -> None:
    print(f"{C.yellow}{msg}{C.reset}")


def err(msg: str) -> None:
    print(f"{C.red}{msg}{C.reset}")


def print_diff(rel: str, old: Optional[str], new: str, max_lines: int = 160) -> None:
    diff = list(
        difflib.unified_diff(
            (old or "").splitlines(),
            new.splitlines(),
            fromfile=f"a/{rel}" if old is not None else "/dev/null",
            tofile=f"b/{rel}",
            lineterm="",
            n=3,
        )
    )
    for line in diff[:max_lines]:
        if line.startswith("+++") or line.startswith("---"):
            print(f"{C.bold}{line}{C.reset}")
        elif line.startswith("+"):
            print(f"{C.green}{line}{C.reset}")
        elif line.startswith("-"):
            print(f"{C.red}{line}{C.reset}")
        elif line.startswith("@@"):
            print(f"{C.cyan}{line}{C.reset}")
        else:
            print(line)
    if len(diff) > max_lines:
        dim(f"... ({len(diff) - max_lines} more diff lines)")


def confirm(question: str, default: str = "y") -> str:
    """Returns 'y', 'n' or 'a' (yes to all). Raises AbortTurn on quit."""
    hint = "[Y/n/a/q]" if default == "y" else "[y/N/q]"
    while True:
        try:
            ans = input(f"{C.yellow}{question}{C.reset} {hint} ").strip().lower()
        except (EOFError, KeyboardInterrupt):
            raise AbortTurn()
        if ans == "":
            return default
        if ans in ("y", "yes"):
            return "y"
        if ans in ("n", "no"):
            return "n"
        if ans in ("a", "all") and default == "y":
            return "a"
        if ans in ("q", "quit"):
            raise AbortTurn()
