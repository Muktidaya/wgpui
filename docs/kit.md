# WGPUI Kit

WGPUI Kit is the application-facing façade over WGPUI, Base, styled components, and bundled icons. Its source is consolidated in this repository, preserving the component adapter's history. The current source candidate targets upstream GPUI Kit v0.8.1 commit `99576db58ec8103417c41a092e891b4022d24e7e`.

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
| wgpui-http-client | `crates/wgpui-http-client/` |
| wgpui-kit | `crates/wgpui-kit/` |
| wgpui-base | `crates/wgpui-base/` |
| wgpui-component | `crates/wgpui-component/` |
| wgpui-kit-macros | `crates/wgpui-kit-macros/` |
| wgpui-kit-assets | `crates/wgpui-kit-assets/` |
| wgpui-platform | `crates/wgpui-platform/` |
| wgpui-fps | `crates/wgpui-fps/` |
| wgpui-perf | `crates/wgpui-perf/` (unpublished) |
| wgpui-kit-harness | `crates/wgpui-kit-harness/` (unpublished) |

The styled component package remains available for existing consumers. New applications should normally use Kit. The assets and macro package names now follow Kit; upstream dependency aliases such as `gpui` are internal source compatibility choices. The assets package uses the distinct Cargo `links` identity `wgpui-kit-default-icons` so it does not conflict with the old assets package.

The derive package retains its existing crates.io name `wgpui_derive`: hyphens and underscores share a registry namespace. Its directory and framework dependency key use `wgpui-derive`, with `package = "wgpui_derive"`.

## Source candidate compatibility

The source candidate uses WGPUI and derive 0.4.0, Kit-family 0.8.1 and wgpui-http-client 0.1.0, with Rust 1.95 as the common minimum. These candidate versions are separate from the published releases described above.

The dependency migration changes public type identities. Use the framework or Kit reexports consistently rather than mixing them with older direct dependencies:

- HTTP integrations implement the trait from `wgpui::http_client` or `wgpui_kit::http_client`. The maintained facade uses stable upstream Reqwest and async-tar; `http_client::reqwest` supplies the matching multipart types. The interface remains executor-neutral, and applications continue selecting their networking runtime and transport.
- Base's public `Rope` is stable Ropey 1.6.1. Its inherent editing and `slice` methods use character indices. Editor ranges and `RopeExt` byte operations retain their byte-offset contract; use `len_bytes` and `byte_slice` when handling those ranges. LF-only editor rows remain independent of downstream Ropey line features.
- Custom grammar registrations use Tree-sitter 0.27-compatible `Language` types. Grammar versions are independent of runtime versions, and registrations must satisfy the runtime's ABI range.
- Public accessibility types come from AccessKit 0.25. Use the framework's reexport so semantic nodes and action values have the same dependency identity.
- `SharedString` is immutable and dereferences directly to `str`. Runtime borrowed strings are copied into owned shared storage, and `Option<SharedString>::as_deref()` yields `Option<&str>`. Use `new_static` for constant strings; construct a new value when changing text.

Constructor attributes resolve through ordinary, renamed and sole-Kit dependencies without a direct ctor dependency. Ctor 1 requires explicit acknowledgement of initialization safety:

```rust
#[wgpui_kit::ctor(unsafe)]
fn register_before_main() {
    // Only operations valid before the application runtime starts belong here.
}
```

An `unsafe fn` with `#[wgpui_kit::ctor]` is also supported. The wrapper forwards the safety contract rather than inserting an implicit acknowledgement.

The layout adapter preserves WGPUI's relative-position default and accounts for Taffy 0.15 containing-block coordinates. HTML conversion uses a private DOM sink for html5ever 0.40; minification preserves literal entities, foreign attributes and inert template fragments. Native theme watcher setup errors now propagate from `ThemeRegistry::watch_dir`, while backend callback errors remain visible in logs.

Taffy 0.15 distinguishes definite heights from content-derived flex allocations. Inside an auto-height panel constrained by `max_h`, a block intermediary can leave a percentage-height child sized to its content instead of the available viewport. Keep the viewport chain explicitly in flex layout, for example `v_flex().flex_1().overflow_hidden()`, so the panel can grow with short content and scroll longer content after reaching its maximum. Dialog's body and Scrollable's wrapper use this structure. A visual clipping mask does not change a scroll region's layout size.

## Typography defaults

UI and monospace text use the OS defaults through `.SystemUIFont` and
`.SystemMonospaceFont`. AppKit supplies the system UI and monospaced system
fonts on macOS. Windows supplies the current message font and fixed-pitch
system font through its font APIs. Linux and FreeBSD use Fontconfig's configured
`system-ui` and `monospace` matches. Font selection preserves the OS locale.
Application and theme family settings remain explicit overrides.

The framework does not bundle text fonts. Kit assets contain icons. Legacy
virtual font aliases resolve to the corresponding system role.

## Backend boundaries

This is a native wgpu/winit port. The v0.8.1 source migration includes frame/input/action/view profiling, touch gestures and selection, reduced-motion preferences, additional styled components, optional speech input and Syntect highlighting. Its acceptance state is recorded in [the inventory](kit-migration-parity.json); source presence is distinct from completed behavioral validation.

The renderer remains WGPUI's own backend. Browser, webview/shell and mobile platform integrations require their own platform work. The optional frame profiler records measured framework work and submission timestamps. The separate WGPUI FPS package consumes those events and operating-system resource counters. The candidate supplies synthetic accessibility children, active-descendant reporting, semantic trees and native AccessKit action delivery. AccessKit-consumer conformance tests validate tree structure; native assistive-technology acceptance remains a separate validation gate.

Validation follows upstream's default GPUI behavior on WGPUI. The optional GPUI Fast backend has separate automatic-retention contracts: upstream gates nine whole tests and one render-count sub-block behind that backend feature. Those contracts are recorded as backend exclusions in the acceptance inventory. Ordinary selection, insertion, reordering, removal, remount, control and style assertions remain active. WGPUI renders uncached entity views normally and supports explicit cached views.

The excluded upstream-kit directory remains historical reference source and does not define an active workspace.

Framework changes needed by the port include shaped character widths, device-pixel snapping, raw SVG bytes, custom test text systems, nested deferred draws with stable cache indices, accessibility focus metadata, and façade-aware derive macros. These changes retain WGPUI's rendering backend.

## Development

```sh
cargo +1.95.0 test --workspace --lib --tests
cargo +1.95.0 test -p wgpui-kit --features test-support --lib --tests
cargo +1.95.0 check -p wgpui-kit --all-features
cargo +1.95.0 run -p wgpui-kit-hello-world
```

The performance runner's protocol remains compatible with existing `util_macros::perf` tests; renaming the package does not rename its existing benchmark metadata protocol.

The byte-index geometry API retains the scope documented by [upstream Kit v0.8.1](https://github.com/longbridge/gpui-kit/blob/v0.8.1/website/base/text-view.md). Full visual navigation with bidi caret affinity and both sides of soft-wrap boundaries remains future work. Base's existing RTL glyph-box range-selection helper and its regression coverage are preserved.
