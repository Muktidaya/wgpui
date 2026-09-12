#!/usr/bin/env python3
"""Check public text for operator paths, shared chats, and unverified repository links."""
import argparse
import json
import os
from pathlib import Path
import re
import subprocess
import sys
import urllib.error
import urllib.parse
import urllib.request


PATTERNS = {
    "operator home path": re.compile(r"(?:/Users/|/home/)[^\s/]+/(?:Developer|Documents|Desktop|\.codex|\.agents|\.local/state)(?:/|\b)"),
    "operator state path": re.compile(r"(?:~/|\$HOME/)(?:\.codex|\.agents|\.local/state)(?:/|\b)"),
    "shared conversation": re.compile(r"https?://chatgpt\.com/share/[^\s)>]+"),
    "private key marker": re.compile(r"-----BEGIN (?:RSA |EC |DSA |OPENSSH )?PRIVATE KEY-----"),
    "credential-shaped value": re.compile(r"\b(?:gh[pousr]_[A-Za-z0-9]{36,255}|github_pat_[A-Za-z0-9_]{70,255})\b"),
}
GITHUB_LINK = re.compile(r"(?:https?://github\.com/|git@github\.com:)([A-Za-z0-9_.-]+/[A-Za-z0-9_.-]+)")


def violations(text, forbidden=()):
    result = [name for name, pattern in PATTERNS.items() if pattern.search(text)]
    if any(name.casefold() in text.casefold() for name in forbidden):
        result.append("private repository reference")
    return result


def public_repository(name):
    name = name.removesuffix(".git").rstrip(".")
    url = "https://api.github.com/repos/" + urllib.parse.quote(name, safe="/")
    headers = {"Accept": "application/vnd.github+json", "User-Agent": "public-boundary-check"}
    if token := os.environ.get("GITHUB_TOKEN"):
        headers["Authorization"] = "Bearer " + token
    try:
        with urllib.request.urlopen(urllib.request.Request(url, headers=headers), timeout=20) as response:
            return json.load(response).get("private") is False
    except (urllib.error.URLError, ValueError):
        return False


def lines(root, base, revision=None):
    if base and set(base) != {"0"}:
        diff = subprocess.check_output(["git", "diff", "--no-ext-diff", "--unified=0", base, revision or "HEAD", "--"], cwd=root, text=True)
        path, number = "", 0
        for line in diff.splitlines():
            if line.startswith("+++ b/"):
                path = line[6:]
            elif line.startswith("@@"):
                number = int(re.search(r"\+(\d+)", line).group(1))
            elif line.startswith("+") and not line.startswith("+++"):
                yield path, number, line[1:]
                number += 1
        return
    if revision:
        entries = subprocess.check_output(["git", "ls-tree", "-rz", revision], cwd=root).split(b"\0")
        process = subprocess.Popen(["git", "cat-file", "--batch"], cwd=root, stdin=subprocess.PIPE, stdout=subprocess.PIPE)
        try:
            for entry in filter(None, entries):
                metadata, name = entry.split(b"\t", 1)
                mode, kind, oid = metadata.split()
                if kind != b"blob" or mode == b"120000":
                    continue
                process.stdin.write(oid + b"\n")
                process.stdin.flush()
                size = int(process.stdout.readline().split()[2])
                data = process.stdout.read(size)
                assert process.stdout.read(1) == b"\n"
                if b"\0" in data[:8192]:
                    continue
                for number, text in enumerate(data.decode("utf-8", errors="replace").splitlines(), 1):
                    yield name.decode("utf-8", errors="replace"), number, text
        finally:
            process.stdin.close()
            process.stdout.close()
            if process.wait():
                raise RuntimeError("Git object inspection failed")
        return
    names = subprocess.check_output(["git", "ls-files", "-z"], cwd=root).decode().split("\0")
    for name in filter(None, names):
        path = root / name
        if path.is_symlink() or not path.is_file():
            continue
        data = path.read_bytes()
        if b"\0" in data[:8192]:
            continue
        for number, text in enumerate(data.decode("utf-8", errors="replace").splitlines(), 1):
            yield name, number, text


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--revision", help="Inspect committed content at this revision, including when the worktree differs.")
    parser.add_argument("--base", help="Scan additions since this commit; otherwise scan tracked text.")
    parser.add_argument("--private-repositories", type=Path, help="Local-only JSON list of private repository identifiers; never commit this file.")
    parser.add_argument("--check-github-links", action="store_true", help="Require added GitHub repository links to resolve as public.")
    args = parser.parse_args()
    root = Path(__file__).resolve().parents[1]
    forbidden = json.loads(args.private_repositories.read_text()) if args.private_repositories else []
    if not isinstance(forbidden, list) or not all(isinstance(value, str) and value for value in forbidden):
        parser.error("Private repository policy must be a JSON list of nonempty identifiers")
    failures = 0
    checked_links = {}
    for path, number, text in lines(root, args.base, args.revision):
        issues = violations(text, forbidden)
        if args.check_github_links:
            for name in GITHUB_LINK.findall(text):
                if name not in checked_links:
                    checked_links[name] = public_repository(name)
                if not checked_links[name]:
                    issues.append("repository reference is not verified public")
        if issues:
            # Do not echo matched values or source lines into public CI logs.
            print(f"{path}:{number}: {', '.join(issues)}", file=sys.stderr)
            failures += 1
    if failures:
        raise SystemExit(1)
    print("Public text boundary check passed; semantic disclosure review is still required.")


if __name__ == "__main__":
    main()
