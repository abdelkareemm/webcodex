"""The Python-only browser boundary for the Rust-primary WebCodex agent.

This executable reads one JSON command per stdin line and emits one or more
JSON response/event lines on stdout. All browser/library diagnostics go to
stderr, preventing them from corrupting the IPC stream.
"""
from __future__ import annotations

import json
import sys
import queue
import threading
from contextlib import redirect_stdout
from pathlib import Path

from .config import Config
from .transports import BrowserTransport


def configure_pipe_encoding():
    """Windows pipe stderr defaults to a legacy code page (often cp1252).

    Redirected Playwright status text can contain emoji, which must not crash
    the browser-sidecar protocol. Keep stdout JSON UTF-8 as well.
    """
    for stream in (sys.stdout, sys.stderr, sys.__stdout__):
        if hasattr(stream, 'reconfigure'):
            stream.reconfigure(encoding='utf-8', errors='backslashreplace')


def status(message):
    emit({'kind': 'status', 'text': message})


def emit(message):
    sys.__stdout__.write(json.dumps(message, ensure_ascii=False) + '\n')
    sys.__stdout__.flush()


def main():
    configure_pipe_encoding()
    transport = None
    requests = queue.Queue()
    cancelled = threading.Event()

    def read_requests():
        for raw in sys.stdin:
            try:
                request = json.loads(raw)
                if request.get('op') == 'cancel':
                    cancelled.set()
                else:
                    requests.put(request)
            except BaseException as exc:
                requests.put({'op': '_read_error', 'error': f'{type(exc).__name__}: {exc}'})
        requests.put({'op': '_eof'})

    threading.Thread(target=read_requests, daemon=True).start()
    while True:
        request = requests.get()
        op = request.get('op')
        if op == '_eof':
            break
        try:
            if op == '_read_error':
                raise ValueError(request['error'])
            with redirect_stdout(sys.stderr):
                if op == 'start':
                    if transport is not None:
                        transport.close()
                    config = Config()
                    config.load_user_file()
                    for key, value in (request.get('config') or {}).items():
                        if key in ('browser', 'cdp', 'model', 'headless', 'temporary', 'url'):
                            setattr(config, key, value)
                        elif key == 'profile_dir':
                            config.profile_dir = Path(value)
                    status('Opening browser profile…')
                    transport = BrowserTransport(config)
                    transport.start()
                    status('Browser launched. Connecting to ChatGPT…')
                    result = {'ok': True}
                elif op == 'close':
                    if transport is not None:
                        transport.close()
                    emit({'kind': 'result', 'ok': True})
                    return 0
                elif transport is None:
                    raise RuntimeError('browser transport has not started')
                elif op == 'open_chat':
                    status('Loading saved ChatGPT conversation…')
                    result = {'ok': transport.open_chat(request['url'])}
                elif op == 'new_chat':
                    status('Waiting for the ChatGPT message box…')
                    transport.new_chat()
                    result = {'ok': True}
                elif op == 'login':
                    result = {'ok': transport.login()}
                elif op == 'current_url':
                    result = {'ok': True, 'url': transport.current_url()}
                elif op == 'send':
                    cancelled.clear()
                    def on_update(text):
                        emit({'kind': 'update', 'text': text})
                    answer = transport.send(request['text'], on_update=on_update, cancelled=cancelled.is_set)
                    result = {'ok': True, 'text': answer}
                elif op == 'recover':
                    cancelled.clear()
                    def on_update(text):
                        emit({'kind': 'update', 'text': text})
                    answer = transport.recover(on_update=on_update, cancelled=cancelled.is_set)
                    result = {'ok': True, 'text': answer}
                else:
                    raise ValueError(f'unknown browser operation: {op}')
            emit({'kind': 'result', **result})
        except BaseException as exc:
            emit({
                'kind': 'result',
                'ok': False,
                'cancelled': type(exc).__name__ == 'TransportCancelled',
                'error': f'{type(exc).__name__}: {exc}',
            })
    if transport is not None:
        transport.close()
    return 0


if __name__ == '__main__':
    raise SystemExit(main())
