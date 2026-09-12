import importlib.util
from pathlib import Path
import subprocess
import tempfile
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

    def test_revision_scan_sees_committed_content_despite_worktree_cleanup(self):
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            subprocess.run(["git", "init", "-q", directory], check=True)
            document = root / "notes.txt"
            committed = "https://" + "chatgpt.com" + "/share/synthetic-example"
            document.write_text(committed)
            subprocess.run(["git", "add", "notes.txt"], cwd=root, check=True)
            subprocess.run(["git", "-c", "user.name=Test", "-c", "user.email=test@example.invalid", "commit", "-qm", "Synthetic fixture"], cwd=root, check=True)
            document.write_text("Public documentation")
            inspected = list(boundary.lines(root, None, "HEAD"))
            self.assertEqual(inspected, [("notes.txt", 1, committed)])
            self.assertIn("shared conversation", boundary.violations(inspected[0][2]))

    def test_missing_repository_is_not_treated_as_public(self):
        with patch.object(boundary.urllib.request, "urlopen", side_effect=urllib.error.URLError("unavailable")):
            self.assertFalse(boundary.public_repository("example-owner/example-project"))


if __name__ == "__main__":
    unittest.main()
