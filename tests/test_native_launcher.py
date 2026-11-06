import unittest
from unittest.mock import patch
from pathlib import Path
from webcodex import cli


class LauncherTests(unittest.TestCase):
    def test_rejects_mismatched_native_version(self):
        with patch.object(cli, "native_executable", return_value=Path(__file__)), patch.object(cli.subprocess, "check_output", return_value="webcodex 0.0.0 (Rust)"), patch.object(cli.subprocess, "call") as run:
            self.assertEqual(cli.main(["--version"]), 2)
            run.assert_not_called()

    def test_reports_missing_native_binary(self):
        with patch.object(cli, "native_executable", return_value=Path("/unlikely/webcodex-native")):
            self.assertEqual(cli.main(["--version"]), 2)

    @patch.object(cli.subprocess, "check_output", return_value="webcodex " + __import__("webcodex").__version__ + " (Rust)")
    def test_preserves_all_flags(self, mock_version):
        import tempfile
        with tempfile.TemporaryDirectory() as folder:
            fake = Path(folder) / "native"
            fake.write_text("test")
            with patch.object(cli, "native_executable", return_value=fake), patch.object(cli.subprocess, "call", return_value=0) as run:
                self.assertEqual(cli.main(["resume", "-s", "danger-full-access", "-a", "never"]), 0)
                run.assert_called_once_with([str(fake), "resume", "-s", "danger-full-access", "-a", "never"])

if __name__ == "__main__": unittest.main()
