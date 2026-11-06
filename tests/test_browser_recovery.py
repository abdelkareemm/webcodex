import unittest

from webcodex.config import Config
from webcodex.transports import BrowserTransport, TransportError


class BrowserRecoveryTests(unittest.TestCase):
    def test_send_refuses_unresolved_request_without_submitting(self):
        transport = BrowserTransport(Config())
        transport._pending_reply = (2, 3)
        attempted = []

        def unexpected_submit(*args, **kwargs):
            attempted.append(True)
            raise AssertionError("A second message must not be submitted")

        transport._set_text = unexpected_submit

        with self.assertRaisesRegex(TransportError, "previous response remains unresolved"):
            transport.send("Duplicate prompt")

        self.assertEqual(attempted, [])
        self.assertEqual(transport._pending_reply, (2, 3))

    def test_unconfirmed_send_does_not_retry_with_keyboard(self):
        from unittest.mock import patch
        from itertools import count

        transport = BrowserTransport(Config())
        transport.page = type("FakePage", (), {"url": "https://chatgpt.com/"})()
        transport._check_cancel = lambda: None
        transport._streaming = lambda: False
        transport._stream_count = lambda: 0
        transport._count = lambda: 0
        transport._count_key = lambda key: 0
        transport._dump_debug = lambda: None

        clicks = []
        keyboard = []
        transport._try_click_send = lambda: clicks.append("click")
        transport._press = lambda key: keyboard.append(key)

        clock = count(0, 7)
        with patch("webcodex.transports.time.time", side_effect=lambda: next(clock)), \
             patch("webcodex.transports.ui.warn"):
            confirmed = transport._submit(0, 0, 0)

        self.assertFalse(confirmed)
        self.assertEqual(clicks, ["click"])
        self.assertEqual(keyboard, [])

    def test_failed_click_is_not_retried(self):
        transport = BrowserTransport(Config())
        transport._check_cancel = lambda: None

        class UncertainButton:
            attempts = 0

            def is_enabled(self):
                return True

            def click(self, **kwargs):
                self.attempts += 1
                raise RuntimeError("Click outcome uncertain")

        button = UncertainButton()
        transport._first_visible = lambda key: button

        transport._try_click_send(wait=0.1)

        self.assertEqual(button.attempts, 1)

    def test_recovery_requires_pending_response(self):
        transport = BrowserTransport(Config())

        with self.assertRaisesRegex(TransportError, "No interrupted response"):
            transport.recover()

    def test_recovery_refuses_missing_stream(self):
        transport = BrowserTransport(Config())
        transport._pending_reply = (2, 3)
        transport._stream_count = lambda: 3

        with self.assertRaisesRegex(TransportError, "refusing to resend"):
            transport.recover()

        self.assertEqual(transport._pending_reply, (2, 3))

    def test_recovery_reads_existing_stream_once(self):
        transport = BrowserTransport(Config())
        transport._pending_reply = (4, 5)
        transport._stream_count = lambda: 6
        observed = []

        def read_reply(n0, on_update=None, s0=0):
            observed.append((n0, s0))
            return "Recovered complete reply"

        transport._wait_reply = read_reply

        self.assertEqual(transport.recover(), "Recovered complete reply")
        self.assertEqual(observed, [(4, 5)])
        self.assertIsNone(transport._pending_reply)


if __name__ == "__main__":
    unittest.main()
