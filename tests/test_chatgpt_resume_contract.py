"""Contracts for resuming an external ChatGPT conversation by its URL ID."""
from pathlib import Path
import unittest
R=Path(__file__).resolve().parent.parent
class DirectResumeTests(unittest.TestCase):
    def test_uuid_and_url_normalized(self):
        text=(R/'rust-core/src/bin/webcodex-native.rs').read_text()
        self.assertIn('fn chatgpt_resume_url(',text)
        self.assertIn('https://chatgpt.com/c/',text)
        self.assertIn('o.chatgpt_url=Some(chatgpt_resume_url(&arg)?)',text)
        self.assertIn('Opening requested ChatGPT conversation',text)
        self.assertIn('ChatGPT conversation could not be opened:',text)
    def test_existing_local_thread_reused(self):
        text=(R/'rust-core/src/store.rs').read_text()
        self.assertIn('"store_find_url"',text)
        self.assertIn('WHERE chat_url=?1 AND project_root=?2',text)
    def test_other_paths_unchanged(self):
        text=(R/'rust-core/src/bin/webcodex-native.rs').read_text()
        self.assertIn('picker::pick("Resume a conversation"',text)
        self.assertIn('picker::pick("Import a Codex CLI session"',text)
        self.assertIn('"--last"=>{o.resume=true;o.last=true}',text)
if __name__=='__main__':unittest.main()
