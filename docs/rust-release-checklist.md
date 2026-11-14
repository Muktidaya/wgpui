# Rust toolchain release preparation

The workspace pins its development and CI compiler in `rust-toolchain.toml`
and `.github/workflows/ci.yml`. The declared consumer minimum is the workspace
`rust-version`; raising it is a consumer compatibility change requiring review.
Before preparing a release, with Python 3.11 or newer and network access, run
`python3 scripts/check-rust-release.py --check`. It fetches the official stable
manifest over HTTPS, prints its version, date and SHA-256, compares the toolchain
and declared consumer minimum, checks the recorded update scope, and fails on
mismatch or malformed/unavailable input. Matching versions reflect the current
release policy, not proof that language features require that minimum. It is a
read-only check: it does not install tools, update pins, resolve dependencies,
push, tag or publish. The manifest is fetched at check time; record its version,
date, hash and source in release evidence rather than treating a past check as
current. The hash identifies the fetched bytes; it does not independently
authenticate the manifest beyond the HTTPS connection.

When the check reports a newer release, `python3 scripts/check-rust-release.py
--prepare-patch` fetches the manifest and prints a unified patch to stdout and
manifest provenance to stderr. It changes no files. Its fixed list and expected
version occurrence counts cover the toolchain file, workspace minimum, CI
matrix, `scripts/check-perf.py` and `scripts/check-kit-consumers.py` defaults,
and identified current command/minimum docs; it fails before printing a patch
if that scope drifts. Save and review the patch, apply it deliberately, audit
the repository for additional references, and rerun the check. The list and
counts must themselves be reviewed when files change; matching counts cannot
prove semantic correctness or detect every new reference.
Keep historical validation evidence and independently versioned packages
distinct. Review whether any older compiler support is necessary and document
the reason and verification if retained. Do not silently refresh `Cargo.lock`
or third-party dependencies. Validate locked metadata, packages, formatting,
lint, workspace tests and native/consumer coverage on the exact proposed tuple
before release. CI configuration and documentation still require semantic
review beyond the scoped occurrence check. Existing isolated tests and historical results do not qualify
a different toolchain or source commit.

The script's offline parser, comparison and mocked failure tests are `python3
-m unittest discover -s scripts -p 'test_rust_release_check.py' -v`. This
checklist schedules nothing and grants no permission to publish.
