#!/usr/bin/env python3
"""Read-only release-preparation check against the official stable manifest."""

import argparse
import hashlib
from pathlib import Path
import re
import sys
import tomllib
from urllib.request import HTTPRedirectHandler, build_opener

ROOT = Path(__file__).resolve().parents[1]
MANIFEST = "https://static.rust-lang.org/dist/channel-rust-stable.toml"
MAX_BYTES = 4 * 1024 * 1024
VERSION = re.compile(r"^(\d+\.\d+\.\d+) \([0-9a-f]+ \d{4}-\d{2}-\d{2}\)$")


class RejectRedirect(HTTPRedirectHandler):
    def redirect_request(self, *args, **kwargs):
        raise ValueError("unexpected manifest redirect")


def manifest_version(data: bytes) -> tuple[str, str]:
    document = tomllib.loads(data.decode("utf-8"))
    release = document["pkg"]["rust"]["version"]
    match = VERSION.fullmatch(release)
    if match is None:
        raise ValueError("unexpected official Rust version format")
    date = document["date"]
    if not isinstance(date, str) or re.fullmatch(r"\d{4}-\d{2}-\d{2}", date) is None:
        raise ValueError("unexpected official Rust manifest date")
    return match.group(1), date


def check(latest: str, toolchain: dict, workspace: dict) -> None:
    pin = toolchain["toolchain"]["channel"]
    minimum = workspace["workspace"]["package"]["rust-version"]
    if pin != latest or minimum != latest:
        raise ValueError(f"refresh needed: official={latest}, project={pin}, minimum={minimum}")


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--check", action="store_true", required=True, help="fetch and compare; never edit files")
    parser.parse_args()
    try:
        with build_opener(RejectRedirect()).open(MANIFEST, timeout=15) as response:
            if response.geturl() != MANIFEST:
                raise ValueError("unexpected manifest redirect")
            data = response.read(MAX_BYTES + 1)
        if len(data) > MAX_BYTES:
            raise ValueError("manifest exceeds size limit")
        latest, date = manifest_version(data)
        with (ROOT / "rust-toolchain.toml").open("rb") as handle:
            toolchain = tomllib.load(handle)
        with (ROOT / "Cargo.toml").open("rb") as handle:
            workspace = tomllib.load(handle)
        check(latest, toolchain, workspace)
    except (OSError, UnicodeError, ValueError, KeyError, TypeError) as error:
        print(f"Rust release check failed: {error}", file=sys.stderr)
        return 1
    digest = hashlib.sha256(data).hexdigest()
    print(f"Official Rust stable {latest}, manifest date {date}, SHA-256 {digest}; project and minimum match.")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
