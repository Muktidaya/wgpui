# WGPUI status

Updated: 2026-09-05

## Goal

Prepare **wgpui 0.3.5** for a later crates.io publish (do not publish in this pass). Independent wgpu + winit UI crate. Discriminating API: `WgpuSurface`. Not a drop-in for GPUI-CE git main or Zed GPUI.

Sibling track: `wgpui-component` 0.6.0 in `/Users/muk/Developer/Muktidaya/wgpui-component` (do not edit from this tree). Component path-depends on this checkout.

## Current state

On-disk identity is **0.3.5** for both `wgpui` and `wgpui_derive`. crates.io still has **0.3.4** only (2026-08-19, `5e94b544`). No git tag `v0.3.5`. This `root` checkpoint includes the unpublished API/test work and `.github/workflows/ci.yml`. Pushed as `53da4d8d36`. **Not published.** First Actions run `33987452612` failed on Windows: `flume` was macOS/Linux-only while `BackgroundExecutor::spawn_realtime` uses it on every OS. Fix: move `flume` to shared `[dependencies]`.

This bump is necessary and sufficient for the wgpui half.

Pins: wgpu 30, winit 0.30.13, taffy 0.13, cosmic-text 0.19. Path GPU pass, clipboard/cursor/dialogs/open/IME/file-drop/displays are wired. Local surface after 0.3.4: derive emits `wgpui::`, glyph-atlas `write_texture` order, a11y stubs, `container_query`, gestures, spring, `ListState` follow, `BoxShadow::new` + `inset`, focus helpers taking `&mut App`, plus dirty-tree shims (test clock, `open_window`, aux-vs-primary click, a11y forwarding, unhandled key text).

## Decisions

- Completeness ≠ CE/Zed API identity. Do not copy `gpui_platform`.
- Publish order: `wgpui_derive` 0.3.5 → `wgpui` 0.3.5 → component 0.6.0 graph. Dry-run of `wgpui` against the registry will fail until derive 0.3.5 is on crates.io; that is expected, not a packaging defect.
- Packaging defaults taken: `rust-version = "1.94"`, docs.rs metadata, repository `https://github.com/Muktidaya/wgpui`, derive README/keywords, exclude STATUS/AGENTS/flake/`.cargo`/`.github`/`scripts` from the crates.io tarball (CI file stays on disk).
- Do not yank 0.3.4. Do not AccessKit / crate-split / WASM in this bump.
- Uncommitted human work is not disposable; 0.3.5 includes that tree in this commit.

## Remaining (after 0.3.5 commit, before publish)

1. `cargo +1.94.0 publish -p wgpui_derive` then `-p wgpui` (held until asked).
2. Tag `v0.3.5` after a successful publish if desired.

Still out of crate scope: credentials / keychain, `register_url_scheme`, auxiliary executable, dock menu, AccessKit backend, crate split, app hide/restart, WASM, HDR `color_space`, path MSAA.

## Evidence

- `Cargo.toml` / `tooling/derive/Cargo.toml` version 0.3.5
- `CHANGELOG.md`, `README.md`
- `src/platform/atlas.rs`, `tooling/derive/src/gpui_path.rs`, `src/style.rs` (`BoxShadow`)
- Included: `src/app.rs`, `src/app/test_context.rs`, `src/elements/div.rs`, `src/key_dispatch.rs`, `src/platform/platform.rs`, `src/platform/test/dispatcher.rs`, `src/scheduler/test_scheduler.rs`, `src/window.rs`, `.github/workflows/ci.yml`, `tooling/derive/README.md`
- Verify: `cargo +1.94.0 test --lib`; `cargo +1.94.0 publish --dry-run -p wgpui_derive`; `cargo +1.94.0 publish --dry-run -p wgpui` (wgpui dry-run fails until derive 0.3.5 is on crates.io)
- CI: https://github.com/Muktidaya/wgpui/actions/runs/33987452612 — ubuntu/macos green; Windows `cargo check --locked --lib` E0433 `flume` in `src/scheduler/executor.rs:173`
