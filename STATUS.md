# WGPUI status

Updated: 2026-08-21

## Goal

WGPUI is a **wgpu + winit** UI framework with a GPUI-shaped programming model. Discriminating API: `WgpuSurface` inside the element tree. Not a drop-in for GPUI-CE git main or Zed GPUI.

## Current state

**0.3.4 is published** on crates.io (`wgpui` and `wgpui_derive`). Git: `root` at `5e94b544ff`.

Pins: wgpu 30, winit 0.30.13, taffy 0.13, cosmic-text 0.19. Path GPU pass, clipboard/cursor/dialogs/open/IME/file-drop/displays are wired. `WgpuSurface` remains the 3D/CAD child. Public docs and rustdocs describe WGPUI as an independent wgpu+winit crate, not a CE/Zed drop-in.

The glyph atlas now uploads masks with ordered `Queue::write_texture` calls. This fixes zeroed atlas
textures observed in both the hello-world example and the Topology CAD shell on macOS.

Verify: `cargo test --lib` (85 passed). Clippy: `./scripts/clippy`. Manual: `hello_world` text and
Topology CAD shell, `examples/learn/wgpu_surface.rs`, `examples/learn/paths.rs`, paste/copy, file
dialog, IME in a text field.

## Decisions

- Completeness ≠ CE/Zed API identity. Do not copy `gpui_platform`.
- CE/Zed are steal-from references (`gpui_wgpu` path raster).
- Discriminating test: CAD/3D shell with `WgpuSurface` + winit.

## Remaining (after 0.3.4)

- Credentials / keychain, `register_url_scheme`, auxiliary executable, dock menu
- AccessKit
- Crate split
- Gestures / lerp / spring
- App hide / restart
- WASM
- HDR `color_space` unless a 3D example needs it
- Path MSAA (`WGPUI_PATH_SAMPLE_COUNT` is parsed, unused)

A later 0.4.0 is reserved for product-shaped work (AccessKit, crate split), not leftover crate versions.

## Evidence

- `Cargo.toml` version 0.3.4, `CHANGELOG.md`
- `src/platform/renderer.rs` (`PrimitiveBatch::Paths`)
- `src/platform/platform.rs` (OS services)
- `src/elements/wgpu_surface.rs`
- `examples/learn/paths.rs`, `examples/learn/wgpu_surface.rs`
