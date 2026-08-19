//! Path GPU pass example.
//!
//! Draws filled vector paths through `PathBuilder` and `window.paint_path`.
//! This is the discriminating visual for WGPUI 0.3.4 path rasterization.

use wgpui::{
    Application, Bounds, Context, PathBuilder, Render, Window, WindowBounds, WindowOptions, canvas,
    div, point, prelude::*, px, rgb, size,
};

struct PathsExample;

impl Render for PathsExample {
    fn render(&mut self, _window: &mut Window, _cx: &mut Context<Self>) -> impl IntoElement {
        div().size_full().bg(rgb(0x1a1d27)).child(
            canvas(
                |_, _, _| {},
                |bounds, _, window, _| {
                    let mut star = PathBuilder::fill();
                    let center = point(
                        bounds.origin.x + bounds.size.width / 2.,
                        bounds.origin.y + bounds.size.height / 2.,
                    );
                    for i in 0..10 {
                        let angle = std::f32::consts::PI / 2.0
                            - (i as f32) * std::f32::consts::PI / 5.0;
                        let radius = if i % 2 == 0 { 120.0 } else { 50.0 };
                        let vertex = point(
                            center.x + px(angle.cos() * radius),
                            center.y - px(angle.sin() * radius),
                        );
                        if i == 0 {
                            star.move_to(vertex);
                        } else {
                            star.line_to(vertex);
                        }
                    }
                    if let Ok(path) = star.build() {
                        window.paint_path(path, rgb(0xfacc15));
                    }

                    let mut ring = PathBuilder::stroke(px(8.));
                    ring.move_to(point(bounds.origin.x + px(40.), bounds.origin.y + px(40.)));
                    ring.line_to(point(
                        bounds.origin.x + bounds.size.width - px(40.),
                        bounds.origin.y + px(40.),
                    ));
                    ring.curve_to(
                        point(
                            bounds.origin.x + bounds.size.width - px(40.),
                            bounds.origin.y + bounds.size.height - px(40.),
                        ),
                        point(
                            bounds.origin.x + bounds.size.width - px(40.),
                            bounds.origin.y + bounds.size.height / 2.,
                        ),
                    );
                    if let Ok(path) = ring.build() {
                        window.paint_path(path, rgb(0x38bdf8));
                    }
                },
            )
            .size_full(),
        )
    }
}

fn main() {
    Application::new().run(|cx| {
        cx.open_window(
            WindowOptions {
                window_bounds: Some(WindowBounds::Windowed(Bounds::centered(
                    None,
                    size(px(640.), px(480.)),
                    cx,
                ))),
                ..Default::default()
            },
            |_, cx| cx.new(|_| PathsExample),
        )
        .unwrap();
    });
}
