"""Exercise consumer generation with a Windows cp1252 default, without Cargo."""
from contextlib import redirect_stdout
import io
from pathlib import Path
import runpy
import tempfile
import unittest
from unittest.mock import patch


class ConsumerEncodingTests(unittest.TestCase):
    def test_fixture_generation_preserves_unicode_with_cp1252_default(self):
        original_open = Path.open
        generated = []

        def windows_open(path, mode="r", buffering=-1, encoding=None, errors=None, newline=None):
            if "b" not in mode and encoding in (None, "locale"):
                encoding = "cp1252"
            return original_open(path, mode, buffering, encoding, errors, newline)

        def inspect_consumer(command, *, env, check, cwd):
            self.assertTrue(check)
            self.assertIn("test", command)
            directory = Path(cwd)
            source = (directory / "src/lib.rs").read_bytes().decode("utf-8")
            manifest = (directory / "Cargo.toml").read_bytes().decode("utf-8")
            self.assertIn("[dependencies]", manifest)
            if "styles-" not in directory.name:
                self.assertIn('String::from("Δ🦀")', source)
                self.assertIn('assert_eq!(borrowed, Some("Δ🦀"))', source)
            else:
                self.assertIn("renamed_style_macro_paths_apply_real_refinements", source)
                self.assertIn("assert_eq!(probe.style.visibility", source)
            generated.append(directory.name)

        with patch.object(Path, "open", windows_open):
            # Prove this discriminator would reject the original implicit encoding.
            with tempfile.TemporaryDirectory() as temporary:
                with self.assertRaises(UnicodeEncodeError):
                    (Path(temporary) / "implicit.rs").write_text("Δ🦀")
            with patch("subprocess.run", side_effect=inspect_consumer), redirect_stdout(io.StringIO()):
                runpy.run_path(str(Path(__file__).with_name("check-kit-consumers.py")), run_name="__main__")
        self.assertEqual(generated, ["wgpui_kit-default", "ui-default", "ui-base",
                                    "framework-wgpui", "framework-engine", "styles-engine", "styles-ui"])


if __name__ == "__main__":
    unittest.main()
