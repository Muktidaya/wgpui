import importlib.util
from pathlib import Path
import unittest
from unittest.mock import patch
import urllib.error

spec = importlib.util.spec_from_file_location("boundary", Path(__file__).with_name("check-public-boundary.py"))
boundary = importlib.util.module_from_spec(spec)
spec.loader.exec_module(boundary)


class PublicBoundaryTests(unittest.TestCase):
    def test_operator_paths_and_shared_chats_are_rejected(self):
        path = "/" + "Users/operator/Developer/internal-project"
        chat = "https://" + "chatgpt.com" + "/share/synthetic-example"
        self.assertIn("operator home path", boundary.violations(path))
        self.assertIn("shared conversation", boundary.violations(chat))

    def test_private_identifier_matching_is_case_insensitive(self):
        self.assertIn("private repository reference", boundary.violations("EXAMPLE-OWNER/project", ["example-owner/project"]))

    def test_public_api_prose_is_allowed(self):
        self.assertEqual(boundary.violations("Kit exposes native rendering and a reusable application API."), [])

    def test_missing_repository_is_not_treated_as_public(self):
        with patch.object(boundary.urllib.request, "urlopen", side_effect=urllib.error.URLError("unavailable")):
            self.assertFalse(boundary.public_repository("example-owner/example-project"))


if __name__ == "__main__":
    unittest.main()
