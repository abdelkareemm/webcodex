"""Integration checks for the isolated staged Rust runtime."""

import os
import subprocess
import tempfile
import unittest
from pathlib import Path


ROOT = Path(__file__).resolve().parents[1]
NATIVE = ROOT / "rust-core" / "target-staging" / "release" / "webcodex-native.exe"
CORE = ROOT / "rust-core" / "target-staging" / "release" / "webcodex-core.exe"


@unittest.skipUnless(
    os.name == "nt" and NATIVE.is_file() and CORE.is_file(),
    "Staged Windows Rust executables are not built",
)
class StagedNativeRuntimeTests(unittest.TestCase):
    def test_native_worker_starts_with_isolated_database(self):
        with tempfile.TemporaryDirectory(prefix="webcodex-runtime-") as temp:
            base = Path(temp)
            project = base / "project"
            home = base / "home"
            project.mkdir()
            home.mkdir()

            env = os.environ.copy()
            env["WEBCODEX_HOME"] = str(home)
            env["WEBCODEX_RUST_CORE"] = str(CORE)

            result = subprocess.run(
                [str(NATIVE), "-C", str(project), "--list-threads"],
                cwd=str(ROOT),
                env=env,
                capture_output=True,
                text=True,
                timeout=15,
            )

            self.assertEqual(result.returncode, 0, result.stderr)
            self.assertEqual(result.stdout.strip(), "")
            self.assertTrue(
                (home / "workspace.sqlite3").is_file(),
                "Native worker did not initialize the isolated database",
            )

    def test_invalid_resume_url_is_rejected_before_browser_start(self):
        result = subprocess.run(
            [str(NATIVE), "resume", "https://example.com/c/not-a-chat"],
            cwd=str(ROOT),
            capture_output=True,
            text=True,
            timeout=10,
        )

        self.assertNotEqual(result.returncode, 0)
        self.assertIn("resume requires", result.stderr)
