"""Verify consistent WebCodex release metadata across Python and Rust."""

import pathlib
import re
import unittest

ROOT = pathlib.Path(__file__).resolve().parent.parent


def read_version(path):
    content = (ROOT / path).read_text(encoding="utf-8")
    match = re.search(r'(?m)^version\s*=\s*"([^"]+)"', content)
    if match is None:
        raise AssertionError(f"Missing package version: {path}")
    return match.group(1)


class ReleaseVersionTests(unittest.TestCase):
    def test_release_version_consistent(self):
        version = read_version("pyproject.toml")

        for path in ("rust-core/Cargo.toml", "rust-tui/Cargo.toml"):
            with self.subTest(path=path):
                self.assertEqual(read_version(path), version)

        checks = {
            "webcodex/__init__.py": f'__version__ = "{version}"',
            "rust-core/src/bin/webcodex-native.rs": 'env!("CARGO_PKG_VERSION")',
            "rust-core/src/main.rs": 'env!("CARGO_PKG_VERSION")',
        }

        for path, expected in checks.items():
            with self.subTest(path=path):
                self.assertIn(expected, (ROOT / path).read_text(encoding="utf-8"))

    def test_release_history(self):
        history = (ROOT / "CHANGELOG.md").read_text(encoding="utf-8")
        self.assertIn("## 1.2.1", history)
        self.assertIn("Release #23", history)

        rows = re.findall(r"^\|\s*(\d+)\s*\|", history, re.M)
        self.assertEqual([int(x) for x in rows], list(range(1, 23)))
