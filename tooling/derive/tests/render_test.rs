#[test]
fn test_derive_render() {
    use wgpui_derive::Render;

    #[derive(Render)]
    struct _Element;
}
