# Rust toolchain release preparation

The workspace pins its development and CI compiler in `rust-toolchain.toml`
and `.github/workflows/ci.yml`. The declared consumer minimum is the workspace
`rust-version`; raising it is a consumer compatibility change requiring review.
Before preparing a release, with Python 3.11 or newer and network access, run
`python3 scripts/check-rust-release.py --check`. It fetches the official stable
manifest over HTTPS, prints its version, date and SHA-256, compares the toolchain and
consumer minimum, and fails on mismatch or malformed/unavailable input. It is a
read-only check: it does not install tools, update pins, resolve dependencies,
push, tag or publish. The manifest is fetched at check time; record its version,
date, hash and source in release evidence rather than treating a past check as
current. The hash identifies the fetched bytes; it does not independently
authenticate the manifest beyond the HTTPS connection.

When the check reports a newer release, deliberately update and review the
toolchain file, workspace minimum, CI matrix, `scripts/check-perf.py` and
`scripts/check-kit-consumers.py` defaults, and current command/minimum docs.
Keep historical validation evidence and independently versioned packages
distinct. Review whether any older compiler support is necessary and document
the reason and verification if retained. Do not silently refresh `Cargo.lock`
or third-party dependencies. Validate locked metadata, packages, formatting,
lint, workspace tests and native/consumer coverage on the exact proposed tuple
before release. CI pin and docs still require review: this checker does not parse
or validate them. Existing isolated tests and historical results do not qualify
a different toolchain or source commit.

The script's offline parser, comparison and mocked failure tests are `python3
-m unittest discover -s scripts -p 'test_rust_release_check.py' -v`. This
checklist schedules nothing and grants no permission to publish.
