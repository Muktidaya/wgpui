# WGPUI Kit

The application-facing toolkit for the native **wgpu + winit** WGPUI framework,
ported from Longbridge GPUI Kit 0.6.1. The façade re-exports framework APIs,
Base behavior, styled components, default icons, and optional interaction tests.

```toml
[dependencies]
wgpui-kit = "0.6.1"
```

```rust
use wgpui_kit::*;
use wgpui_kit::component::{Root, button::Button};
```

Initialize with `wgpui_kit::init(cx)`. Defaults enable components and assets.
The native backend does not include upstream WASM, webview/shell integrations,
or WindowProfiler frame tracing. Source and migration details:
https://github.com/Muktidaya/wgpui/blob/root/docs/kit.md
