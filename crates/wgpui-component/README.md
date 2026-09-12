# wgpui-component

Styled desktop components for the native WGPUI framework, adapted from
Longbridge GPUI Kit v0.6.1 (`36b51819`). This implementation crate is now part
of the [WGPUI workspace](https://github.com/Muktidaya/wgpui).

New applications should use **wgpui-kit**, which initializes the framework,
Base, styled components, and default assets through one dependency. Existing
applications can still depend on `wgpui-component = "0.6.1"` and
`wgpui = "0.3.6"` directly.

```rust
use wgpui_kit::*;
use wgpui_kit::component::{Root, button::Button};
```

Base owns reusable behavior and interaction; this crate supplies presentation,
theme tokens, and styled controls. Native rendering remains wgpu + winit.
WASM, upstream webview/shell integrations, and frame profiling are not ported.

See the [Kit guide](https://github.com/Muktidaya/wgpui/blob/root/docs/kit.md)
for package paths, features, migration details, and acceptance boundaries.
Upstream's original documents remain in `upstream-kit/` in the source repository.

Licensed under Apache-2.0. Upstream attribution and history are preserved.
