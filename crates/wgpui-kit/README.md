# WGPUI Kit

The application-facing toolkit for the native **wgpu + winit** WGPUI framework. The current source candidate targets Longbridge GPUI Kit v0.8.1. It reexports framework APIs, Base behavior, styled components, default icons, and UI testing helpers.

This candidate is under validation and has not been published. Use its workspace path during development:

```toml
[dependencies]
wgpui-kit = { path = "../wgpui/crates/wgpui-kit" }
```

```rust
use wgpui_kit::*;
use wgpui_kit::component::button::Button;
```

Initialize with wgpui_kit::init(cx), then use wgpui_kit::open_window to install the Base root and enabled component plugins around application content. Defaults enable components and assets. Optional features include decimal, inspector, speech, Syntect, Tree-sitter grammars, frame profiling, and test support.

Kit behavior uses WGPUI's existing renderer. Alternative GPUI engines, browser integration, and webview/shell platform support are separate from the native port. The [Kit guide](https://github.com/Muktidaya/wgpui/blob/root/docs/kit.md) and its acceptance inventory record current validation and public API migration details.
