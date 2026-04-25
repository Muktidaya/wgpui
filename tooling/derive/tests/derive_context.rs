#[cfg(not(rust_analyzer))]
#[test]
#[ignore = "VisualContext trait shape differs between wgpui and wgpui-ce macro output"]
fn test_derive_context() {
    use wgpui::{App, Window};
    use wgpui_derive::{AppContext, VisualContext};

    #[derive(AppContext, VisualContext)]
    struct _MyCustomContext<'a, 'b> {
        #[app]
        app: &'a mut App,
        #[window]
        window: &'b mut Window,
    }
}
