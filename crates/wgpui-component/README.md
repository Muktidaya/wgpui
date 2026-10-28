# wgpui-component

Styled desktop components for the native WGPUI framework, adapted from
Longbridge GPUI Kit v0.8.1 (`99576db5`). This implementation crate is now part
of the [WGPUI workspace](https://github.com/Muktidaya/wgpui).

New applications should use **wgpui-kit**, which initializes the framework,
Base, styled components, and default assets through one dependency. Existing
applications can use the styled implementation crate directly. The current source candidate pairs wgpui-component 0.8.1 with WGPUI 0.4.0 and requires Rust 1.95; it remains under validation.

```rust
use wgpui_kit::*;
use wgpui_kit::component::{Root, button::Button};
```

Base owns reusable behavior and interaction; this crate supplies presentation,
theme tokens, and styled controls. Native rendering remains wgpu + winit.
The source candidate includes optional speech input, Syntect highlighting and the v0.8.1 component surfaces. Frame profiling is available through the framework and Kit. Browser and webview/shell platform integrations remain separate from this native port.

See the [Kit guide](https://github.com/Muktidaya/wgpui/blob/root/docs/kit.md)
for package paths, features, migration details, and acceptance boundaries.
Current architecture/design/testing guides live in `docs/kit/`; upstream provenance and retired source recovery are recorded there.

Licensed under Apache-2.0. Upstream attribution and history are preserved.
