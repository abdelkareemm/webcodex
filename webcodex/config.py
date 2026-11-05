from __future__ import annotations

import json
import os
from dataclasses import dataclass, field
from pathlib import Path
from typing import Optional

HOME = Path(os.environ.get("WEBCODEX_HOME") or (Path.home() / ".webcodex"))

# CSS selectors for the ChatGPT web UI. ChatGPT changes its markup now and then;
# if something stops working, add your own selectors in ~/.webcodex/config.json:
#   {"selectors": {"send": ["button.my-new-send-button"]}}
# User selectors are tried BEFORE these defaults.
DEFAULT_SELECTORS = {
    "composer": [
        "#prompt-textarea",
        "[data-composer-input] [contenteditable='true']",
        "div.ProseMirror[contenteditable='true']",
        "div[contenteditable='true'][data-virtualkeyboard]",
        "textarea",
    ],
    "send": [
        "[data-testid='send-button']",
        "[data-testid='composer-submit-button']",
        "button[aria-label='Send prompt']",
        "button[aria-label='Send message']",
        "button[type='submit'][aria-label='Send']",
        "button[aria-label^='Send']",
        "form button[type='submit']",
    ],
    "stop": [
        "[data-testid='stop-button']",
        "button[aria-label='Stop streaming']",
        "button[aria-label='Stop generating']",
        "button[aria-label='Stop']",
    ],
    # Shown when a long reply is cut off; WebCodex clicks it for you.
    "continue": [
        "button:has-text('Continue generating')",
    ],
    "user": [
        "[data-message-author-role='user']",
    ],
    "assistant": [
        "[data-message-author-role='assistant']",
    ],
    # Optional: elements holding ChatGPT's visible "thinking" summary. Empty by default
    # (the markup varies); if you find one in DevTools, add it here to stream it too.
    "thinking": [],
    # Shown only to logged-OUT visitors:
    "login_button": [
        "[data-testid='login-button']",
        "[data-testid='signup-button']",
        "a[href*='/auth/login']",
        "button:has-text('Log in')",
        "button:has-text('Sign up')",
    ],
    # Shown only when logged IN:
    "logged_in": [
        "[data-testid='accounts-profile-button']",
        "button[aria-label='Open profile menu']",
        "[data-testid='profile-button']",
        "button[data-testid*='profile']",
    ],
}


@dataclass
class Config:
    url: str = "https://chatgpt.com/"
    browser: str = "chrome"  # chrome | msedge | chromium
    profile_dir: Path = HOME / "profile"
    headless: bool = False
    cdp: Optional[str] = None  # e.g. http://localhost:9222 to attach to your own Chrome
    temporary: bool = False  # use ChatGPT "temporary chat" (not saved to history)
    model: Optional[str] = None  # best-effort ?model=<slug> URL param

    max_chars: int = 60000  # budget of file characters per message
    max_file_chars: int = 20000  # files bigger than this are not auto-included
    max_files: int = 4000  # refuse to scan folders bigger than this
    max_turns: int = 6  # max automatic follow-up turns per task
    reply_timeout: int = 900
    start_timeout: int = 90
    stable_secs: float = 2.5
    run_timeout: int = 120

    selectors: dict = field(
        default_factory=lambda: {k: list(v) for k, v in DEFAULT_SELECTORS.items()}
    )

    def load_user_file(self) -> None:
        path = HOME / "config.json"
        if not path.exists():
            return
        try:
            data = json.loads(path.read_text(encoding="utf-8"))
        except Exception as e:  # noqa: BLE001
            print(f"warning: could not read {path}: {e}")
            return
        for key, val in (data.pop("selectors", {}) or {}).items():
            user = [val] if isinstance(val, str) else list(val)
            self.selectors[key] = user + self.selectors.get(key, [])
        for key, val in data.items():
            if hasattr(self, key):
                setattr(self, key, Path(val).expanduser() if key == "profile_dir" else val)
