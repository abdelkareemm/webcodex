"""Static checks for Rust UI changes; Cargo on Windows runs behavioral Rust tests."""
from pathlib import Path
import unittest

ROOT=Path(__file__).resolve().parents[1]
TUI=(ROOT/'rust-tui/src/main.rs').read_text(encoding='utf-8')
NATIVE=(ROOT/'rust-core/src/bin/webcodex-native.rs').read_text(encoding='utf-8')
BRIDGE=(ROOT/'webcodex/browser_bridge.py').read_text(encoding='utf-8')

class QoLContract(unittest.TestCase):
    def test_history_and_scrolling(self):
        for token in ('fn recall_prev(', 'fn recall_next(', 'fn jump_bottom(', 'ScrollbarState::new(', 'KeyCode::PageUp', 'KeyCode::End if app.draft.is_empty()', 'fn history_up_down_restores_unsent_draft()', 'fn scrollback_pauses_follow_and_bottom_resumes()'):
            self.assertIn(token,TUI)
    def test_code_suppressed_in_all_layers(self):
        for src in (TUI,NATIVE):
            self.assertIn('t.starts_with("FILE:")',src)
            self.assertIn('t.starts_with("<pre")',src)
            self.assertIn('count>=3' if src is NATIVE else 'n>=3',src)
    def test_startup_progress(self):
        self.assertLess(NATIVE.index('let mut ui=Ui::new(&root,o.plain||o.login);'), NATIVE.index('let mut browser=Browser::spawn('))
        self.assertIn('ui.event("ready","")',NATIVE)
        self.assertIn('status(\'Waiting for the ChatGPT message box…\')',BRIDGE)

if __name__=='__main__':unittest.main()
