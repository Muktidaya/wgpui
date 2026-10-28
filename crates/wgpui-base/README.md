# wgpui-base

[![License](https://img.shields.io/crates/l/wgpui-base.svg)](LICENSE-APACHE)

Unstyled behavior and infrastructure for [WGPUI](https://github.com/Muktidaya/wgpui) applications, adapted from Longbridge GPUI Kit v0.8.1. The current source candidate pairs **wgpui-base 0.8.1** with **WGPUI 0.4.0** and requires **Rust 1.95**. Candidate validation and publication are in progress; [release status](../../STATUS.md) records acceptance and publication evidence.

The published Base 0.6.1 and framework 0.3.6 releases remain distinct from this source candidate. Base and the framework use independent version numbers.

Use [WGPUI Kit](../../docs/kit.md) for the application facade, or [`wgpui-component`](../wgpui-component) for ready-to-use styled controls. Use `wgpui-base` if your application should own its visual styles while reusing interaction, focus, accessibility, animation, virtual lists, and theme tokens.

Version requirements for the candidate, once published:

```toml
[dependencies]
wgpui = "0.4.0"
wgpui-base = "0.8.1"
```

Until these candidate versions are published, use path dependencies on this repository: the framework is at the repository root and Base is at `crates/wgpui-base`. See the [Kit guide](../../docs/kit.md#source-candidate-compatibility) for dependency and public type compatibility. Internal source aliases such as `gpui` do not change the public package names; applications import `wgpui` and `wgpui_base`.

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

| Feature | Enabled by default | Purpose |
| --- | --- | --- |
| `decimal` | No | `rust_decimal::Decimal` values in plot scales |
| `inspector` | No | Inspector support in WGPUI and `wgpui_derive` |
| `test-support` | No | Framework support for deterministic UI tests |

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
| [`Input`](src/input/input/mod.rs)       | `InputState`    | Single-line values, masking, validation, and number stepping                              |
| [`Textarea`](src/input/textarea/mod.rs) | `TextareaState` | Ordinary multi-line text, fixed rows, wrapping, and auto-grow                             |
| [`Editor`](src/input/editor/mod.rs)     | `EditorState`   | Source code, highlighting, gutter, folding, decorations, diagnostics, and LSP integration |

### Focus, scrolling, animation, themes

- `FocusTrapElement`, `InteractiveElementExt`, `ElementExt`, `FocusableExt`
- `Scrollbar`, `v_virtual_list` / `h_virtual_list`, `AutoScroll`
- `motion::transition` (preferred) and legacy `animation::Transition`
- `Theme` / `SemanticThemeTokens` / `StyledExt`

Base includes **NavStack**, **History** (browser-style trail), and **UndoHistory** (grouped undo/redo). **Root** owns window-scoped layers and presentation plugins. The [input module guide](src/input/README.md) describes the shared editing engine; the [Kit compatibility guide](../../docs/kit.md#source-candidate-compatibility) explains the byte-offset editor contract and stable Ropey API.

## Development

From the consolidated WGPUI repository root, using Rust 1.95 or later:

```bash
cargo +1.95.0 check --locked -p wgpui-base --all-features --all-targets
cargo +1.95.0 test --locked -p wgpui-base --all-features --lib
```

## Related

- [wgpui-component](../wgpui-component)
- [WGPUI Kit guide](../../docs/kit.md)
- [wgpui](https://github.com/Muktidaya/wgpui)
- [Published wgpui-base documentation](https://docs.rs/wgpui-base) (candidate documentation follows publication)
- Upstream [Longbridge GPUI Kit](https://github.com/longbridge/gpui-kit)

## License

Apache-2.0. See [LICENSE-APACHE](LICENSE-APACHE) for the license and Longbridge attribution. Upstream notices are preserved.
