# wgpui-component adapter status

**GitHub:** private [wgpui-component](https://github.com/Muktidaya/wgpui-component) (`root`). Longbridge remains `upstream`.

**Upstream pin:** `94a313a7` (gpui-component **v0.6.0**, 2026-09-03) — `vendor/UPSTREAM_SHA`

**Stack:** `wgpui = { version = "0.3.5", path = "../wgpui" }` (and the `gpui` alias of the same). Rust 1.94.0.

**Merge:** complete on `root` as `7272570f` (parents `2177ce92` + `94a313a7`). Packaging follow-up `28707b21` dropped unpublished `wgpui-platform` from the published `wgpui-base` graph.

**crates.io (2026-09-05):** **0.6.0 published** — macros → assets → base → component from `28707b21`. `wgpui-platform` and `gpui-wry` held. `wgpui` **not** republished (still 0.3.5).

**CI:** [33993771674](https://github.com/Muktidaya/wgpui-component/actions/runs/33993771674) on `7272570f` **red** (`--all-targets`: `gpui_component` / `gpui_base` leftovers). [33994263316](https://github.com/Muktidaya/wgpui-component/actions/runs/33994263316) on `ebf839fb` still **red**: `--lib --tests` compiled the unpublished `wgpui-base` showcase **bin**. Fix: `autobins = false` / `autoexamples = false` on `wgpui-base`; alias `crates/base/tests/element_ext.rs`. Local `cargo +1.94.0 check --locked --workspace --lib --tests` is green.

## 0.6.0 port (2026-09-05) — compiling sync + lib tests green

**Strategy:** `git merge --no-ff v0.6.0` onto adaptor `2177ce92` (base `5cb09462`). Did **not** take the 32 commits after the tag.

**21 commits brought in (`5cb09462..v0.6.0`):** NavStack; History / UndoHistory split (public base API); dock reconcile + tiles persistence; button/tab/sheet/notification/settings fixes; repo rename `crates/ui`→`crates/component` (mapped back to `crates/ui`); assets `links = "gpui-kit-default-icons"` (crate name stays `wgpui-component-assets`).

**Preserved local work:** CI rewrite (sibling `wgpui` checkout, triggers on `root`); `extern crate gpui as wgpui`; `ArenaClearNeeded::clear()`; motion `mul_f64`; blink-cursor `Result` handling; SharedString/`ArcCow` / `Option<TextStyleRefinement>` remaps; `publish = false` workspace default with explicit `publish = true` on component/base/macros/assets; `gpui-wry` `publish = false`; platform 0.6.0 unpublished. Stash `pre-0.6.0-port: local adaptor CI/publish/API remaps` is still present as backup.

**Not adopted:** `gpui-kit` umbrella (on disk, not a workspace member). `examples/hello_world` uses public names `wgpui` / `wgpui_component`.

## Version pins

| Crate | Version | publish |
|-------|---------|---------|
| `wgpui-component` (`crates/ui`) | 0.6.0 | true |
| `wgpui-base` | 0.6.0 | true |
| `wgpui-component-macros` | 0.6.0 | true |
| `wgpui-component-assets` | 0.6.0 | true |
| `wgpui-platform` | 0.6.0 | **false** (shim) |
| `gpui-wry` | 0.6.0 | **false** (name owned by Longbridge; not a workspace member) |
| workspace default | — | false |
| path `wgpui` / `wgpui_derive` | 0.3.5 | path only |

## Verify (`cargo +1.94.0`)

| Command | Result |
|---------|--------|
| `check -p wgpui-base --lib` | ok |
| `check -p wgpui-component --lib` | ok |
| `check -p hello_world` | ok |
| `test -p wgpui-base --lib` | **764 passed** |
| `test -p wgpui-component --lib` | **417 passed** |
| `test -p wgpui-component-harness` | **4 passed** |
| `publish --dry-run` then `publish -p wgpui-component-macros` | **published 0.6.0** |
| `publish --dry-run` then `publish -p wgpui-component-assets` | **published 0.6.0** (wgpui 0.3.5 was the previous blocker) |
| `publish --dry-run` then `publish -p wgpui-base` | **published 0.6.0** after dropping unused native `gpui_platform` |
| `publish --dry-run` then `publish -p wgpui-component` | **published 0.6.0** |

Sibling `wgpui` was **not** republished. crates.io `wgpui` / `wgpui_derive` remain **0.3.5**.

### 9 `wgpui-component` `--lib` failures fixed (2026-09-05) — adaptor, not wgpui

Did **not** edit sibling `wgpui`. Did **not** take post-tag `upstream/main` commits.

| Test | Cause | Fix |
|------|--------|-----|
| `input::editor::tests::the_rows_follow_the_font_size` | `StyleRefinement.text` refine **replaces** the `Option`, so `.text_size(24)` dropped `line_height(1.5)` and fell through to φ≈1.618 → 39px | `refine_style_refinement` merges nested text |
| `inspector::tests::test_rust_to_style` | WGPUI inspector expander matches `wgpui_macros::p_1`, but `styled.rs` calls `wgpui_derive::padding_style_methods!()` — `p_1`/`mx_2` never entered `styled_reflection` | Re-expand those macros on a local `TokenStyled` trait |
| `message_scroller::tests::test_message_scroller_state_builder` | WGPUI `ListState::scroll_to` never calls `stop_following` | `scroll_to_item` sets `FollowMode::Normal` |
| `scroll::scrollable::tests::{scrollable,horizontal}_flex_item_shrinks_below_its_content` and `overflow_y_scrollbar_preserves_gap_for_exact_issue_chain` | Inner content kept `flex_1`'s `flex_basis: 0%` after `flex_none()`, so content sized to the viewport and could not scroll | Inner `.flex_basis(Length::Auto)` |
| `spinner::tests::reduced_motion_spinner_is_static_and_requests_no_frame` | Spinner always `with_animation`, unlike shimmer/progress | Skip animation when `cx.reduce_motion()` |
| `text::tests::legacy_partial_styles_refine_component_theme_defaults` | Same text-option replace as editor | `compat::resolve_component_style` uses `refine_style_refinement` |
| `text::window_selection::tests::drag_inside_dialog_still_selects_its_text` | Open animation `top(y * delta)` at t≈0 parked the panel under the title bar (WGPUI animation uses wall `Instant`; `settle(500ms)` cannot finish it) | Keep resting `.top(y)`; animate shadow only. Backdrop fade stays on the overlay, not the occluding parent |

## Divergence from upstream v0.6.0 (intentional)

- Depend on **wgpui 0.3.5**, not `gpui-pre` 0.3.1
- Crate names `wgpui-*`, not `gpui-kit` / `gpui-kit-assets`
- Layout `crates/ui` + `crates/macros` (upstream `crates/component` + `crates/component-macros`)
- Reduced workspace (no story / kit / shell / wasm / webview members)
- hello_world uses `wgpui` / `wgpui_component` (public names)
- Adaptor remaps listed above
- Stopped at tag; 32 later `upstream/main` commits not taken

## crates.io (published 2026-09-05)

From clean `root` `28707b21` with `cargo +1.94.0 publish` (dry-run then real, no `--allow-dirty`, no yank):

| Crate | URL |
|-------|-----|
| `wgpui-component-macros` 0.6.0 | https://crates.io/crates/wgpui-component-macros/0.6.0 |
| `wgpui-component-assets` 0.6.0 | https://crates.io/crates/wgpui-component-assets/0.6.0 |
| `wgpui-base` 0.6.0 | https://crates.io/crates/wgpui-base/0.6.0 |
| `wgpui-component` 0.6.0 | https://crates.io/crates/wgpui-component/0.6.0 |

**Held:** `wgpui-platform` (`publish = false`; crates.io 404). Workspace `gpui-wry` (`publish = false`; Longbridge owns the crates.io name). **Not republished:** `wgpui` / `wgpui_derive` (still 0.3.5).

**Tag:** origin has no `v0.6.0`. Local `v0.6.0` is the Longbridge upstream tag at `94a313a7`, not the adaptor publish commit. Not pushed; not force-moved.

**Packaging fix first:** `28707b21` removed the unused native `gpui_platform` (`wgpui-platform`) dep from `wgpui-base` so crates.io would not require an unpublished crate.

## Next actions

1. Optional WGPUI follow-ups (not blocking this adaptor): mark `Style.text` `#[refineable]`; expand `wgpui_derive::` style macros in inspector reflection; `ListState::scroll_to` should `stop_following`; animation should use the test clock.
2. Do not take the 32 post-tag `upstream/main` commits unless asked.
3. If an adaptor tag is wanted later, pick a name that does not collide with upstream `v0.6.0` (or accept that moving `v0.6.0` would be a force-move).
