<p align="center">
  <img src="https://raw.githubusercontent.com/longbridge/gpui-component/main/website/public/logo.svg" width="112" alt="GPUI Component logo" />
  <br>
  <strong>wgpui-component</strong>
</p>

[English](./README.md) | [简体中文](./README.zh-CN.md)

> **wgpui adaptor (this tree):** Tracks Longbridge [gpui-component](https://github.com/longbridge/gpui-component) **v0.6.0** (`94a313a7`) and remaps it onto local **[wgpui](https://github.com/Muktidaya/wgpui)** 0.3.5. Published crate names stay `wgpui-*` (`wgpui-component`, `wgpui-base`, `wgpui-component-macros`, `wgpui-component-assets`). Vendored sources keep `use gpui::` / `gpui_component::` via workspace aliasing. This tree does **not** publish as `gpui-kit`. See [STATUS.md](./STATUS.md).

Build fantastic, high-performance desktop apps with Rust and [wgpui](https://github.com/Muktidaya/wgpui).

Upstream GPUI Kit (Longbridge) remains the design source; this adaptor maps `crates/component` → `crates/ui` and depends on path `wgpui` 0.3.5 instead of crates.io `gpui-pre` 0.3.1.

## Features

- **60+ UI Components**: Forms, navigation, overlays, feedback, layout, and more, with polished interactions and productive defaults.
- **Production Ready**: Used to build Longbridge Pro from day one and continuously refined in a publicly shipped commercial desktop application.
- **Native Feel**: Modern controls inspired by macOS and Windows, backed by semantic themes and multiple sizes.
- **120 FPS**: GPU-accelerated interfaces that remain smooth under load.
- **Data Tables**: Virtual scrolling, fixed and resizable columns, sorting, and cell selection across hundreds of thousands of rows.
- **Virtual Lists**: Render only the visible range, including lists whose items have different sizes.
- **Code Editor**: Stable performance at 200K lines with Tree-sitter highlighting and LSP diagnostics, completion, and hover.
- **Dock Layout**: Resizable panels, draggable tabs, nested splits, edge docks, and serializable freeform Tiles.
- **Rich Content**: Native Markdown and HTML rendering, syntax highlighting, and built-in charts.
- **Design Freedom**: Use the complete visual system or build your own on the behavior and infrastructure in `gpui-base`.
- **JavaScript Extensions**: `gpui-shell` lets a shipped Rust host load panels and business logic as scripts, with every capability granted explicitly.
- **Cross Platform**: Ship one Rust codebase to macOS, Windows, and Linux.

## Framework Architecture

### Three layers. One ecosystem.

Use `gpui-component` to keep the application coherent with one complete visual
and interaction system. Use `gpui-base` when your product needs to create and
own that system itself. Use `gpui-shell` when the application should be
extensible in JavaScript after it ships.

| **`gpui-component`**             | **`gpui-base`**                               | **`gpui-shell`**                           |
| -------------------------------- | --------------------------------------------- | ------------------------------------------ |
| Complete, styled components      | Unstyled behavior and infrastructure          | JavaScript runtime hosted by Rust          |
| Productive defaults with theming | Full control over structure and visual design | Capabilities granted one at a time         |
| Best for building applications   | Best for building design systems              | Best for plugins and scripted applications |

```text
                             APPLICATION
                                  │
              ┌───────────────────┼───────────────────┐
              │                   │                   │
              ▼                   ▼                   ▼
    ┌──────────────────┐ ┌──────────────────┐ ┌──────────────────┐
    │  gpui-component  │ │ Your Design      │ │    gpui-shell    │
    │    Styled UI     │ │ System           │ │  JS extensions   │
    └────────┬─────────┘ └────────┬─────────┘ └────────┬─────────┘
             │                    │                    │
             └────────────────────┼────────────────────┘
                                  ▼
                        ┌──────────────────┐
                        │    gpui-base     │
                        │ Behavior · State │
                        │ Infrastructure   │
                        └────────┬─────────┘
                                 ▼
                               GPUI
```

> **Behavior belongs to the foundation. Presentation belongs to the application.**

Use **`gpui-component`** when you want polished controls ready to ship. Build on
**`gpui-base`** when your application should own its component source, layout,
styling, and motion while reusing difficult interaction behavior. Add
**`gpui-shell`** when contributors should extend the product without a fork or
a release.

The layering follows the same separation that makes the
[shadcn](https://ui.shadcn.com) ecosystem flexible:

| GPUI Kit ecosystem                   | Web ecosystem                   |
| ------------------------------------ | ------------------------------- |
| GPUI                                 | HTML + Tailwind CSS             |
| [`gpui-base`](crates/base/README.md) | [Base UI](https://base-ui.com)  |
| `gpui-component`                     | shadcn's styled component layer |

[Explore the architecture →](docs/ARCHITECTURE.md)

## Showcase

GPUI Kit has powered [Longbridge Pro](https://longbridge.com/desktop)
from day one. The framework is extracted from the demands of a publicly shipped
commercial desktop application rather than designed in isolation.

> **GPUI provides the rendering foundation. Longbridge provides the production foundation.**

<img width="1763" alt="Image" src="https://github.com/user-attachments/assets/e1ecb9c3-2dd3-431e-bd97-5a819c30e551" />

## Usage

```toml
[dependencies]
wgpui = "0.3.5"
wgpui-component = "0.6.0"
```

`wgpui-component` pulls `wgpui-base` and the default icon set. Features
(`inspector`, `decimal`, `tree-sitter`, and each `tree-sitter-<language>`)
keep their upstream names.

This workspace aliases those packages as `gpui` / `gpui-component` so vendored
sources compile unchanged. A crates.io consumer imports `wgpui` and
`wgpui_component`.

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
        // This must be called before using any GPUI Component features.
        wgpui_component::init(cx);

        cx.spawn(async move |cx| {
            cx.open_window(WindowOptions::default(), |window, cx| {
                let view = cx.new(|_| HelloWorld);
                // This first level on the window, should be a Root.
                cx.new(|cx| Root::new(view, window, cx))
            })
            .expect("Failed to open window");
        })
        .detach();
    });
}
```

### Icons

The default `assets` feature bundles the [Lucide](https://lucide.dev) icon set
as `gpui-kit-assets`; pass it to the application with
`gpui_kit::application().with_assets(gpui_kit::assets::Assets)`. To ship your
own icons instead, leave that feature out and name the SVG files as defined in
[IconName](https://github.com/longbridge/gpui-kit/blob/main/crates/component/src/icon.rs#L86).

## Skills for AI Coding Agents

Install the GPUI Kit skills for your AI coding agent (Cursor, Claude Code, Gemini CLI, Codex, etc.):

```bash
npx skills add longbridge/gpui-kit
```

| Skill                    | Description                                                                                                                         |
| ------------------------ | ----------------------------------------------------------------------------------------------------------------------------------- |
| `gpui-kit`               | Setup, component catalog, usage patterns, GPUI mechanics (elements, entities, async, focus, actions, tests), and the Coding Guides. |
| `gpui-kit-design-guides` | The Design Guides: layout, spacing, hierarchy, interaction states, overlays, and interface copy.                                    |

## Development

### Desktop Gallery (Story)

The `story` crate is a gallery application that showcases all available components. Run it with:

```bash
cargo run
```

### Examples

Some important examples are built into the `story` crate and can be run directly:

```bash
# Code editor with LSP support and syntax highlighting
cargo run --example editor

# Dock layout system (panels, split views, tabs)
cargo run --example dock

# Markdown rendering
cargo run --example markdown

# HTML rendering
cargo run --example html
```

The `examples` directory also contains standalone examples, each focused on a single feature. Each example is a separate crate, run them with `cargo run -p <name>`:

```bash
# Basic hello world
cargo run -p hello_world

# System monitor (real-time charts with CPU/memory data)
cargo run -p system_monitor

# Window title customization
cargo run -p window_title
```

Check out [CONTRIBUTING.md](CONTRIBUTING.md) for more details.

## Compare to others

See the [comparison with Iced, egui and Qt 6](https://gpui-kit.com/docs/comparison) on the site.

## License

Apache-2.0

- Built on [GPUI](https://github.com/zed-industries/zed), the UI framework behind Zed, also Apache-2.0.
- UI design based on [shadcn/ui](https://ui.shadcn.com), some from [Reui](https://reui.io).
- Icons from [Lucide](https://lucide.dev).
