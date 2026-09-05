# WGPUI status

Updated: 2026-09-05

## Goal

Independent wgpu + winit UI crate. Discriminating API: `WgpuSurface`. Not a drop-in for GPUI-CE git main or Zed GPUI.

Sibling track: `wgpui-component` 0.6.0 in `/Users/muk/Developer/Muktidaya/wgpui-component` (do not edit from this tree). Component path-depends on this checkout; **not published**.

## Current state

**wgpui 0.3.5 is on crates.io.** Published 2026-09-05 from clean `root` HEAD `cc6706e62bec78b2567e6cd6cc172dcf07644b5a` (Windows `flume` fix on top of the 0.3.5 identity commit `53da4d8d36`).

- `wgpui_derive` 0.3.5: https://crates.io/crates/wgpui_derive/0.3.5
- `wgpui` 0.3.5: https://crates.io/crates/wgpui/0.3.5
- Git tag `v0.3.5` points at the published commit and was pushed to `origin`. 0.3.3 and 0.3.4 were not yanked.

Pins: wgpu 30, winit 0.30.13, taffy 0.13, cosmic-text 0.19. Path GPU pass, clipboard/cursor/dialogs/open/IME/file-drop/displays are wired. Local surface after 0.3.4: derive emits `wgpui::`, glyph-atlas `write_texture` order, a11y stubs, `container_query`, gestures, spring, `ListState` follow, `BoxShadow::new` + `inset`, focus helpers taking `&mut App`, plus dirty-tree shims (test clock, `open_window`, aux-vs-primary click, a11y forwarding, unhandled key text).

## Decisions

- Completeness ≠ CE/Zed API identity. Do not copy `gpui_platform`.
- Publish order: `wgpui_derive` 0.3.5 → `wgpui` 0.3.5 → component 0.6.0 graph (macros → assets → base → component). Derive must exist on the index before `wgpui` dry-run/publish.
- Packaging defaults taken: `rust-version = "1.94"`, docs.rs metadata, repository `https://github.com/Muktidaya/wgpui`, derive README/keywords, exclude STATUS/AGENTS/flake/`.cargo`/`.github`/`scripts` from the crates.io tarball (CI file stays on disk).
- Do not yank 0.3.3 or 0.3.4. Do not AccessKit / crate-split / WASM in this bump.
- Uncommitted human work is not disposable; 0.3.5 includes that tree in this commit.

## Remaining

1. Hold `wgpui-component` (and macros/assets/base/platform) until asked. Then: macros → assets → base → component after this 0.3.5 is on the index (it is).
2. Still out of crate scope: credentials / keychain, `register_url_scheme`, auxiliary executable, dock menu, AccessKit backend, crate split, app hide/restart, WASM, HDR `color_space`, path MSAA.

## Evidence

- `Cargo.toml` / `tooling/derive/Cargo.toml` version 0.3.5
- `CHANGELOG.md`, `README.md`
- crates.io: `wgpui_derive` 0.3.5 (id 3166555, 2026-09-05T22:19:51Z); `wgpui` 0.3.5 (id 3166560, 2026-09-05T22:21:55Z)
- `cargo search` reports `wgpui = "0.3.5"` and `wgpui_derive = "0.3.5"`
- `wgpui-component` not on crates.io (API 404)
- Tag: `v0.3.5` → `cc6706e62bec78b2567e6cd6cc172dcf07644b5a`
- CI: https://github.com/Muktidaya/wgpui/actions/runs/33987452612 — first Windows fail (`flume`); fix is the published commit
