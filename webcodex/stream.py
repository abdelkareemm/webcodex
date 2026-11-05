"""Reads ChatGPT's reply from the page's own conversation stream instead of scraping the DOM.

The chatgpt.com front end POSTs to /backend-api/f/conversation and receives a server-sent-event
stream ("delta_encoding: v1"). Each event is a JSON patch against the message being built:

    event: delta
    data: {"p":"","o":"add","v":{"message":{...,"author":{"role":"assistant"},"channel":"final",...}}}
    data: {"o":"patch","v":[{"p":"/message/content/parts/0","o":"append","v":"Hey!"}, ...]}
    data: {"v":[{"p":"/message/content/parts/0","o":"append","v":"?"}]}
    data: {"v":"more text"}                      <- bare string: append to the last path used
    data: {"type":"message_stream_complete"}

Thinking models also emit hidden messages (model_editable_context, reasoning_recap, analysis
channel). Only assistant text that the page would actually show as the answer is returned.
This module is pure Python so it can be tested without a browser.
"""
from __future__ import annotations

import json
from typing import Any, Dict, List, Optional, Tuple

HIDDEN_CHANNELS = {"analysis", "commentary"}


def _split_path(path: str) -> List[str]:
    return [p.replace("~1", "/").replace("~0", "~") for p in path.split("/") if p != ""]


def _walk(root: Any, parts: List[str]) -> Tuple[Any, Optional[str]]:
    """Return (container, last_key) for a JSON-pointer-like path, or (None, None)."""
    cur = root
    for key in parts[:-1]:
        if isinstance(cur, list):
            try:
                cur = cur[int(key)]
            except (ValueError, IndexError):
                return None, None
        elif isinstance(cur, dict):
            if key not in cur or cur[key] is None:
                cur[key] = {}
            cur = cur[key]
        else:
            return None, None
    return cur, (parts[-1] if parts else None)


def _apply(msg_wrapper: dict, path: str, op: str, value: Any) -> None:
    parts = _split_path(path)
    if parts and parts[0] == "message":
        target = msg_wrapper
    else:
        return
    cont, key = _walk(target, parts)
    if cont is None or key is None:
        return
    if isinstance(cont, list):
        try:
            idx = int(key)
        except ValueError:
            return
        while op in ("append", "replace", "add") and idx >= len(cont):
            cont.append("")
        old = cont[idx] if idx < len(cont) else None
        if op == "remove":
            if idx < len(cont):
                cont.pop(idx)
            return
        cont[idx] = _combine(old, op, value)
        return
    old = cont.get(key)
    if op == "remove":
        cont.pop(key, None)
        return
    cont[key] = _combine(old, op, value)


def _combine(old: Any, op: str, value: Any) -> Any:
    if op in ("replace", "add", "set"):
        return value
    # append
    if old is None:
        return value
    if isinstance(old, str) and isinstance(value, str):
        return old + value
    if isinstance(old, list):
        return old + (value if isinstance(value, list) else [value])
    if isinstance(old, dict) and isinstance(value, dict):
        merged = dict(old)
        merged.update(value)
        return merged
    return value


class ReplyStream:
    """Incrementally feed raw SSE text; read .text / .complete at any time."""

    def __init__(self) -> None:
        self.messages: Dict[str, dict] = {}
        self.order: List[str] = []
        self.current: Optional[str] = None
        self.last_path: str = ""
        self.last_op: str = "append"
        self.complete: bool = False
        self.conversation_id: Optional[str] = None
        self.error: Optional[str] = None

    # ---- feeding ----------------------------------------------------------
    def feed(self, raw: str) -> "ReplyStream":
        for block in raw.replace("\r\n", "\n").split("\n\n"):
            for line in block.split("\n"):
                if not line.startswith("data:"):
                    continue
                payload = line[5:].strip()
                if not payload or payload == "[DONE]":
                    if payload == "[DONE]":
                        self.complete = True
                    continue
                try:
                    obj = json.loads(payload)
                except ValueError:
                    continue
                self._event(obj)
        return self

    def _add_message(self, msg: dict, conv_id: Optional[str] = None) -> None:
        mid = msg.get("id") or f"anon-{len(self.order)}"
        wrapper = {"message": msg}
        if mid not in self.messages:
            self.order.append(mid)
        self.messages[mid] = wrapper
        self.current = mid
        if conv_id:
            self.conversation_id = conv_id

    def _event(self, obj: Any) -> None:
        if isinstance(obj, str):  # e.g. the "v1" of an `event: delta_encoding` line
            return
        if not isinstance(obj, dict):
            return
        typ = obj.get("type")
        if typ == "message_stream_complete":
            self.complete = True
            return
        if typ in ("resume_conversation_token", "input_message", "server_ste_metadata",
                   "message_marker", "user_signal_metadata", "conversation_detail_metadata",
                   "title_generation"):
            if obj.get("conversation_id"):
                self.conversation_id = obj["conversation_id"]
            return
        if obj.get("error"):
            self.error = str(obj["error"])
        v = obj.get("v")
        op = obj.get("o")
        path = obj.get("p")
        # a whole new message
        if isinstance(v, dict) and isinstance(v.get("message"), dict):
            self._add_message(v["message"], v.get("conversation_id"))
            return
        # a list of patch operations
        if isinstance(v, list) and (op == "patch" or op is None or path is None or path == ""):
            for item in v:
                if isinstance(item, dict) and "p" in item:
                    self._patch(item["p"], item.get("o") or "append", item.get("v"))
            return
        # one operation, or a bare value that continues the last path
        if isinstance(path, str) and path:
            self._patch(path, op or "append", v)
        elif isinstance(v, str):
            self._patch(self.last_path, self.last_op, v)

    def _patch(self, path: str, op: str, value: Any) -> None:
        if self.current is None or not path:
            return
        self.last_path, self.last_op = path, op
        _apply(self.messages[self.current], path, op, value)

    # ---- reading ----------------------------------------------------------
    @staticmethod
    def _text_of(msg: dict) -> str:
        content = msg.get("content") or {}
        if content.get("content_type") not in ("text", "multimodal_text", None):
            return ""
        parts = content.get("parts") or []
        return "".join(p for p in parts if isinstance(p, str))

    def _visible_answers(self) -> List[dict]:
        out = []
        for mid in self.order:
            msg = self.messages[mid]["message"]
            if (msg.get("author") or {}).get("role") != "assistant":
                continue
            if msg.get("recipient", "all") != "all":
                continue  # a tool call, not an answer
            if msg.get("channel") in HIDDEN_CHANNELS:
                continue
            if not self._text_of(msg):
                continue
            out.append(msg)
        return out

    @property
    def text(self) -> str:
        """Everything the assistant has said as its answer so far."""
        msgs = self._visible_answers()
        finals = [m for m in msgs if m.get("channel") == "final"]
        chosen = finals or msgs
        return "\n\n".join(self._text_of(m) for m in chosen).strip("\n")

    @property
    def finished(self) -> bool:
        """True once the stream says the answer is done (not merely that the socket closed)."""
        if self.complete:
            return True
        msgs = self._visible_answers()
        return bool(msgs) and all(m.get("status") == "finished_successfully" for m in msgs) \
            and bool(msgs[-1].get("end_turn"))


def parse(raw: str) -> ReplyStream:
    return ReplyStream().feed(raw)
