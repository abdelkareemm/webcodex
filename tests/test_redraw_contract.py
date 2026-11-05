"""Regression contracts for Windows/ConPTY terminal repaint and safe output."""
import unittest
from pathlib import Path

TUI = (Path(__file__).resolve().parents[1] / 'rust-tui/src/main.rs').read_text(encoding='utf-8')

class RedrawContract(unittest.TestCase):
    def test_full_frame_and_backend_are_cleared(self):
        self.assertIn('f.render_widget(Clear, area)', TUI)
        self.assertIn('if full_repaint {terminal.clear()?;', TUI)
        self.assertIn('terminal.clear().and_then(|_|loop_ui', TUI)

    def test_scroll_resize_and_new_transcript_force_repaint(self):
        self.assertIn('Event::Resize(_,_)=>{app.transcript_dirty=true;full_repaint=true;}', TUI)
        self.assertIn('MouseEventKind::ScrollUp=>{app.move_scroll(-3);full_repaint=true;}', TUI)
        self.assertIn('KeyCode::PageUp=>{app.move_scroll(-20);full_repaint=true;}', TUI)
        self.assertIn('"history"|"stream"|"stream_end"', TUI)

    def test_no_full_repaint_for_typing(self):
        self.assertIn('KeyCode::Char(c) if !modifiers.contains(KeyModifiers::CONTROL)=>{app.put(c);}', TUI)

    def test_transcript_avoids_last_terminal_column(self):
        self.assertIn('let transcript_area = Rect { width: rows[1].width.saturating_sub(1).max(1), ..rows[1] };', TUI)
        self.assertIn('Paragraph::new(visible),transcript_area', TUI)

    def test_untrusted_output_does_not_contain_escape_sequences(self):
        self.assertIn('fn sanitize_display(input: &str) -> String', TUI)
        self.assertIn('sanitize_display(&hide_code_blocks(&text))', TUI)
        self.assertIn('fn model_control_sequences_cannot_move_terminal_cursor()', TUI)

if __name__ == '__main__': unittest.main()
