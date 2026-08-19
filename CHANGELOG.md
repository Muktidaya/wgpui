# Changelog

## 0.3.4 — 2026-08-19

First independent-line WGPUI release. This is a wgpu + winit UI crate, not a GPUI-CE or Zed drop-in.

### Breaking

- Renderer targets **wgpu 30** (was 28): `Queue::present`, `CurrentSurfaceTexture`, optional bind-group / vertex-buffer slots, `SurfaceConfiguration.color_space = Auto`.
- Layout uses **taffy 0.13** (was pinned `=0.9.0`). `AlignItems` / `AlignContent` are structs (`Self::START`, `Self::FLEX_START`, `Self::SPACE_BETWEEN`, …).
- Text shaping uses **cosmic-text 0.19** (was 0.18.2).
- Removed the uninhabited public `surface()` / `Surface` API. Use `WgpuSurface` as the GPU child.
- Removed Cargo fiction: empty `blade-*`, `wayland`, `x11`, and related no-op features. Default features are `font-kit` and `windows-manifest`.
- Renderer env vars renamed: `WGPUI_PATH_SAMPLE_COUNT`, `WGPUI_FONTS_GAMMA`, `WGPUI_FONTS_GRAYSCALE_ENHANCED_CONTRAST` (was `ZED_*`).

### Added

- GPU path rasterization for `PrimitiveBatch::Paths` (loop-blinn intermediate, then composite). Example: `examples/learn/paths.rs`.
- OS services on winit:
  - clipboard (text via `arboard`)
  - cursor (`CursorStyle` → `winit::CursorIcon`)
  - `open_url` / `open_with_system` (`open`)
  - `reveal_path` (macOS `open -R`; elsewhere the parent directory)
  - path dialogs (`rfd` on the foreground thread; mixed file+dir selection is not supported)
  - IME (`WindowEvent::Ime` → `PlatformInputHandler`)
  - file drop (`DroppedFile` / `HoveredFile`)
  - displays (`MonitorHandle`) and `active_window()` from winit focus

### Not in 0.3.4

Credentials, URL scheme registration, auxiliary executables, dock menu, app hide/restart, AccessKit, crate split, WASM, HDR color spaces.

### Dependency bumps (stable crates.io)

| Crate | From (0.3.3 line) | To |
| --- | --- | --- |
| wgpu | 28 | 30 |
| winit | 0.30.12 | 0.30.13 |
| taffy | =0.9.0 | 0.13 |
| cosmic-text | 0.18.2 | 0.19 |
| pollster | 0.4 | 1 |
| resvg / usvg | 0.47 | 0.48.1 |
| objc2 | 0.5.2 | 0.6.4 |
| objc2-app-kit / foundation | 0.2.x | 0.3.2 |
| windows-core | 0.61 | 0.62 |
| cocoa | =0.26.0 | 0.26.1 |
| arboard, rfd, open | — | added |

`gpui_*` satellites remain 0.2.2. `oo7` stays 0.6.x (`0.7` is alpha). Pre-release crates (winit 0.31 beta) were not taken.

Dropped unused macOS GPU crates: `metal`, `core-text`, `core-video`, `objc2-metal`. `objc2-app-kit` remains for native menus.
