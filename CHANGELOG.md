# Changelog

## 0.6.0 — 2026-09-05

Adaptor of Longbridge [gpui-component](https://github.com/longbridge/gpui-component) **v0.6.0** (`94a313a7`) onto independently versioned **[wgpui](https://github.com/Muktidaya/wgpui) 0.3.5**. The version numbers are supposed to differ: this crate follows upstream component 0.6.0; the runtime is wgpui 0.3.5, not Zed `gpui` and not `gpui-pre` 0.3.1.

Merge: `2177ce92` (v0.5.2 adaptor) + tag `v0.6.0` → `root`. Did not take the 32 commits after the tag.

### Upstream 0.6.0 brought in

- NavStack.
- History / UndoHistory split (public base API).
- Dock reconcile and tiles persistence.
- Button, tab, sheet, notification, and settings fixes.
- Upstream layout `crates/ui` → `crates/component` (mapped back to `crates/ui` here).
- Assets `links = "gpui-kit-default-icons"`; published crate name stays `wgpui-component-assets`.

### Adaptor

- Path pin `wgpui = { version = "0.3.5", path = "../wgpui" }` (and the `gpui` alias of the same).
- Published names: `wgpui-component`, `wgpui-base`, `wgpui-component-macros`, `wgpui-component-assets` (all 0.6.0). Workspace default `publish = false`. `wgpui-platform` and `gpui-wry` unpublished.
- `extern crate gpui as wgpui`; SharedString / `ArcCow` / `Option<TextStyleRefinement>` remaps; motion `mul_f64`; blink-cursor `Result` handling; `ArenaClearNeeded::clear()`.
- Nested text refine merge (`refine_style_refinement`); inspector `p_1`/`mx_2` reflection; `scroll_to_item` stops list follow; scroll inner `flex_basis(Auto)`; reduced-motion spinner; dialog open animation no longer sets `top(y * delta)`.
- Lib tests: `wgpui-component` 417, `wgpui-base` 764, harness 4.
- CI on default branch `root`; sibling checkout of `wgpui`.

### Not in this adaptor

- `gpui-kit` umbrella (on disk, not a workspace member).
- Story gallery, WASM, `gpui-shell`, `gpui-wry` as workspace members.
- Actual crates.io upload. Publish order after wgpui 0.3.5 exists: macros → assets → base → component.

## 0.5.2

Prior adaptor of Longbridge gpui-component v0.5.2 onto wgpui 0.3.4 (`2177ce92`).
