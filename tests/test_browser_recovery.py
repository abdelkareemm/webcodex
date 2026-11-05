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
