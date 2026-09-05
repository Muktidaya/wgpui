# Changelog

## 0.3.5 — 2026-09-05

Work after crates.io 0.3.4 (`5e94b544`). Same wgpu 30 / winit 0.30.13 / taffy 0.13 / cosmic-text 0.19 pins. **Publish `wgpui_derive` 0.3.5 before `wgpui` 0.3.5**: crates.io derive 0.3.4 does not emit `BoxShadow.inset`, so a packaged `wgpui` that depends on `"0.3.5"` cannot dry-run against the registry until derive 0.3.5 exists.

### Breaking (relative to crates.io 0.3.4)

- Derive macros always expand to `wgpui::` (no `gpui::` fallback from `CARGO_PKG_NAME`).
- `BoxShadow` gained `inset: bool`. Prefer `BoxShadow::new(offset_x, offset_y, color)` plus `.blur_radius()` / `.spread_radius()`. Style macros initialize `inset: false`.
- `FocusHandle::focus`, `Window::focus`, `focus_next`, and `focus_prev` take `&mut App`.

### Added

- Glyph atlas mask uploads use ordered `Queue::write_texture` (fixes zeroed atlas textures on macOS).
- Accessibility surface: `AriaProperties`, `A11ySubtreeBuilder`, extra AccessKit fields forwarded from `Div`, `Stateful` a11y passthrough. `Window::is_a11y_active` is still a stub (`false`).
- `container_query` element (size from style; children rendered after layout).
- Scroll `OngoingScroll` gesture helper; spring / interpolation (`SpringConfig`, `SpringAnimation`, `Interpolate`, sampled easing).
- `ListState` tail follow (`FollowMode`), `remeasure` / `remeasure_items`, `scroll_to_end`.
- Geometry: `Anchor`, bound edge/center helpers.
- Style helpers: `aspect_ratio`, `self_start` / `self_end`, `flex_grow` / `flex_shrink`.
- App identity and system-notification stubs; `App::reduce_motion` (platform wiring still stubbed).
- Primary vs auxiliary click: `on_click` ignores non-left mouse; `on_aux_click` handles middle/right without firing primary handlers.
- Unhandled key text: ordinary Unicode from `KeyboardInput` reaches the platform input handler when the event propagates; Control/Platform shortcuts do not.

### Test-support shims

- `TestAppContext::open_window`, `App::set_reduce_motion`
- `HitboxId::placeholder`, `Window::simulate_next_frame`
- Test dispatcher / scheduler `next_deadline` / `next_timer_deadline`

### Packaging

- `rust-version = "1.94"`; `documentation` + `[package.metadata.docs.rs]` on both crates.
- Repository URL `https://github.com/Muktidaya/wgpui`.
- Crate package excludes `STATUS.md`, `AGENTS.md`, Nix flake, `.cargo/` Zed leftovers, `scripts/`, and `.github/`. Untracked `.github/workflows/ci.yml` stays in the git tree as the intended CI.

### Not in 0.3.5

AccessKit platform backend, crate split, WASM, credentials / URL schemes / dock menu / hide-restart, path MSAA.

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
