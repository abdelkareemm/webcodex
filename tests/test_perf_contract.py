"""Source-level contracts for the native performance improvements.

Rust unit tests in rust-tui/src/main.rs verify cache and history semantics.
These checks guard against accidentally restoring the old per-frame parser.
"""
import pathlib
import unittest

ROOT = pathlib.Path(__file__).resolve().parents[1]
TUI = (ROOT / 'rust-tui/src/main.rs').read_text(encoding='utf-8')
NATIVE = (ROOT / 'rust-core/src/bin/webcodex-native.rs').read_text(encoding='utf-8')


class TranscriptPerformanceTests(unittest.TestCase):
    def test_scroll_uses_cached_viewport(self):
        self.assertIn('cached_rows: Vec<Line', TUI)
        self.assertIn('if !app.transcript_dirty && app.cache_width==width {return;}', TUI)
        self.assertIn('app.cached_rows[start..end].to_vec()', TUI)
        self.assertIn('fn cached_transcript_does_not_rebuild_for_scroll()', TUI)

    def test_redraw_is_event_driven(self):
        self.assertIn('if redraw {', TUI)
        self.assertIn('terminal.draw(|f|render(f,&mut app))?', TUI)
        self.assertNotIn('terminal.draw(|f|render(f,&mut app))?;\n        if !event::poll', TUI)

    def test_resume_uses_one_history_event(self):
        self.assertIn('ui.event("history",&batch)', NATIVE)
        self.assertIn('"history"=>{if let Ok(messages)', TUI)
        self.assertIn('"limit":50', NATIVE)
        self.assertNotIn('"user"=>ui.event("user",content)', NATIVE)


if __name__ == '__main__':
    unittest.main()
