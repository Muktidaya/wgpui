"""Offline unit checks for release manifest parsing and comparison."""

import importlib.util
import io
from pathlib import Path
from types import SimpleNamespace
import unittest
from unittest.mock import patch

spec = importlib.util.spec_from_file_location("rust_release", Path(__file__).with_name("check-rust-release.py"))
module = importlib.util.module_from_spec(spec)
spec.loader.exec_module(module)


class RustReleaseCheckTests(unittest.TestCase):
    def run_prepare_with_fake_inputs(self, manifest, *, drift=False):
        class Response:
            def __enter__(self):
                return self

            def __exit__(self, *_):
                return False

            def geturl(self):
                return module.MANIFEST

            def read(self, limit):
                return manifest[:limit]

        virtual_root = Path("/virtual/wgpui")
        files = {path: "1.100.0\n" * count for path, count in module.SCOPE.items()}
        if drift:
            files[".github/workflows/ci.yml"] = "1.100.0\n"

        def fake_open(path, mode="r", *args, **kwargs):
            self.assertEqual(mode, "rb")
            if path == virtual_root / "rust-toolchain.toml":
                return io.BytesIO(b'[toolchain]\nchannel = "1.100.0"\n')
            if path == virtual_root / "Cargo.toml":
                return io.BytesIO(b'[workspace.package]\nrust-version = "1.100.0"\n')
            raise AssertionError(f"unexpected open: {path}")

        def fake_read_text(path, *args, **kwargs):
            return files[str(path.relative_to(virtual_root))]

        stdout, stderr = io.StringIO(), io.StringIO()
        with patch.object(module, "ROOT", virtual_root), \
             patch.object(module, "build_opener", return_value=SimpleNamespace(open=lambda *args, **kwargs: Response())), \
             patch.object(Path, "open", autospec=True, side_effect=fake_open), \
             patch.object(Path, "read_text", autospec=True, side_effect=fake_read_text), \
             patch("sys.argv", ["check-rust-release.py", "--prepare-patch"]), \
             patch("sys.stdout", stdout), patch("sys.stderr", stderr):
            result = module.main()
        return result, stdout.getvalue(), stderr.getvalue()

    def test_manifest_and_matching_pins(self):
        data = b'date = "2026-11-13"\n[pkg.rust]\nversion = "1.100.0 (0ef1bc386 2026-11-12)"\n'
        self.assertEqual(module.manifest_version(data), ("1.100.0", "2026-11-13"))
        module.check("1.100.0", {"toolchain": {"channel": "1.100.0"}},
                     {"workspace": {"package": {"rust-version": "1.100.0"}}})

    def test_mismatch_fails(self):
        with self.assertRaises(ValueError):
            module.check("1.100.0", {"toolchain": {"channel": "1.99.1"}},
                         {"workspace": {"package": {"rust-version": "1.100.0"}}})

    def test_invalid_manifest_fails(self):
        for data in (b"bad TOML =", b'date = "invalid"\n[pkg.rust]\nversion = "stable"\n', b'\xff'):
            with self.subTest(data=data), self.assertRaises((ValueError, UnicodeError, KeyError)):
                module.manifest_version(data)

    def test_missing_pin_fails(self):
        with self.assertRaises(KeyError):
            module.check("1.100.0", {}, {})

    def test_patch_is_reviewable_and_scope_is_fail_closed(self):
        files = {path: "1.100.0\n" * count for path, count in module.SCOPE.items()}
        patch_text = module.prepare_patch(files, "1.100.0", "1.101.0")
        self.assertIn("--- a/rust-toolchain.toml", patch_text)
        self.assertIn("+1.101.0", patch_text)
        files[".github/workflows/ci.yml"] = "1.100.0\n"
        with self.assertRaisesRegex(ValueError, "reviewed scope changed"):
            module.prepare_patch(files, "1.100.0", "1.101.0")

    def test_patch_rejects_nonversion_input_and_version_substrings(self):
        files = {path: "1.100.0\n" * count for path, count in module.SCOPE.items()}
        with self.assertRaises(ValueError):
            module.prepare_patch(files, "1.100.0", "stable")
        files["rust-toolchain.toml"] = "1.100.01\n"
        with self.assertRaises(ValueError):
            module.prepare_patch(files, "1.100.0", "1.101.0")

    def test_main_prepare_emits_complete_scope_patch_without_file_writes(self):
        manifest = b'date = "2026-12-01"\n[pkg.rust]\nversion = "1.101.0 (abc123 2026-11-30)"\n'
        result, stdout, stderr = self.run_prepare_with_fake_inputs(manifest)
        self.assertEqual(result, 0)
        for path in module.SCOPE:
            self.assertIn(f"--- a/{path}\n", stdout)
            self.assertIn(f"+++ b/{path}\n", stdout)
        self.assertIn("+1.101.0", stdout)
        self.assertIn("SHA-256", stderr)

    def test_main_prepare_drift_or_malformed_manifest_never_emits_partial_patch(self):
        valid = b'date = "2026-12-01"\n[pkg.rust]\nversion = "1.101.0 (abc123 2026-11-30)"\n'
        cases = [
            (valid, True),
            (b'date = "2026-12-01"\n[pkg.rust]\nversion = 101\n', False),
            (b'date = 12\n[pkg.rust]\nversion = "1.101.0 (abc123 2026-11-30)"\n', False),
            (b'date = "2026-12-01"\ndate = "2026-12-02"\n[pkg.rust]\nversion = "1.101.0 (abc123 2026-11-30)"\n', False),
            (b'date = "2026-12-01"\n[pkg.rust]\nversion = "1.101.0 (abc123 2026-11-30)"\nversion = "1.102.0 (def456 2026-12-01)"\n', False),
        ]
        for manifest, drift in cases:
            with self.subTest(manifest=manifest, drift=drift):
                result, stdout, stderr = self.run_prepare_with_fake_inputs(manifest, drift=drift)
                self.assertEqual(result, 1)
                self.assertEqual(stdout, "")
                self.assertIn("Rust release check failed", stderr)

    def test_network_failure_is_reported(self):
        def unavailable(*args, **kwargs):
            raise OSError("offline")

        with patch.object(module, "build_opener", return_value=SimpleNamespace(open=unavailable)), \
             patch("sys.argv", ["check-rust-release.py", "--check"]), \
             patch("sys.stderr", new_callable=io.StringIO):
            self.assertEqual(module.main(), 1)

    def test_redirect_and_oversized_response_fail_before_reading_project_files(self):
        class Response:
            def __init__(self, url, data):
                self.url, self.data = url, data

            def __enter__(self):
                return self

            def __exit__(self, *_):
                return False

            def geturl(self):
                return self.url

            def read(self, limit):
                return self.data[:limit]

        cases = [("https://example.invalid/redirect", b""),
                 (module.MANIFEST, b"x" * (module.MAX_BYTES + 1))]
        for url, data in cases:
            with self.subTest(url=url), \
                 patch.object(module, "build_opener", return_value=SimpleNamespace(open=lambda *args, **kwargs: Response(url, data))), \
                 patch("sys.argv", ["check-rust-release.py", "--check"]), \
                 patch("sys.stderr", new_callable=io.StringIO), \
                 patch.object(Path, "open", side_effect=AssertionError("unexpected project read")):
                self.assertEqual(module.main(), 1)

    def test_redirect_handler_refuses_before_following(self):
        with self.assertRaises(ValueError):
            module.RejectRedirect().redirect_request(None, None, None, None, None, "https://example.invalid/")


if __name__ == "__main__":
    unittest.main()
