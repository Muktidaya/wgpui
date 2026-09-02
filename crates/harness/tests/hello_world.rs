use gpui::{
    AppContext, Context, IntoElement, ParentElement, Render, TestAppContext, Window, div,
};
use gpui_component::{button::*, *};
use gpui_component_assets::Assets;

struct Example;

impl Render for Example {
    fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
        div()
            .v_flex()
            .gap_2()
            .size_full()
            .items_center()
            .justify_center()
            .child("Hello, World!")
            .child(
                Button::new("ok")
                    .primary()
                    .label("Let's Go!")
                    .on_click(|_, _, _| {}),
            )
    }
}

#[test]
fn assets_load() {
    let _ = Assets;
}

#[gpui::test]
fn hello_world_window_opens(cx: &mut TestAppContext) {
    cx.skip_drawing();
    cx.update(|cx| gpui_component::init(cx));
    let _window = cx.add_window(|_window, cx| Example);
}
