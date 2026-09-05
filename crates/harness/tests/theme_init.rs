extern crate gpui as wgpui;

use gpui::TestAppContext;
use gpui_component::{ActiveTheme as _, ThemeRegistry};

#[gpui::test]
fn init_registers_default_theme(cx: &mut TestAppContext) {
    cx.update(|cx| {
        gpui_component::init(cx);
        assert!(cx.has_global::<ThemeRegistry>());
        assert!(cx.theme().background.a > 0.0);
    });
}
