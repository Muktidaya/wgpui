use wgpui::App;

pub use wgpui_base::GlobalState;

pub(crate) fn init(cx: &mut App) {
    // Preserve the legacy initialization point while `wgpui_base::init` remains
    // after Root initialization for focus-trap ordering compatibility.
    GlobalState::init(cx);
}
