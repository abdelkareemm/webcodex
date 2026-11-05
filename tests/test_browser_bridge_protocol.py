"""Tests for the browser bridge's newline-delimited JSON protocol."""

import json
import subprocess
import sys
import unittest


def exchange(requests):
    payload = "".join(line + "\n" for line in requests)
    result = subprocess.run(
        [sys.executable, "-m", "webcodex.browser_bridge"],
        input=payload,
        capture_output=True,
        text=True,
        encoding="utf-8",
        timeout=15,
        check=True,
    )
    return [json.loads(line) for line in result.stdout.splitlines()]


class BrowserBridgeProtocolTests(unittest.TestCase):
    def test_uninitialized_bridge_reports_error_and_closes(self):
        messages = exchange([
            json.dumps({"op": "current_url"}),
            json.dumps({"op": "close"}),
        ])
        self.assertEqual(len(messages), 2)
        self.assertFalse(messages[0]["ok"])
        self.assertIn("has not started", messages[0]["error"])
        self.assertEqual(messages[1], {"kind": "result", "ok": True})

    def test_malformed_json_does_not_terminate_bridge(self):
        messages = exchange([
            "{invalid json}",
            json.dumps({"op": "close"}),
        ])
        self.assertEqual(len(messages), 2)
        self.assertFalse(messages[0]["ok"])
        self.assertIn("JSONDecodeError", messages[0]["error"])
        self.assertEqual(messages[1], {"kind": "result", "ok": True})


if __name__ == "__main__":
    unittest.main()
