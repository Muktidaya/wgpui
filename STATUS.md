# WGPUI status

## Kit migration implementation — 2026-09-11

The consolidated candidate has 11 workspace members and preserves the complete component adapter ancestry and upstream Kit v0.6.1 (`36b51819deb52c947a79f8de29e0e9175eda7464`). Canonical paths and the application façade are documented in [Kit](docs/kit.md); package and feature boundaries are listed in [the acceptance inventory](docs/kit-migration-parity.json). Framework/derive candidates are 0.3.6 and the Kit family is 0.6.1. These versions are not yet published.

Implemented: canonical crate paths; façade-aware derives; native Kit test support; shaping, raw SVG and pixel snapping; cached nested deferred draws; upstream editor/component changes; generated icons; performance runner repair; licenses, package manifests and consumer checks. Original unported upstream packages remain reference source under `upstream-kit/`.

Verified locally on macOS with Rust 1.94:

- Workspace/all-target compilation, 1,515 default workspace tests and 84 Kit interaction tests passed before the final lint cleanup. The formerly ignored derive compile-contract test is now enabled.
- `scripts/clippy --workspace` passed release/all-target/all-feature checks with warnings denied.
- All supported Kit feature flags compile together. External applications using only Kit, a renamed Kit dependency, or Base-only Kit pass derive/render tests outside this workspace's Cargo configuration.
- SDK and CAD pass with their existing dependency baseline against consolidated framework paths. Lab's native feature passes using only Kit.
- The native hello-world window renders text and its styled button through wgpu; click callbacks were observed. The macOS accessibility tree exposes window controls only, not a full component tree.
- The performance runner fixture verifies timing metadata and pass/fail classification.

Final acceptance: 1,624 all-feature workspace tests and two Kit doctests pass. All eight packages pass the Cargo 1.95 publication dry run without uploads. Cargo 1.94 hits an internal temporary-registry checksum error. A fresh GitHub checkout passes Developer bootstrap and resolves 11 members without a component sibling. Source checkpoint: `c583f837f88aeaeaf89304213c67b4aadf256d95`, [draft PR #1](https://github.com/Muktidaya/wgpui/pull/1). Linux CI passed; macOS passed all tests and external consumers, then revealed a missing Clippy toolchain component. The workflow now installs it explicitly. Windows validation remains in progress. Detailed logs are in the recovery directory.

Release gates: exact-commit Linux/macOS/Windows CI, verified package dry-run, live consumer/submodule cutover, and registry-only verification after an authorized release. WASM, shell/webview integrations, upstream WindowProfiler, Metal-specific rendering tests, synthetic native accessibility children and active-descendant reporting are not implemented. Headless successes do not establish full upstream or cross-platform parity.

Recovery and detailed command logs: `/Users/muk/.local/state/wgpui-migration/20260911/`. Original live repositories and uncommitted work remain preserved. The dated record below describes the preceding release, not this candidate.

## Historical release record

Updated: 2026-09-05

## Goal

Independent wgpu + winit UI crate. Discriminating API: `WgpuSurface`. Not a drop-in for GPUI-CE git main or Zed GPUI.

Sibling track: `wgpui-component` 0.6.0 in `/Users/muk/Developer/wgpui-component` (do not edit from this tree). Component path-depends on this checkout. Its newer September 5 [status](../wgpui-component/STATUS.md) records macros/assets/base/component 0.6.0 publication; platform remains held. This reconciles the local records without a new registry check.

## Current state

**wgpui 0.3.5 is on crates.io.** Published 2026-09-05 from clean `root` HEAD `cc6706e62bec78b2567e6cd6cc172dcf07644b5a` (Windows `flume` fix on top of the 0.3.5 identity commit `53da4d8d36`).

- `wgpui_derive` 0.3.5: https://crates.io/crates/wgpui_derive/0.3.5
- `wgpui` 0.3.5: https://crates.io/crates/wgpui/0.3.5
- Git tag `v0.3.5` points at the published commit and was pushed to `origin`. 0.3.3 and 0.3.4 were not yanked.

Pins: wgpu 30, winit 0.30.13, taffy 0.13, cosmic-text 0.19. Path GPU pass, clipboard/cursor/dialogs/open/IME/file-drop/displays are wired. Local surface after 0.3.4: derive emits `wgpui::`, glyph-atlas `write_texture` order, a11y stubs, `container_query`, gestures, spring, `ListState` follow, `BoxShadow::new` + `inset`, focus helpers taking `&mut App`, plus subsequently committed integration shims (test clock, `open_window`, aux-vs-primary click, a11y forwarding, unhandled key text).

## Decisions

- Completeness ≠ CE/Zed API identity. Do not copy `gpui_platform`.
- Publish order: `wgpui_derive` 0.3.5 → `wgpui` 0.3.5 → component 0.6.0 graph (macros → assets → base → component). Derive must exist on the index before `wgpui` dry-run/publish.
- Packaging defaults taken: `rust-version = "1.94"`, docs.rs metadata, repository `https://github.com/Muktidaya/wgpui`, derive README/keywords, exclude STATUS/AGENTS/flake/`.cargo`/`.github`/`scripts` from the crates.io tarball (CI file stays on disk).
- Do not yank 0.3.3 or 0.3.4. Do not AccessKit / crate-split / WASM in this bump.
- Uncommitted human work is not disposable; 0.3.5 includes that tree in this commit.

## Remaining

1. Component macros/assets/base/component publication is recorded complete in its September 5 ledger. Do not repeat that release from the superseded hold instruction. Any new release or platform publication is a separate task.
2. Still out of crate scope: credentials / keychain, `register_url_scheme`, auxiliary executable, dock menu, AccessKit backend, crate split, app hide/restart, WASM, HDR `color_space`, path MSAA.

## Evidence

- `Cargo.toml` / `tooling/derive/Cargo.toml` version 0.3.5
- `CHANGELOG.md`, `README.md`
- crates.io: `wgpui_derive` 0.3.5 (id 3166555, 2026-09-05T22:19:51Z); `wgpui` 0.3.5 (id 3166560, 2026-09-05T22:21:55Z)
- `cargo search` reports `wgpui = "0.3.5"` and `wgpui_derive = "0.3.5"`
- Historical pre-publication observation: `wgpui-component` returned API 404 before the later September 5 publication recorded in the component ledger.
- Tag: `v0.3.5` → `cc6706e62bec78b2567e6cd6cc172dcf07644b5a`
- CI: https://github.com/Muktidaya/wgpui/actions/runs/33987452612 — first Windows fail (`flume`); fix is the published commit
