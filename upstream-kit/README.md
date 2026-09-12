<p align="center">
  <img src="./website/public/logo.svg" width="112" alt="GPUI Component logo" />
  <br>
  <strong>wgpui-component</strong>
</p>

[English](./README.md) | [简体中文](./README.zh-CN.md)

**Adaptor** of Longbridge [gpui-component](https://github.com/longbridge/gpui-component) **0.6.0** (`94a313a7`) onto independently versioned **[wgpui](https://github.com/Muktidaya/wgpui) 0.3.5**.

The two version numbers are intentional: this crate tracks upstream component **0.6.0**; the UI runtime is **wgpui 0.3.5**, not Zed `gpui` and not crates.io `gpui-pre`. Published names are `wgpui-*`. This tree does **not** publish as `gpui-kit`. See [STATUS.md](./STATUS.md).

```toml
[dependencies]
wgpui = "0.3.5"
wgpui-component = "0.6.0"
```

A crates.io consumer imports `wgpui` and `wgpui_component`. In this workspace, Cargo aliases `gpui` / `gpui_component` onto those packages so vendored upstream sources compile unchanged.

## Features

- **60+ UI Components**: Forms, navigation, overlays, feedback, layout, and more, with polished interactions and productive defaults.
- **Production Ready**: Upstream GPUI Kit is used to build Longbridge Pro and refined in a shipped commercial desktop application.
- **Native Feel**: Modern controls inspired by macOS and Windows, backed by semantic themes and multiple sizes.
- **GPU UI**: Interfaces rendered through wgpui (wgpu + winit).
- **Data Tables**: Virtual scrolling, fixed and resizable columns, sorting, and cell selection across hundreds of thousands of rows.
- **Virtual Lists**: Render only the visible range, including lists whose items have different sizes.
- **Code Editor**: Stable performance at 200K lines with Tree-sitter highlighting and LSP diagnostics, completion, and hover.
- **Dock Layout**: Resizable panels, draggable tabs, nested splits, edge docks, and serializable freeform Tiles.
- **Rich Content**: Native Markdown and HTML rendering, syntax highlighting, and built-in charts.
- **Design Freedom**: Use the complete visual system (`wgpui-component`) or build your own on the behavior in `wgpui-base`.
- **Cross Platform**: macOS, Windows, and Linux.

## Crate map

| Published crate | Path | Upstream source name |
| --- | --- | --- |
| `wgpui-component` | `crates/ui` | `gpui-component` (`crates/component`) |
| `wgpui-base` | `crates/base` | `gpui-base` |
| `wgpui-component-macros` | `crates/macros` | `gpui-component-macros` |
| `wgpui-component-assets` | `crates/assets` | `gpui-kit-assets` |
| `wgpui-platform` (`publish = false`) | `crates/platform` | shim for `gpui_platform::application()` |

`wgpui-component` 0.6.0 depends on `wgpui` 0.3.5 (path `../wgpui` in this lab). `gpui-kit`, story gallery, WASM, and `gpui-wry` remain on disk but are not workspace members.

Upstream architecture docs still say `gpui-component` / `gpui-base` / `gpui-shell`. That layering is real in the vendored sources; the crates.io names for this adaptor are the `wgpui-*` rows above. `gpui-shell` is not built here.

[Explore the architecture →](docs/ARCHITECTURE.md)

## Showcase

Upstream GPUI Kit has powered [Longbridge Pro](https://longbridge.com/desktop) from day one.

<img width="1763" alt="Longbridge Pro using GPUI Component" src="https://github.com/user-attachments/assets/e1ecb9c3-2dd3-431e-bd97-5a819c30e551" />

## Usage

```toml
[dependencies]
wgpui = "0.3.5"
wgpui-component = "0.6.0"
```

`wgpui-component` pulls `wgpui-base` and the default icon set. Features (`inspector`, `decimal`, `tree-sitter`, and each `tree-sitter-<language>`) keep their upstream names.

Nothing has been uploaded to crates.io for this 0.6.0 line yet; until `wgpui` 0.3.5 is on the registry, depend on git or a path checkout.

### Basic Example

```rs
use wgpui::*;
use wgpui_component::{button::*, *};

pub struct HelloWorld;
impl Render for HelloWorld {
    fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
        div()
            .v_flex()
            .gap_2()
            .size_full()
            .items_center()
            .justify_center()
            .child("Hello, World!")
            .child(
                Button::new("ok")
                    .primary()
                    .label("Let's Go!")
                    .on_click(|_, _, _| println!("Clicked!")),
            )
    }
}

fn main() {
    wgpui::Application::new().run(move |cx| {
        wgpui_component::init(cx);

        cx.spawn(async move |cx| {
            cx.open_window(WindowOptions::default(), |window, cx| {
                let view = cx.new(|_| HelloWorld);
                cx.new(|cx| Root::new(view, window, cx))
            })
            .expect("Failed to open window");
        })
        .detach();
    });
}
```

The in-tree example [`examples/hello_world`](examples/hello_world) uses these same public crate names. Vendored library sources still `use gpui::` via workspace aliases.

### Icons

The default assets crate bundles the [Lucide](https://lucide.dev) icon set as `wgpui-component-assets`. Pass it to the application with `Application::new().with_assets(wgpui_component_assets::Assets)`. To ship your own icons, depend without that crate and name SVG files as defined on `IconName` in `crates/ui/src/icon.rs`.

## Development

Default branch is **`root`**. Clone:

```bash
git clone https://github.com/Muktidaya/wgpui-component.git
```

This lab expects a sibling checkout of [wgpui](https://github.com/Muktidaya/wgpui) at `../wgpui` (also branch `root`). Toolchain: Rust **1.94.0**.

Workspace default member is `hello_world`:

```bash
cargo +1.94.0 run -p hello_world
cargo +1.94.0 test -p wgpui-base --lib
cargo +1.94.0 test -p wgpui-component --lib
cargo +1.94.0 test -p wgpui-component-harness
```

The upstream Story gallery (`crates/story`) is not a workspace member in this adaptor; `cargo run` with no `-p` still builds `hello_world`, not the gallery.

See [CONTRIBUTING.md](CONTRIBUTING.md) and [STATUS.md](./STATUS.md).

## License

Apache-2.0. Copyright for upstream GPUI Component / GPUI Kit remains with Longbridge; see [LICENSE-APACHE](./LICENSE-APACHE).

- Runtime: [wgpui](https://github.com/Muktidaya/wgpui) (independent wgpu + winit UI crate).
- Upstream UI: Longbridge [gpui-component](https://github.com/longbridge/gpui-component) 0.6.0.
- UI design based on [shadcn/ui](https://ui.shadcn.com), some from [Reui](https://reui.io).
- Icons from [Lucide](https://lucide.dev).
