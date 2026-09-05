# wgpui-base

[![License](https://img.shields.io/crates/l/wgpui-base.svg)](../../LICENSE-APACHE)

Unstyled behavior and infrastructure for [wgpui](https://github.com/Muktidaya/wgpui) applications. This is the Muktidaya adaptor of Longbridge `gpui-base` at gpui-component **0.6.0**. Published crate name: **`wgpui-base` 0.6.0**. It depends on independently versioned **wgpui 0.3.5** — those numbers are supposed to differ.

Use [`wgpui-component`](https://github.com/Muktidaya/wgpui-component) if you want ready-to-use styled controls. Use `wgpui-base` if your application should own its visual styles while reusing interaction, focus, accessibility, animation, virtual lists, and theme tokens.

```toml
[dependencies]
wgpui = "0.3.5"
wgpui-base = "0.6.0"
```

Until `wgpui` 0.3.5 is on crates.io, use a git or path checkout. This 0.6.0 line is not uploaded yet.

Vendored sources still `use gpui::` via workspace aliases. A crates.io consumer imports `wgpui` and `wgpui_base`.

## Where it fits

```text
application
├── wgpui-component     Complete, styled framework experience
└── custom UI           Application-owned design system
         └── wgpui-base Interaction, state, and infrastructure (this crate)
```

Dependencies point from higher layers toward the foundation: `wgpui-base` does not depend on `wgpui-component`.

## Design Principles

- **Behavior belongs to the foundation:** click handling, keyboard activation, controlled state, focus, accessibility roles, and infrastructure.
- **Presentation belongs to the application:** layout, size, color, spacing, radius, borders, shadows, variants, and animation.
- **Semantic APIs come first:** themes expose tokens such as `primary`, `surface`, and `destructive`.
- **wgpui-native composition:** controls implement `Styled` and `ParentElement` and work with the fluent builder API.

For example, `Button::new("save")` has no padding, background, radius, or size by default. Being unstyled is an explicit API contract, not a missing feature.

## Initialization

Call `wgpui_base::init(cx)` once before creating windows or using foundation controls. If the application already calls `wgpui_component::init(cx)`, do not call base init again.

```rust
use wgpui::*;
use wgpui_base as _;

fn main() {
    wgpui::Application::new().run(|cx| {
        wgpui_base::init(cx);
    });
}
```

### Optional Features

| Feature     | Enabled by default | Purpose                                      |
| ----------- | ------------------ | -------------------------------------------- |
| `inspector` | No                 | Inspector support in wgpui / `wgpui_derive`  |

## Quick Start

```rust
use wgpui::*;
use wgpui_base::Button;

struct SaveButton;

impl Render for SaveButton {
    fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
        Button::new("save")
            .px_3()
            .py_2()
            .rounded(px(6.))
            .bg(rgb(0x2563eb))
            .text_color(rgb(0xffffff))
            .accessibility_label("Save document")
            .on_click(|_, _, _| println!("save"))
            .child("Save")
    }
}
```

An `ElementId` must remain stable within a view so wgpui can preserve focus and element state.

### Controlled State

`Checkbox`, `Radio`, `Switch`, and `Toggle` are controlled components. Their callbacks report the next value; the application updates its own state and passes that value back on the next render.

Semantic state styles layer in a fixed order: builder chain, then value states (`checked`, `pressed`, `selected`, `focused`), then `disabled` last.

## Capability Overview

### Unstyled Controls

| API                                      | Behavior provided                                                                                     |
| ---------------------------------------- | ----------------------------------------------------------------------------------------------------- |
| `Button`                                 | Click and keyboard activation, focus, disabled and selected states, and the Button accessibility role |
| `Checkbox` / `CheckboxIndicator`         | Checked, unchecked, and indeterminate states with corresponding accessibility semantics               |
| `Radio` / `RadioGroup`                   | Radio activation, focus, and a grouping container                                                     |
| `Switch` / `SwitchTrack` / `SwitchThumb` | A controlled switch with independently styled track and thumb parts                                   |
| `Toggle` / `ToggleGroup`                 | A controlled pressed state and grouping container                                                     |
| `Link`                                   | Link semantics and activation with an application-provided `open_with` navigation strategy            |
| `Table` and semantic table parts         | Table, row-group, row, column-header, cell roles, and accessibility indices without layout or styling |
| `Toast` / `ToastStack` / `ToastManager`  | Alert semantics, lifecycle, timers, limits, measured stack geometry, and interaction-aware motion     |

### Text Editing

| Control                                                 | State           | Use                                                                                       |
| ------------------------------------------------------- | --------------- | ----------------------------------------------------------------------------------------- |
| [`Input`](../../website/base/primitives/input.md)       | `InputState`    | Single-line values, masking, validation, and number stepping                              |
| [`Textarea`](../../website/base/primitives/textarea.md) | `TextareaState` | Ordinary multi-line text, fixed rows, wrapping, and auto-grow                             |
| [`Editor`](../../website/base/primitives/editor.md)     | `EditorState`   | Source code, highlighting, gutter, folding, decorations, diagnostics, and LSP integration |

### Focus, scrolling, animation, themes

- `FocusTrapElement`, `InteractiveElementExt`, `ElementExt`, `FocusableExt`
- `Scrollbar`, `v_virtual_list` / `h_virtual_list`, `AutoScroll`
- `motion::transition` (preferred) and legacy `animation::Transition`
- `Theme` / `SemanticThemeTokens` / `StyledExt`

Upstream 0.6.0 also adds **NavStack** and splits **History** (browser-style trail) from **UndoHistory** (grouped undo/redo).

## Development

From the adaptor repository root (Rust 1.94.0, sibling `../wgpui`):

```bash
cargo +1.94.0 check -p wgpui-base --lib
cargo +1.94.0 test -p wgpui-base --lib
```

## Related

- [wgpui-component repository](https://github.com/Muktidaya/wgpui-component)
- [wgpui](https://github.com/Muktidaya/wgpui)
- [docs.rs/wgpui-base](https://docs.rs/wgpui-base) (after publish)
- Upstream [Longbridge gpui-component](https://github.com/longbridge/gpui-component)
- [Contributing](../../CONTRIBUTING.md)

## License

Apache-2.0. See [`../../LICENSE-APACHE`](../../LICENSE-APACHE). Upstream copyright remains with Longbridge.
