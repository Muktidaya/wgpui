# WGPUI Kit

WGPUI Kit is the application-facing façade over WGPUI, Base, styled components, and bundled icons. Its source is consolidated in this repository, preserving the component adapter's history and the exact upstream `gpui-kit` v0.6.1 commit `36b51819deb52c947a79f8de29e0e9175eda7464`.

```toml
[dependencies]
wgpui-kit = "0.6.1"
```

Kit 0.6.1 and framework 0.3.6 are published on crates.io; [STATUS.md](../STATUS.md) records release evidence. For local development use `path = "../wgpui/crates/wgpui-kit"`.

```rust
use wgpui_kit::*;
use wgpui_kit::component::{button::Button, Root};
```

`wgpui_kit::application()` creates the native application. Call `wgpui_kit::init(cx)` before constructing components. Default features include `component` and `assets`; Base and the framework remain available with defaults disabled. `test-support` provides deterministic native-event tests and observed element snapshots. `inspector`, `decimal`, and the upstream language feature flags forward to their implementation crates.

| Package | Source |
| --- | --- |
| wgpui | `src/` |
| wgpui_derive | `crates/wgpui-derive/` |
| wgpui-kit | `crates/wgpui-kit/` |
| wgpui-base | `crates/wgpui-base/` |
| wgpui-component | `crates/wgpui-component/` |
| wgpui-kit-macros | `crates/wgpui-kit-macros/` |
| wgpui-kit-assets | `crates/wgpui-kit-assets/` |
| wgpui-platform | `crates/wgpui-platform/` |
| wgpui-perf | `crates/wgpui-perf/` (unpublished) |
| wgpui-kit-harness | `crates/wgpui-kit-harness/` (unpublished) |

The styled component package remains available for existing consumers. New applications should normally use Kit. The assets and macro package names now follow Kit; upstream dependency aliases such as `gpui` are internal source compatibility choices. The assets package uses the distinct Cargo `links` identity `wgpui-kit-default-icons` so it does not conflict with the old assets package.

The derive package retains its existing crates.io name `wgpui_derive`: hyphens and underscores share a registry namespace. Its directory and framework dependency key use `wgpui-derive`, with `package = "wgpui_derive"`.

## Backend boundaries

This is a native wgpu/winit port. It does not provide upstream's WASM backend, webview/shell integrations, or `WindowProfiler` frame/input tracing. The upstream Metal-specific rendering test is not a WGPUI rendering acceptance test. Unported sources and upstream documentation remain in `upstream-kit/` for reference; their manifests do not define another active workspace. See [the acceptance inventory](kit-migration-parity.json) for validation and explicit divergences. A successful headless test does not verify GPU output. Native synthetic accessibility children and active-descendant reporting remain unimplemented; focus metadata tests do not establish full native accessibility parity.

Framework changes needed by the port include shaped character widths, device-pixel snapping, raw SVG bytes, custom test text systems, nested deferred draws with stable cache indices, accessibility focus metadata, and façade-aware derive macros. These changes retain WGPUI's rendering backend.

## Development

```sh
cargo +1.94.0 test --workspace --lib --tests
cargo +1.94.0 test -p wgpui-kit --features test-support --lib --tests
cargo +1.94.0 check -p wgpui-kit --all-features
cargo +1.94.0 run -p wgpui-kit-hello-world
```

The performance runner's protocol remains compatible with existing `util_macros::perf` tests; renaming the package does not rename its existing benchmark metadata protocol.
