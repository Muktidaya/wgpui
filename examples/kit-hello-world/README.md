# WGPUI Kit hello world

This workspace example uses the local WGPUI Kit source candidate: Kit 0.8.1
on framework 0.4.0, with Rust 1.95. Its manifest points directly to
`crates/wgpui-kit` in this repository.

```rust
use wgpui_kit::*;
use wgpui_kit::component::{button::*, *};
```

Run it from the repository root:

```sh
cargo +1.95.0 run -p wgpui-kit-hello-world
```

The example is unpublished and does not require a sibling repository checkout.
See [the Kit guide](../../docs/kit.md) for initialization, feature flags,
published-version information and the candidate's validation boundaries.
