"""Release metadata must agree across Python, Rust crates, and native protocol."""
import pathlib
import re
import unittest

ROOT = pathlib.Path(__file__).resolve().parent.parent

class ReleaseVersionTests(unittest.TestCase):
    def test_release_version_consistent(self):
        checks = {
            'pyproject.toml': 'version = "1.2.0"',
            'rust-core/Cargo.toml': 'version = "1.2.0"',
            'rust-tui/Cargo.toml': 'version = "1.2.0"',
            'webcodex/__init__.py': '__version__ = "1.2.0"',
            'rust-core/src/bin/webcodex-native.rs': 'webcodex 1.2.0 (Rust)',
            'rust-core/src/main.rs': '\"version\":\"1.2.0\"',
        }
        for path, needle in checks.items():
            with self.subTest(path=path):
                self.assertIn(needle, (ROOT/path).read_text(encoding='utf-8'))
    def test_release_history(self):
        history = (ROOT/'CHANGELOG.md').read_text(encoding='utf-8')
        self.assertIn('Release #22', history)
        rows = re.findall(r'^\|\s*(\d+)\s*\|',history,re.M)
        self.assertEqual([int(x) for x in rows], list(range(1,23)))
