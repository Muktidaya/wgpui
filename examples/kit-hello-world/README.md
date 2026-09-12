# hello_world

Minimal window using the **published crate names**:

```toml
wgpui = "0.3.5"
wgpui-component = "0.6.0"
```

```rust
use wgpui::*;
use wgpui_component::{button::*, *};
```

This example does **not** `use gpui::`. Vendored library sources in `crates/` still do, via workspace Cargo aliases (`gpui` → `wgpui`, `gpui-component` → `wgpui-component`).

```bash
cargo +1.94.0 run -p hello_world
```

Requires a sibling `../wgpui` checkout. Default git branch is `root`.
