"""Tests cooperative cancellation of ChatGPT generation."""
import unittest

from webcodex.transports import BrowserTransport, TransportCancelled


class TransportCancellationTests(unittest.TestCase):
    def test_cancel_stops_generation_and_discards_pending_reply(self):
        transport = BrowserTransport.__new__(BrowserTransport)
        stopped = []
        transport._cancelled = lambda: True
        transport._pending_reply = (3, 2)
        transport._stop_generation = lambda: stopped.append(True)

        with self.assertRaises(TransportCancelled):
            transport._check_cancel()

        self.assertEqual(stopped, [True])
        self.assertIsNone(transport._pending_reply)

    def test_without_cancel_generation_continues(self):
        transport = BrowserTransport.__new__(BrowserTransport)
        stopped = []
        transport._cancelled = lambda: False
        transport._pending_reply = (3, 2)
        transport._stop_generation = lambda: stopped.append(True)

        transport._check_cancel()

        self.assertEqual(stopped, [])
        self.assertEqual(transport._pending_reply, (3, 2))


if __name__ == "__main__":
    unittest.main()
