"""Static wiring checks for source that is compiled by Cargo on Windows."""
import unittest
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]


class TerminalDisplayContract(unittest.TestCase):
    def test_native_stream_is_filtered_before_ui(self):
        src = (ROOT / "rust-core/src/bin/webcodex-native.rs").read_text(encoding="utf-8")
        self.assertIn('ui.event("stream",&clean_reply(text))', src)
        self.assertIn('let visible=clean_reply(text)', src)
        self.assertIn('let display=clean_reply(reply)', src)

    def test_tui_filters_code_and_shades_user_rows(self):
        src = (ROOT / "rust-tui/src/main.rs").read_text(encoding="utf-8")
        self.assertIn('sanitize_display(&hide_code_blocks(&text))', src)
        self.assertIn('lines.extend(user_message_rows(', src)
        self.assertIn('bg(Color::Rgb(48,48,48))', src)
        self.assertIn('fn user_long_messages_wrap_without_overflow()', src)


if __name__ == '__main__':
    unittest.main()
