// Adapted from the historical GPUI Component/Kit window_title example; see docs/kit/upstream-provenance.md.
use wgpui_kit::component::{
    TitleBar,
    button::{Button, ButtonVariants},
    h_flex, v_flex,
};
use wgpui_kit::*;

pub struct Example;
impl Render for Example {
    fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
        v_flex()
            .size_full()
            .child(
                // Render custom title bar on top of Root view.
                TitleBar::new().child(
                    h_flex()
                        .w_full()
                        .pr_2()
                        .justify_between()
                        .child("App with Custom title bar")
                        .child("Right Item"),
                ),
            )
            .child(
                div()
                    .id("window-body")
                    .p_5()
                    .size_full()
                    .items_center()
                    .justify_center()
                    .child("Hello, World!")
                    .child(
                        Button::new("ok")
                            .primary()
                            .label("Let's Go!")
                            .on_click(|_, _, _| println!("Clicked!")),
                    ),
            )
    }
}

fn main() {
    wgpui_kit::application().run(|cx| {
        wgpui_kit::init(cx);
        let options = TitleBar::window_options();
        if let Err(error) = wgpui_kit::open_window(options, cx, |_, cx| cx.new(|_| Example)) {
            eprintln!("Could not open example: {error:#}");
            cx.quit();
        }
    });
}
