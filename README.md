# wgpui

A fork of [GPUI](https://gpui.rs) (Zed's GPU-accelerated UI framework) with a unified **wgpu + winit** backend, replacing the original per-platform Metal, Blade, and Direct3D renderers.

## What changed

- Single cross-platform renderer built on [wgpu](https://wgpu.rs) and [winit](https://github.com/rust-windowing/winit).
- All native platform backends (macOS/Metal, Linux/Blade, Windows/Direct3D) and their dependencies have been removed.
- Text rendering uses [cosmic-text](https://github.com/pop-os/cosmic-text) and [font-kit](https://github.com/servo/font-kit).

## Usage

```toml
[dependencies]
wgpui = { version = "0.3.3" }
```
