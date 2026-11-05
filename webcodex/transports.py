"""How prompts get to ChatGPT: a real browser (Playwright) driving the ChatGPT website,
or manual copy/paste. No API is involved."""
from __future__ import annotations

import os
import re
import shutil
import subprocess
import sys
import time
from pathlib import Path
from typing import Optional

from .config import Config
from . import ui
from .stream import ReplyStream


class TransportError(Exception):
    pass


class TransportCancelled(TransportError):
    pass


# Walks the DOM of the assistant message(s) and rebuilds markdown-ish text.
# Code blocks are emitted verbatim inside ```` fences (exact whitespace preserved).
EXTRACT_JS = r"""
({sel, start}) => {
  const BLOCK = new Set(['P','DIV','LI','UL','OL','H1','H2','H3','H4','H5','H6','TABLE','TR','BLOCKQUOTE','SECTION']);
  const SKIP = new Set(['SCRIPT','STYLE','BUTTON','SVG','NOSCRIPT']);
  function codeText(pre) {
    const cm = pre.querySelector('.cm-content');
    if (cm) {
      const lines = cm.querySelectorAll('.cm-line');
      if (lines.length) return Array.from(lines).map(l => l.textContent).join('\n');
    }
    const code = pre.querySelector('code');
    return code ? code.textContent : pre.innerText;
  }
  function walk(n, out) {
    if (n.nodeType === 3) { out.push(n.nodeValue); return; }
    if (n.nodeType !== 1) return;
    const tag = n.tagName.toUpperCase();
    if (tag === 'PRE') { out.push('\n````\n' + codeText(n).replace(/\n$/, '') + '\n````\n'); return; }
    if (tag === 'BR') { out.push('\n'); return; }
    if (SKIP.has(tag)) return;
    const block = BLOCK.has(tag);
    if (block) out.push('\n');
    for (const c of n.childNodes) walk(c, out);
    if (block) out.push('\n');
  }
  const nodes = Array.from(document.querySelectorAll(sel)).slice(start);
  return nodes.map(n => { const out = []; walk(n, out); return out.join(''); }).join('\n\n');
}
"""

COUNT_JS = "(sel) => document.querySelectorAll(sel).length"

# Installed in every page before ChatGPT's own scripts run. It only *copies* the response of the
# page's own "send a message" request (POST .../backend-api/[f/]conversation) so WebCodex can read
# the reply from the stream itself; it changes nothing about the request or the page.
STREAM_HOOK_JS = r"""
(() => {
  if (window.__wc && window.__wc.hooked) return;
  const wc = window.__wc = { hooked: true, recs: [] };
  const origFetch = window.fetch;
  const wanted = (url, method) => {
    try {
      const u = new URL(url, location.href);
      return /^\/backend-api\/(f\/)?conversation$/.test(u.pathname) && String(method || 'GET').toUpperCase() === 'POST';
    } catch (e) { return false; }
  };
  window.fetch = async function(input, init) {
    const res = await origFetch.apply(this, arguments);
    try {
      const url = typeof input === 'string' ? input : (input && input.url) || '';
      const method = (init && init.method) || (input && input.method) || 'GET';
      if (wanted(url, method) && res.body) {
        const rec = { url, text: '', done: false, error: null };
        wc.recs.push(rec);
        const reader = res.clone().body.getReader();
        const dec = new TextDecoder();
        (async () => {
          try {
            for (;;) {
              const { value, done } = await reader.read();
              if (done) break;
              rec.text += dec.decode(value, { stream: true });
            }
          } catch (e) { rec.error = String(e); }
          rec.done = true;
        })();
      }
    } catch (e) { /* never break the page */ }
    return res;
  };
})();
"""
STREAM_STATE_JS = "() => { const r = (window.__wc && window.__wc.recs) || []; return r.map(x => ({text: x.text, done: x.done})); }"

