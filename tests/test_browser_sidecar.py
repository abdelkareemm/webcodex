"""Tests the JSON protocol without launching a real browser."""
import contextlib
import io
import json
import sys
import unittest
from unittest.mock import patch
from webcodex import browser_bridge


class FakeBrowser:
    def __init__(self, config):
        self.config = config
    def start(self): pass
    def close(self): pass
    def send(self, text, on_update=None):
        on_update("first")
        on_update("first and second")
        return "first and second"


class SidecarTests(unittest.TestCase):
    def test_streamed_updates_have_separate_result(self):
        commands = '\n'.join([
            json.dumps({'op': 'start', 'config': {'headless': True}}),
            json.dumps({'op': 'send', 'text': 'hello'}),
            json.dumps({'op': 'close'}),
        ]) + '\n'
        output = io.StringIO()
        with patch('webcodex.browser_bridge.BrowserTransport', FakeBrowser), \
             patch.object(sys, 'stdin', io.StringIO(commands)), \
             patch.object(sys, '__stdout__', output), \
             contextlib.redirect_stderr(io.StringIO()):
            self.assertEqual(browser_bridge.main(), 0)
        events = [json.loads(line) for line in output.getvalue().splitlines()]
        self.assertEqual([event['kind'] for event in events if event['kind']!='status'],
                         ['result', 'update', 'update', 'result', 'result'])
        self.assertEqual([event for event in events if event['kind']=='result'][1]['text'], 'first and second')

if __name__ == '__main__': unittest.main()
