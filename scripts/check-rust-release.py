#!/usr/bin/env python3
"""Read-only release-preparation check against the official stable manifest."""

import argparse
import difflib
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

# Fixed occurrence counts fail closed when the reviewed update scope drifts.
SCOPE = {
    "rust-toolchain.toml": 1, "Cargo.toml": 1, ".github/workflows/ci.yml": 2,
    "scripts/check-kit-consumers.py": 1, "scripts/check-perf.py": 1,
    "README.md": 1, "STATUS.md": 1, "CHANGELOG.md": 1,
    "crates/wgpui-base/README.md": 4, "crates/wgpui-component/AGENTS.md": 7,
    "crates/wgpui-component/README.md": 1, "crates/wgpui-component/themes/README.md": 2,
    "crates/wgpui-derive/README.md": 1, "crates/wgpui-fps/README.md": 1,
    "crates/wgpui-http-client/README.md": 1, "crates/wgpui-kit-assets/README.md": 1,
    "crates/wgpui-kit-harness/README.md": 1, "crates/wgpui-kit-macros/README.md": 1,
    "crates/wgpui-kit/TESTING.md": 3, "docs/kit-migration-parity.json": 4,
    "docs/kit.md": 5, "docs/kit/native-validation.md": 1,
    "docs/kit/themes-and-examples.md": 2, "docs/rendering.md": 2,
    "examples/kit-hello-world/README.md": 2,
}


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
        raise ValueError(f"release policy review needed: official={latest}, project={pin}, declared minimum={minimum}")


def prepare_patch(files: dict[str, str], old: str, new: str) -> str:
    if not re.fullmatch(r"\d+\.\d+\.\d+", old) or not re.fullmatch(r"\d+\.\d+\.\d+", new):
        raise ValueError("expected exact numeric compiler versions")
    token = re.compile(rf"(?<![\d.]){re.escape(old)}(?![\d.])")
    patches = []
    for path, count in SCOPE.items():
        content = files[path]
        if len(token.findall(content)) != count:
            raise ValueError(f"reviewed scope changed: {path}")
        updated = token.sub(new, content)
        patches.extend(difflib.unified_diff(content.splitlines(keepends=True),
                                           updated.splitlines(keepends=True),
                                           fromfile=f"a/{path}", tofile=f"b/{path}"))
    return "".join(patches)


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    mode = parser.add_mutually_exclusive_group(required=True)
    mode.add_argument("--check", action="store_true", help="fetch and compare; never edit files")
    mode.add_argument("--prepare-patch", action="store_true", help="print a proposed patch; never edit files")
    args = parser.parse_args()
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
        pin = toolchain["toolchain"]["channel"]
        minimum = workspace["workspace"]["package"]["rust-version"]
        if pin != minimum:
            raise ValueError("toolchain and declared minimum differ; review policy before preparing")
        files = {path: (ROOT / path).read_text(encoding="utf-8") for path in SCOPE}
        patch = prepare_patch(files, pin, latest)
        if args.check:
            check(latest, toolchain, workspace)
    except (OSError, UnicodeError, ValueError, KeyError, TypeError) as error:
        print(f"Rust release check failed: {error}", file=sys.stderr)
        return 1
    digest = hashlib.sha256(data).hexdigest()
    if args.prepare_patch:
        print(f"Official Rust stable {latest}, manifest date {date}, SHA-256 {digest}; proposed patch only; review MSRV, docs, compatibility and validation.", file=sys.stderr)
        print(patch, end="")
    else:
        print(f"Official Rust stable {latest}, manifest date {date}, SHA-256 {digest}; project and declared minimum match release policy.")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
