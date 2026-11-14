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
