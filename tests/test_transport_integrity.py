"""Regression checks for response-integrity safeguards."""
from pathlib import Path
import unittest


SOURCE = (
    Path(__file__).resolve().parents[1] / "webcodex" / "transports.py"
).read_text(encoding="utf-8")


class TransportIntegrityTests(unittest.TestCase):
    def test_raw_stream_is_not_overridden_by_dom(self):
        self.assertIn("if rs is None:", SOURCE)
        self.assertNotIn("if len(dom) > len(text):", SOURCE)

    def test_incomplete_stream_rejected(self):
        self.assertIn("elif not open_ and not rs.finished:", SOURCE)
        self.assertIn("if rs is not None and not rs.finished:", SOURCE)

    def test_dom_commands_are_rejected(self):
        self.assertIn("if rs is None and re.search(", SOURCE)
        self.assertIn("not a verified response stream", SOURCE)


if __name__ == "__main__":
    unittest.main()
