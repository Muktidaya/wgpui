# WGPUI status

## Kit migration planning — 2026-09-11

The [locally grounded migration plan](docs/kit-migration-plan.md) records the proposed crates layout, history preservation, exact upstream 0.6.1 target, consumer/remote cutover and release gates. Inspection and a derive compilation check are complete; implementation and full parity validation have not started. Existing uncommitted work is preserved. The release record below remains dated September 5 evidence.

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
