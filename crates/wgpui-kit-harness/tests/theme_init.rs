use wgpui::TestAppContext;
use wgpui_component::{ActiveTheme as _, ThemeRegistry};

#[wgpui::test]
fn init_registers_default_theme(cx: &mut TestAppContext) {
    cx.update(|cx| {
        wgpui_component::init(cx);
        assert!(cx.has_global::<ThemeRegistry>());
        assert!(cx.theme().background.a > 0.0);
    });
}