PASTE_JS = r"""
(el, text) => {
  el.focus();
  const dt = new DataTransfer();
  dt.setData('text/plain', text);
  el.dispatchEvent(new ClipboardEvent('paste', {clipboardData: dt, bubbles: true, cancelable: true}));
  return true;
}
"""


def tidy(text: str) -> str:
    """Collapse runs of blank lines outside code fences."""
    segs = text.split("````")
    for i in range(0, len(segs), 2):
        segs[i] = re.sub(r"\n{3,}", "\n\n", segs[i])
    return "````".join(segs).strip()


class BrowserTransport:
    def __init__(self, cfg: Config):
        self.cfg = cfg
        self._pw = None
        self._browser = None
        self._ctx = None
        self.page = None
        self._pending_reply = None
        self._cancelled = None

    def _check_cancel(self):
        if self._cancelled is not None and self._cancelled():
            self._stop_generation()
            self._pending_reply = None
            raise TransportCancelled("Cancelled by user")

    # ---- lifecycle ------------------------------------------------------
    def start(self) -> None:
        try:
            from playwright.sync_api import sync_playwright
        except ImportError:
            raise TransportError(
                "Playwright is not installed. Run:  python -m pip install playwright"
            )
        cfg = self.cfg
        self._pw = sync_playwright().start()
        if cfg.cdp:
            self._browser = self._pw.chromium.connect_over_cdp(cfg.cdp)
            self._ctx = (
                self._browser.contexts[0]
                if self._browser.contexts
                else self._browser.new_context()
            )
        else:
            Path(cfg.profile_dir).mkdir(parents=True, exist_ok=True)
            kwargs = dict(
                user_data_dir=str(cfg.profile_dir),
                headless=cfg.headless,
                viewport={"width": 1280, "height": 900} if cfg.headless else None,
                args=["--disable-blink-features=AutomationControlled"],
                ignore_default_args=["--enable-automation"],
            )
            channel = None if cfg.browser == "chromium" else cfg.browser
            try:
                if channel:
                    self._ctx = self._pw.chromium.launch_persistent_context(
                        channel=channel, **kwargs
                    )
                else:
                    self._ctx = self._pw.chromium.launch_persistent_context(**kwargs)
            except Exception as e:  # noqa: BLE001
                if channel:
                    ui.warn(
                        f"Could not launch '{channel}' ({str(e).splitlines()[0]}). "
                        "Falling back to bundled Chromium (run: python -m playwright install chromium)."
                    )
                    self._ctx = self._pw.chromium.launch_persistent_context(**kwargs)
                else:
                    raise
        try:
            self._ctx.add_init_script(STREAM_HOOK_JS)
        except Exception:  # noqa: BLE001
            pass
        pages = self._ctx.pages
        self.page = pages[0] if pages else self._ctx.new_page()
        try:  # pages that were already open when the hook was added
            self.page.evaluate(STREAM_HOOK_JS)
        except Exception:  # noqa: BLE001
            pass

    def close(self) -> None:
        for fn in (
            lambda: self._ctx.close() if (self._ctx and not self.cfg.cdp) else None,
            lambda: self._browser.close() if self._browser else None,
            lambda: self._pw.stop() if self._pw else None,
        ):
            try:
                fn()
            except BaseException:  # noqa: BLE001  (incl. a second Ctrl-C)
                pass

    # ---- helpers --------------------------------------------------------
    def _sel(self, key: str) -> str:
        return ", ".join(self.cfg.selectors[key])

    VOICE_WORDS = ("voice", "dictat", "microphone", "speech")

    def _is_voice_button(self, el) -> bool:
        """ChatGPT shows a blue voice button in the same spot as Send when the box is empty."""
        try:
            blob = el.evaluate(
                "e => ((e.getAttribute('aria-label')||'') + ' ' + (e.getAttribute('data-testid')||'')).toLowerCase()"
            )
        except Exception:  # noqa: BLE001
            return False
        return isinstance(blob, str) and any(w in blob for w in self.VOICE_WORDS)

    def _first_visible(self, key: str):
        for sel in self.cfg.selectors[key]:
            try:
                loc = self.page.locator(sel)
                if loc.count():
                    first = loc.first
                    if first.is_visible():
                        if key == "send" and self._is_voice_button(first):
                            continue
                        return first
            except Exception:  # noqa: BLE001
                continue
        return None

    def _composer_len(self) -> int:
        comp = self._first_visible("composer")
        if comp is None:
            return -1
        try:
            return int(comp.evaluate("e => (e.innerText || e.value || '').trim().length"))
        except Exception:  # noqa: BLE001
            return -1

    def _streaming(self) -> bool:
        return self._first_visible("stop") is not None

    # ChatGPT's markup changes; if the configured selectors match nothing, these are tried
    # and whichever matches the most elements is used (each one on its own, never merged).
    FALLBACKS = {
        "assistant": [
            "[data-turn='assistant']",
            "[data-testid^='conversation-turn'][data-turn='assistant']",
            "div.markdown.prose",
            "div.markdown",
            ".agent-turn",
        ],
        "user": [
            "[data-turn='user']",
            "button[aria-label='Edit message']",
            "button[aria-label='Copy message']",
        ],
    }

    def _candidates(self, key: str) -> list:
        configured = list(self.cfg.selectors.get(key) or [])
        out = [", ".join(configured)] if configured else []
        out += [f for f in self.FALLBACKS.get(key, []) if f not in configured]
        return out

    def _best_sel(self, key: str) -> str:
        best, best_n = "", -1
        for sel in self._candidates(key):
            try:
                n = int(self.page.evaluate(COUNT_JS, sel))
            except Exception:  # noqa: BLE001
                continue
            if n > best_n:
                best, best_n = sel, n
        return best or self._sel(key)

    # ---- reading the reply from the page's own stream ----------------------
    def _stream_records(self) -> list:
        try:
            recs = self.page.evaluate(STREAM_STATE_JS)
        except Exception:  # noqa: BLE001
            return []
        if not isinstance(recs, list):
            return []
        return [r for r in recs if isinstance(r, dict) and "text" in r and "done" in r]

    def _stream_count(self) -> int:
        return len(self._stream_records())

    def _stream_reply(self, s0: int):
        """(ReplyStream, still_open) for the newest reply stream after the first s0, or (None, False)."""
        recs = self._stream_records()
        if len(recs) <= s0:
            return None, False
        rec = recs[-1]
        try:
            return ReplyStream().feed(rec.get("text") or ""), not rec.get("done")
        except Exception:  # noqa: BLE001
            return None, False

    def _count(self) -> int:
        best = 0
        for sel in self._candidates("assistant"):
            try:
                best = max(best, int(self.page.evaluate(COUNT_JS, sel)))
            except Exception:  # noqa: BLE001
                continue
        return best

    def _extract(self, start: int) -> str:
        raw = self.page.evaluate(
            EXTRACT_JS, {"sel": self._best_sel("assistant"), "start": start}
        )
        return tidy(raw or "")

    def _wait_ready(self, max_wait: int = 600) -> None:
        t0 = time.time()
        told = False
        while time.time() - t0 < max_wait:
            if self._first_visible("composer") is not None:
                return
            if not told and time.time() - t0 > 8:
                ui.warn(
                    "Waiting for the ChatGPT message box... complete any login or "
                    "verification step in the browser window yourself; WebCodex waits "
                    "and does not try to get around it."
                )
                told = True
            time.sleep(1)
        raise TransportError("Timed out waiting for the ChatGPT composer to appear.")

    # ---- public API -----------------------------------------------------
    def _logged_in(self) -> bool:
        if self._first_visible("login_button") is not None:
            return False
        if self._first_visible("logged_in") is not None:
            return True
        try:
            names = [c["name"] for c in self._ctx.cookies(self.cfg.url)]
        except Exception:  # noqa: BLE001
            names = []
        return any(n.startswith("__Secure-next-auth.session-token") for n in names)

    def _ensure_logged_in(self, strict: bool) -> bool:
        """Waits for a real login. strict=True (--login) waits up to 10 min and
        returns False if it could not confirm. strict=False warns and continues
        after 2 min, in case detection is wrong because the UI changed."""
        limit = 600 if strict else 120
        t0 = time.time()
        told = False
        while time.time() - t0 < limit:
            if self._logged_in():
                return True
            if not told and time.time() - t0 > 6:
                ui.warn(
                    "You are NOT logged in to ChatGPT. Log in in the browser window "
                    "(waiting up to %d min)..." % (limit // 60)
                )
                told = True
            time.sleep(1)
        if not strict:
            ui.warn("Could not confirm a ChatGPT login; continuing anyway (anonymous or UI changed).")
        return False

    def new_chat(self, check_login: bool = True) -> None:
        cfg = self.cfg
        params = []
        if cfg.temporary:
            params.append("temporary-chat=true")
        if cfg.model:
            params.append(f"model={cfg.model}")
        url = cfg.url
        if params:
            url += ("&" if "?" in url else "?") + "&".join(params)
        self.page.goto(url, wait_until="domcontentloaded")
        if check_login:
            self._ensure_logged_in(strict=False)
        self._wait_ready()
        self._wait_hydrated()

    def _wait_hydrated(self, max_wait: float = 25.0) -> None:
        """chatgpt.com first shows a plain placeholder box (#pending-home-input) whose
        Send/Dictate buttons are disabled until the page finishes loading. Text typed
        before that can be lost, so wait until the page is really interactive."""
        try:
            self.page.wait_for_load_state("networkidle", timeout=int(max_wait * 1000))
        except Exception:  # noqa: BLE001
            pass
        t0 = time.time()
        while time.time() - t0 < max_wait:
            try:
                dictate = self.page.locator("button[aria-label='Dictate']")
                if not dictate.count():
                    return  # no placeholder composer on this page
                if dictate.first.is_enabled():
                    time.sleep(0.5)
                    return
            except Exception:  # noqa: BLE001
                return
            time.sleep(0.3)

    # ---- resuming a chat ------------------------------------------------
    CHAT_URL_RE = re.compile(r"/c/[0-9A-Za-z-]{8,}")

    def current_url(self) -> Optional[str]:
        """URL of the open conversation (https://chatgpt.com/c/<id>), or None.
        Temporary chats and not-yet-started chats have no such URL."""
        try:
            url = self.page.url
        except Exception:  # noqa: BLE001
            return None
        return url if self.CHAT_URL_RE.search(url or "") else None

    def _wait_history_settled(self, max_wait: float = 20.0) -> None:
        """An old chat renders its messages a moment after the composer appears.
        Wait until the number of assistant messages stops changing, otherwise the
        next reply would be extracted together with the old ones."""
        t0 = time.time()
        last, last_change = -1, time.time()
        while time.time() - t0 < max_wait:
            n = self._count()
            if n != last:
                last, last_change = n, time.time()
            elif time.time() - last_change >= 2.0:
                return
            time.sleep(0.3)

    def open_chat(self, url: str) -> bool:
        """Reopen an existing conversation. False if it can't be opened
        (deleted chat, logged out, redirected to the home page)."""
        try:
            self.page.goto(url, wait_until="domcontentloaded")
            self._ensure_logged_in(strict=False)
            self._wait_ready()
            self._wait_history_settled()
        except Exception:  # noqa: BLE001
            return False
        return self.current_url() is not None

    def login(self) -> bool:
        self.page.goto(self.cfg.url, wait_until="domcontentloaded")
        return self._ensure_logged_in(strict=True)

    # ---- putting the prompt in the message box -----------------------------
    MOD = "Meta" if sys.platform == "darwin" else "Control"

    def _registered(self) -> bool:
        """True once ChatGPT itself knows there is a message: the Send button is
        enabled. Text that is merely *visible* in the box can still leave Send dead."""
        btn = self._first_visible("send")
        if btn is None:
            return False
        try:
            return bool(btn.is_enabled())
        except Exception:  # noqa: BLE001
            return True

    def _clear_composer(self, comp) -> None:
        try:
            comp.click()
            self.page.keyboard.press(f"{self.MOD}+A")
            self.page.keyboard.press("Delete")
        except Exception:  # noqa: BLE001
            pass

    def _into_clipboard(self, text: str) -> bool:
        try:  # browser clipboard (needs permission, granted below)
            if self._ctx is not None:
                try:
                    self._ctx.grant_permissions(["clipboard-read", "clipboard-write"],
                                                origin=self.cfg.url.rstrip("/"))
                except Exception:  # noqa: BLE001
                    pass
            self.page.evaluate("t => navigator.clipboard.writeText(t)", text)
            return True
        except Exception:  # noqa: BLE001
            return copy_to_clipboard(text)  # OS clipboard as a fallback

    def _m_keyboard_paste(self, comp, text: str) -> None:
        """A real Ctrl/Cmd+V: the page sees a trusted paste, same as a person pasting."""
        if not self._into_clipboard(text):
            return
        comp.click()
        self.page.keyboard.press(f"{self.MOD}+V")

    def _m_insert_text(self, comp, text: str) -> None:
        comp.click()
        self.page.keyboard.insert_text(text)

    def _m_synthetic_paste(self, comp, text: str) -> None:
        comp.evaluate(PASTE_JS, text)

    def _m_fill(self, comp, text: str) -> None:
        comp.fill(text)

    def _set_text(self, text: str) -> None:
        comp = self._first_visible("composer")
        if comp is None:
            raise TransportError("Could not find the ChatGPT message box.")
        comp.click()
        tag = comp.evaluate("e => e.tagName.toLowerCase()")
        methods = [self._m_fill] if tag == "textarea" else [
            self._m_keyboard_paste, self._m_insert_text, self._m_synthetic_paste, self._m_fill,
        ]
        for i, method in enumerate(methods):
            if i:
                self._clear_composer(comp)
            try:
                method(comp, text)
            except Exception:  # noqa: BLE001
                continue
            t0 = time.time()
            while time.time() - t0 < 4:  # big pastes take a moment to register
                if self._registered():
                    return
                time.sleep(0.25)
        ui.warn(
            "ChatGPT's Send button never became active after inserting the prompt. "
            "Trying to send anyway; if nothing happens, click in the message box and "
            "press Enter yourself."
        )

    # ---- submitting ------------------------------------------------------
    def _count_key(self, key: str) -> int:
        cands = self._candidates(key) if key in self.FALLBACKS else (
            [", ".join(self.cfg.selectors.get(key) or [])] if self.cfg.selectors.get(key) else []
        )
        best = 0
        for sel in cands:
            try:
                best = max(best, int(self.page.evaluate(COUNT_JS, sel)))
            except Exception:  # noqa: BLE001
                continue
        return best

    def _try_click_send(self, wait: float = 15.0) -> None:
        """Click the Send button once it is there and enabled (big pastes take a moment)."""
        t0 = time.time()
        while time.time() - t0 < wait:
            self._check_cancel()
            btn = self._first_visible("send")
            if btn is not None:
                try:
                    if btn.is_enabled():
                        try:
                            btn.click(timeout=3000)
                        except Exception:  # noqa: BLE001
                            try:
                                btn.click(force=True, timeout=3000)
                            except Exception:  # noqa: BLE001
                                btn.evaluate("e => e.click()")
                        return
                except Exception:  # noqa: BLE001
                    pass
            time.sleep(0.4)

    def _press(self, combo: str) -> None:
        comp = self._first_visible("composer")
        if comp is None:
            return
        try:
            comp.click()
            self.page.keyboard.press(combo)
        except Exception:  # noqa: BLE001
            pass

    def _dump_debug(self) -> Optional[Path]:
        """Save a screenshot and a list of the page's buttons so selectors can be fixed."""
        try:
            d = Path(self.cfg.profile_dir).parent / "debug"
            d.mkdir(parents=True, exist_ok=True)
            stamp = time.strftime("%Y%m%d-%H%M%S")
            try:
                self.page.screenshot(path=str(d / f"send-{stamp}.png"))
            except Exception:  # noqa: BLE001
                pass
            info = self.page.evaluate(
                """() => Array.from(document.querySelectorAll('button, [role=button]')).slice(0, 80).map(b => ({
                    testid: b.getAttribute('data-testid'), aria: b.getAttribute('aria-label'),
                    disabled: b.disabled || b.getAttribute('aria-disabled'),
                    text: (b.innerText || '').trim().slice(0, 40)}))"""
            )
            lines = [f"url: {getattr(self.page, 'url', '')}"]
            try:
                comp = self.page.evaluate(
                    """(sel) => { const e = document.querySelector(sel); if (!e) return 'composer not found';
                       return JSON.stringify({tag: e.tagName, id: e.id, contenteditable: e.getAttribute('contenteditable'),
                         textLength: (e.innerText || e.value || '').length, html: e.outerHTML.slice(0, 600)}); }""",
                    ", ".join(self.cfg.selectors["composer"]),
                )
                lines.append(f"composer: {comp}")
            except Exception:  # noqa: BLE001
                pass
            try:
                sel_counts = {
                    k: self._count_key(k) for k in ("user", "assistant", "stop", "send", "composer")
                }
                lines.append(f"selector counts: {sel_counts}")
                facts = self.page.evaluate(
                    """() => {
                      const uniq = (sel, attr) => Array.from(new Set(Array.from(document.querySelectorAll(sel)).map(e => e.getAttribute(attr))));
                      const edit = document.querySelector("button[aria-label='Edit message']");
                      const chain = [];
                      for (let e = edit; e && chain.length < 8; e = e.parentElement)
                        chain.push(e.tagName + ' ' + Array.from(e.attributes).filter(a => a.name !== 'class' && a.name !== 'style').map(a => a.name + '=' + a.value.slice(0, 40)).join(' '));
                      const rate = document.querySelector("button[aria-label='Rate response'], button[aria-label='Regenerate response']");
                      const chain2 = [];
                      for (let e = rate; e && chain2.length < 8; e = e.parentElement)
                        chain2.push(e.tagName + ' ' + Array.from(e.attributes).filter(a => a.name !== 'class' && a.name !== 'style').map(a => a.name + '=' + a.value.slice(0, 40)).join(' '));
                      return {
                        author_roles: uniq('[data-message-author-role]', 'data-message-author-role'),
                        data_turn: uniq('[data-turn]', 'data-turn'),
                        turn_testids: uniq('[data-testid^=conversation-turn]', 'data-testid').slice(0, 10),
                        user_ancestors: chain, assistant_ancestors: chain2,
                      };
                    }"""
                )
                lines.append(f"dom facts: {facts}")
            except Exception as exc:  # noqa: BLE001
                lines.append(f"dom facts failed: {exc}")
            lines += [str(x) for x in (info or [])]
            (d / f"send-{stamp}.txt").write_text("\n".join(lines), encoding="utf-8")
            return d
        except Exception:  # noqa: BLE001
            return None

    def _submit(self, users0: int, asst0: int, stream0: int = 0) -> bool:
        """Get the prompt sent and CONFIRM it went out. Returns False if nothing worked."""
        url0 = getattr(self.page, "url", "") or ""

        def sent() -> bool:
            url_now = getattr(self.page, "url", "") or ""
            # a brand-new chat moves from the home page to /c/<id> once the message is accepted
            moved_to_chat = bool(self.CHAT_URL_RE.search(url_now)) and not self.CHAT_URL_RE.search(url0)
            return (
                self._streaming()
                or self._stream_count() > stream0
                or self._count() > asst0
                or self._count_key("user") > users0
                or moved_to_chat
            )

        steps = (
            self._try_click_send,
            lambda: self._press("Enter"),
            lambda: self._press("Control+Enter"),
        )
        for step in steps:
            self._check_cancel()
            step()
            t0 = time.time()
            while time.time() - t0 < 6:
                self._check_cancel()
                if sent():
                    return True
                time.sleep(0.3)
        d = self._dump_debug()
        ui.warn(
            "WebCodex could not press Send. The prompt is in the ChatGPT message box: "
            "press Enter / click Send in the browser window yourself and WebCodex will "
            "keep waiting."
            + (f"\n  Debug info (screenshot + button list) saved in {d}" if d else "")
        )
        return False

    def send(self, text: str, on_update=None, cancelled=None) -> str:
        self._cancelled = cancelled
        self._check_cancel()
        if self._pending_reply is not None:
            raise TransportError(
                "A previous response remains unresolved. Recover it before sending another prompt."
            )
        n0 = self._count()
        u0 = self._count_key("user")
        s0 = self._stream_count()
        self._set_text(text)
        self._pending_reply = (n0, s0)
        ok = self._submit(u0, n0, s0)
        # An error after submission must not cause an automatic resend.
        answer = self._wait_reply(n0, on_update, start_timeout=None if ok else 600, s0=s0)
        self._pending_reply = None
        self._cancelled = None
        return answer

    def recover(self, on_update=None, cancelled=None) -> str:
        self._cancelled = cancelled
        self._check_cancel()
        """Read an interrupted reply without submitting another prompt.

        Recovery requires a recorded response stream from the pending send.
        DOM-only text is insufficient evidence for executing model instructions.
        """
        if self._pending_reply is None:
            raise TransportError("No interrupted response is available to recover.")
        n0, s0 = self._pending_reply
        if self._stream_count() <= s0:
            raise TransportError(
                "No verified response stream was found for the pending prompt. "
                "Submission status is uncertain; refusing to resend automatically."
            )
        answer = self._wait_reply(n0, on_update, s0=s0)
        self._pending_reply = None
        self._cancelled = None
        return answer

    def _thinking_text(self) -> str:
        sels = self.cfg.selectors.get("thinking") or []
        if not sels:
            return ""
        try:
            return self.page.evaluate(
                "(sel) => Array.from(document.querySelectorAll(sel)).map(e => e.innerText).join('\\n')",
                ", ".join(sels),
            ) or ""
        except Exception:  # noqa: BLE001
            return ""

    def _stop_generation(self) -> None:
        try:
            btn = self._first_visible("stop")
            if btn is not None:
                btn.click()
        except Exception:  # noqa: BLE001
            pass

    def _clear_status(self) -> None:
        sys.stdout.write("\r" + " " * 60 + "\r")
        sys.stdout.flush()

    def _wait_reply(self, n0: int, on_update=None, start_timeout=None, s0: int = 0) -> str:
        cfg = self.cfg
        start_limit = start_timeout or cfg.start_timeout
        t0 = time.time()
        try:
            while time.time() - t0 < start_limit:
                self._check_cancel()
                if self._stream_count() > s0 or self._count() > n0 or self._streaming():
                    break
                if self._first_visible("composer") is None:
                    # Login / verification page: hand control to the user, then keep waiting.
                    self._wait_ready()
                    t0 = time.time()
                time.sleep(0.4)
            else:
                d = self._dump_debug()
                if d:
                    ui.warn(f"Debug info saved in {d}")
                raise TransportError(
                    "ChatGPT did not start replying. Check the browser window "
                    "(logged out? rate limit? captcha? changed UI - see README 'Selectors')."
                )
            last_text, last_change = "", time.time()
            think_printed = ""
            streamed = False
            deadline = time.time() + cfg.reply_timeout
            while time.time() < deadline:
                self._check_cancel()
                streaming = self._streaming()
                rs, open_ = self._stream_reply(s0)
                text = rs.text if rs is not None else ""
                if rs is not None and open_:
                    streaming = True  # the page's reply stream is still running
                if rs is None:
                    text = self._extract(n0)
                elif not open_ and not rs.finished:
                    raise TransportError("ChatGPT response stream ended before completion.")
                now = time.time()
                if text != last_text:
                    last_text, last_change = text, now
                    if on_update and text:
                        if not streamed:
                            self._clear_status()
                            streamed = True
                        on_update(text)
                if not streamed:
                    think = self._thinking_text()
                    if think and think != think_printed and think.startswith(think_printed):
                        self._clear_status()
                        ui.dim(think[len(think_printed):].rstrip("\n"))
                        think_printed = think
                    sys.stdout.write(
                        f"\r{ui.C.dim}  ChatGPT is thinking... {int(now - t0)}s{ui.C.reset}   "
                    )
                    sys.stdout.flush()
                cont = self._first_visible("continue") if not streaming else None
                if cont is not None:
                    try:
                        cont.click()
                        last_change = now
                    except Exception:  # noqa: BLE001
                        pass
                    time.sleep(0.5)
                    continue
                if not streaming and text and now - last_change >= cfg.stable_secs:
                    if rs is None and re.search(
                        r"(?m)^\s*(?:FILE:|<<<<<<< SEARCH|>>>>>>> REPLACE|DELETE:|RUN:)",
                        text,
                    ):
                        raise TransportError(
                            "Executable instructions were recovered from DOM text, "
                            "not a verified response stream. Refusing to execute them."
                        )
                    if rs is not None and not rs.finished:
                        raise TransportError(
                            "Response stream has not completed. Refusing a partial reply."
                        )
                    if not streamed:
                        self._clear_status()
                    return text
                time.sleep(0.3)
            raise TransportError("Timed out waiting for ChatGPT to finish replying.")
        except KeyboardInterrupt:
            self._clear_status()
            self._stop_generation()
            raise


def copy_to_clipboard(text: str) -> bool:
    try:
        if sys.platform.startswith("win"):
            p = subprocess.run(["clip"], input=text.encode("utf-16le"), check=True)
            return p.returncode == 0
        if sys.platform == "darwin":
            subprocess.run(["pbcopy"], input=text.encode("utf-8"), check=True)
            return True
        for cmd in (["wl-copy"], ["xclip", "-selection", "clipboard"], ["xsel", "-b", "-i"]):
            if shutil.which(cmd[0]):
                subprocess.run(cmd, input=text.encode("utf-8"), check=True)
                return True
    except Exception:  # noqa: BLE001
        pass
    return False


class ManualTransport:
    """Fallback: you paste the prompt into ChatGPT yourself, then paste the reply back."""

    def __init__(self, cfg: Config, root: Path):
        self.dir = root / ".webcodex"

    def start(self) -> None:
        pass

    def new_chat(self) -> None:
        ui.info("Open a NEW chat in ChatGPT, then continue.")

    def close(self) -> None:
        pass

    def send(self, text: str, on_update=None) -> str:
        self.dir.mkdir(parents=True, exist_ok=True)
        path = self.dir / "last_prompt.txt"
        path.write_text(text, encoding="utf-8")
        copied = copy_to_clipboard(text)
        ui.info(
            f"Prompt {'copied to clipboard and ' if copied else ''}saved to {path}.\n"
            "Paste it into ChatGPT, copy the whole reply, paste it here, "
            "then type EOF on its own line."
        )
        lines = []
        while True:
            try:
                line = input()
            except EOFError:
                break
            if line.strip() == "EOF":
                break
            lines.append(line)
        return "\n".join(lines)


class DryTransport:
    """Prints the prompt that would be sent, sends nothing."""

    def start(self) -> None:
        pass

    def new_chat(self) -> None:
        pass

    def close(self) -> None:
        pass

    def send(self, text: str, on_update=None) -> str:
        print("=" * 70)
        print(text)
        print("=" * 70)
        print(f"[dry run] {len(text)} characters would be sent.")
        return ""
