extern crate gpui as wgpui;

use gpui::{
    AppContext, Context, IntoElement, ParentElement, Render, Styled, TestAppContext, Window,
    WindowOptions, div,
};
use gpui_component::{button::*, *};

struct ButtonView;

impl Render for ButtonView {
    fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
        div()
            .size_full()
            .items_center()
            .justify_center()
            .child(Button::new("test").primary().label("Click me"))
    }
}

#[gpui::test]
fn primary_button_renders(cx: &mut TestAppContext) {
    cx.skip_drawing();
    cx.update(|cx| {
        gpui_component::init(cx);
        cx.open_window(WindowOptions::default(), |_window, cx| cx.new(|_| ButtonView))
            .expect("open window");
    });
}
