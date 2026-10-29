// Adapted from the historical GPUI Component/Kit tooltip_top_edge example; see docs/kit/upstream-provenance.md.
use wgpui_kit::component::{ActiveTheme as _, Placement, button::*};
use wgpui_kit::*;

struct TooltipTopEdgeExample;

impl Render for TooltipTopEdgeExample {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        div()
            .relative()
            .size_full()
            .bg(cx.theme().background)
            // Keep the trigger pinned to the top edge to exercise tooltip flipping.
            .child(
                div().absolute().top_0().left(px(24.)).child(
                    Button::new("top-edge-tooltip")
                        .primary()
                        .label("Hover for tooltip")
                        .tooltip("This tooltip should appear below the trigger near the top edge.")
                        .tooltip_placement(Placement::Top),
                ),
            )
            .child(
                div()
                    .absolute()
                    .top(px(64.))
                    .left(px(24.))
                    .max_w(px(420.))
                    .text_color(cx.theme().muted_foreground)
                    .child(
                        "Hover the top button. The tooltip should flip below the trigger without changing the original visual gap.",
                    ),
            )
    }
}

fn main() {
    wgpui_kit::application().run(|cx| {
        wgpui_kit::init(cx);
        let options = WindowOptions {
            window_bounds: Some(WindowBounds::centered(size(px(520.), px(260.)), cx)),
            ..Default::default()
        };
        if let Err(error) =
            wgpui_kit::open_window(options, cx, |_, cx| cx.new(|_| TooltipTopEdgeExample))
        {
            eprintln!("Could not open example: {error:#}");
            cx.quit();
        }
    });
}
