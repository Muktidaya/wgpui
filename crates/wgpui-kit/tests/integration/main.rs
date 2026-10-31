//! Kit's integration tests, in one binary.
//!
//! Each file under `tests/` is a binary of its own, linked with all of GPUI
//! and every component -- about 230 MB apiece in a debug build, thirty of
//! them. So they are modules of this one, and a module is compiled where
//! the features it needs are on, which is what each file's `[[test]]` entry
//! used to say with `required-features`.
//!
//! Three stay targets of their own. `rendering` needs a real renderer and its
//! own harness, and is selected with `--test rendering`. `ui` is the example
//! the testing guide shows and tells a reader to copy as a `tests/ui.rs` of
//! their own, beside `tests/common/mod.rs`, so it stays in that shape.
//! `parity_ui` keeps its application-facing acceptance example independent.

#[cfg(feature = "test-support")]
#[path = "../support/common.rs"]
mod common;

#[cfg(feature = "assets")]
mod assets;
#[cfg(all(feature = "test-support", feature = "component"))]
mod collection_steps;
#[cfg(all(feature = "test-support", feature = "component"))]
mod collections;
#[cfg(all(feature = "test-support", feature = "component"))]
mod components;
#[cfg(all(feature = "test-support", feature = "component"))]
mod composition_helpers;
#[cfg(all(feature = "test-support", feature = "component"))]
mod control_steps;
#[cfg(all(feature = "test-support", feature = "component"))]
mod controls;
#[cfg(all(feature = "test-support", feature = "component"))]
mod date_picker;
#[cfg(all(feature = "test-support", feature = "component"))]
mod diff;
#[cfg(all(feature = "test-support", feature = "component"))]
mod disclosure;
#[cfg(all(feature = "test-support", feature = "component"))]
mod dock;
#[cfg(all(feature = "test-support", feature = "component"))]
mod environment;
#[cfg(feature = "test-support")]
mod event_helpers;
mod exports;
#[cfg(all(feature = "test-support", feature = "component"))]
mod input;
#[cfg(all(feature = "test-support", feature = "component"))]
mod input_focus;
#[cfg(feature = "test-support")]
mod interactions;
#[cfg(feature = "test-support")]
mod lifecycle;
#[cfg(all(feature = "test-support", feature = "component"))]
mod menu;
#[cfg(all(feature = "test-support", feature = "component"))]
mod overlays;
#[cfg(feature = "test-support")]
mod query_helpers;
#[cfg(all(feature = "test-support", feature = "component"))]
mod root;
#[cfg(all(feature = "assets", feature = "test-support", feature = "component"))]
mod search;
#[cfg(all(feature = "test-support", feature = "component"))]
mod semantic_controls;
#[cfg(all(feature = "test-support", feature = "component"))]
mod slider_steps;
mod test_macro;
#[cfg(all(feature = "test-support", feature = "component"))]
mod text_selection;
#[cfg(all(feature = "test-support", feature = "component"))]
mod touch_selection;
#[cfg(feature = "test-support")]
mod window;

// Raw framework windows intentionally exercise setup without Base Root.
#[cfg(all(feature = "test-support", feature = "component"))]
#[path = "framework_windows/collections.rs"]
mod framework_collections;
#[cfg(all(feature = "test-support", feature = "component"))]
#[path = "framework_windows/components.rs"]
mod framework_components;
#[cfg(all(feature = "test-support", feature = "component"))]
#[path = "framework_windows/controls.rs"]
mod framework_controls;
#[cfg(all(feature = "test-support", feature = "component"))]
#[path = "framework_windows/date_picker.rs"]
mod framework_date_picker;
#[cfg(all(feature = "test-support", feature = "component"))]
#[path = "framework_windows/disclosure.rs"]
mod framework_disclosure;
#[cfg(all(feature = "test-support", feature = "component"))]
#[path = "framework_windows/dock.rs"]
mod framework_dock;
#[cfg(all(feature = "test-support", feature = "component"))]
#[path = "framework_windows/input.rs"]
mod framework_input;
#[cfg(feature = "test-support")]
#[path = "framework_windows/interactions.rs"]
mod framework_interactions;
#[cfg(feature = "test-support")]
#[path = "framework_windows/lifecycle.rs"]
mod framework_lifecycle;
#[cfg(all(feature = "test-support", feature = "component"))]
#[path = "framework_windows/menu.rs"]
mod framework_menu;
#[cfg(all(feature = "test-support", feature = "component"))]
#[path = "framework_windows/overlays.rs"]
mod framework_overlays;
#[cfg(all(feature = "assets", feature = "test-support", feature = "component"))]
#[path = "framework_windows/search.rs"]
mod framework_search;
#[cfg(feature = "test-support")]
#[path = "framework_windows/window.rs"]
mod framework_window;
