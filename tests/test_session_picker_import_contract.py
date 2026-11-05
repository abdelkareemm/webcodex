"""Basic behavioral/source contracts; the Rust module also has real parser unit tests."""
import pathlib
import unittest

ROOT = pathlib.Path(__file__).resolve().parent.parent


class SessionPickerImportTests(unittest.TestCase):
    def test_picker_and_flags(self):
        src = (ROOT / 'rust-core/src/bin/webcodex-native.rs').read_text(encoding='utf-8')
        self.assertIn('"import-codex"=>o.import_codex=true', src)
        self.assertIn('"--last"=>{o.resume=true;o.last=true}', src)
        self.assertIn('picker::pick("Resume a conversation"', src)
        self.assertIn('picker::pick("Import a Codex CLI session"', src)
        self.assertLess(src.index('picker::pick("Resume a conversation"'), src.index('let mut browser=Browser::spawn'))

    def test_import_has_no_tool_output_and_preserves_codex_home(self):
        src = (ROOT / 'rust-core/src/bin/webcodex-native/codex_import.rs').read_text(encoding='utf-8')
        self.assertIn('"response_item" if p.get("type")?.as_str()?=="message"', src)
        self.assertIn('"input_text","output_text","text"', src)
        self.assertIn('"CODEX_HOME"', src)
        self.assertIn('sessions', src)
        self.assertNotIn('fs::write(&session.path', src)

    def test_handoff_and_old_thread_restore(self):
        src = (ROOT / 'rust-core/src/bin/webcodex-native.rs').read_text(encoding='utf-8')
        self.assertIn('self.state["handoff"]', src)
        self.assertIn('saved["state"]["codex_import"].is_object()', src)
        self.assertIn('"store_save_state"', src)
        self.assertIn('"store_add_message"', src)

if __name__ == '__main__':
    unittest.main()
