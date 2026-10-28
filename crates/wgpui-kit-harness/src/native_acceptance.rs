// The binary uses Kit; framework-only development dependencies also participate in macro lookup.
use wgpui_kit as wgpui;

use wgpui_kit::{
    assets::Assets,
    component::{
        ActiveTheme,
        button::{Button, ButtonVariants as _},
        input::{Input, InputState},
    },
    prelude::*,
    *,
};

actions!(native_acceptance, [Increment, Quit]);

struct NativeAcceptance {
    input: Entity<InputState>,
    last_value: SharedString,
    count: usize,
    status: SharedString,
    _input_observer: Subscription,
}

impl NativeAcceptance {
    fn new(window: &mut Window, cx: &mut Context<Self>) -> Self {
        let input = cx.new(|cx| InputState::new(window, cx));
        let observer = cx.observe(&input, |_, _, cx| cx.notify());
        Self {
            input,
            last_value: "".into(),
            count: 0,
            status: "Ready".into(),
            _input_observer: observer,
        }
    }

    fn increment(&mut self, cx: &mut Context<Self>) {
        self.count += 1;
        println!("NATIVE count={}", self.count);
        cx.notify();
    }

    fn pick(&mut self, files: bool, directories: bool, multiple: bool, cx: &mut Context<Self>) {
        self.status = "Picker pending".into();
        cx.notify();
        let timer = cx
            .background_executor()
            .timer(std::time::Duration::from_millis(500));
        cx.spawn(async move |this, cx| {
            timer.await;
            this.update(cx, |this, _| {
                if this.status.as_ref() == "Picker pending" {
                    println!("NATIVE picker=pending foreground=responsive");
                }
            })?;
            Ok::<(), wgpui_kit::private::anyhow::Error>(())
        })
        .detach_and_log_err(cx);
        let response = cx.prompt_for_paths(PathPromptOptions {
            files,
            directories,
            multiple,
            prompt: Some("WGPUI native acceptance".into()),
        });
        cx.spawn(async move |this, cx| {
            let status = match response.await {
                Ok(Ok(Some(paths))) => {
                    println!(
                        "NATIVE picker=selected count={} paths={paths:?}",
                        paths.len()
                    );
                    format!("Selected {} path(s)", paths.len())
                }
                Ok(Ok(None)) => {
                    println!("NATIVE picker=cancelled");
                    "Picker cancelled".to_owned()
                }
                Ok(Err(error)) => {
                    eprintln!("NATIVE picker=error {error:#}");
                    format!("Picker failed: {error:#}")
                }
                Err(error) => {
                    eprintln!("NATIVE picker=channel-error {error}");
                    format!("Picker channel failed: {error}")
                }
            };
            this.update(cx, |this, cx| {
                this.status = status.into();
                cx.notify();
            })?;
            Ok::<(), wgpui_kit::private::anyhow::Error>(())
        })
        .detach_and_log_err(cx);
    }

    fn save(&mut self, cx: &mut Context<Self>) {
        let response = cx.prompt_for_new_path(&std::env::temp_dir(), Some("WGPUI-Δ.txt"));
        cx.spawn(async move |this, cx| {
            let status = match response.await {
                Ok(Ok(Some(path))) => {
                    println!("NATIVE save=selected path={path:?}");
                    "Save path selected".to_owned()
                }
                Ok(Ok(None)) => {
                    println!("NATIVE save=cancelled");
                    "Save picker cancelled".to_owned()
                }
                Ok(Err(error)) => {
                    eprintln!("NATIVE save=error {error:#}");
                    format!("Save picker failed: {error:#}")
                }
                Err(error) => {
                    eprintln!("NATIVE save=channel-error {error}");
                    format!("Save picker channel failed: {error}")
                }
            };
            this.update(cx, |this, cx| {
                this.status = status.into();
                cx.notify();
            })?;
            Ok::<(), wgpui_kit::private::anyhow::Error>(())
        })
        .detach_and_log_err(cx);
    }

    fn read_clipboard(&mut self, cx: &mut Context<Self>) {
        let response = cx.read_from_clipboard_async();
        cx.spawn(async move |this, cx| {
            let status = match response.await {
                Ok(Some(item)) => {
                    let text = item.text().unwrap_or_default();
                    println!("NATIVE clipboard={text:?}");
                    format!("Clipboard: {text}")
                }
                Ok(None) => {
                    println!("NATIVE clipboard=empty");
                    "Clipboard is empty".to_owned()
                }
                Err(error) => {
                    eprintln!("NATIVE clipboard=error {error:#}");
                    format!("Clipboard failed: {error:#}")
                }
            };
            this.update(cx, |this, cx| {
                this.status = status.into();
                cx.notify();
            })?;
            Ok::<(), wgpui_kit::private::anyhow::Error>(())
        })
        .detach_and_log_err(cx);
    }
}

impl Render for NativeAcceptance {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let value = self.input.read(cx).value();
        if value != self.last_value {
            println!("NATIVE input={value:?}");
            self.last_value = value;
        }
        div()
            .id("native-acceptance")
            .size_full()
            .flex()
            .flex_col()
            .p_4()
            .gap_3()
            .bg(cx.theme().background)
            .text_color(cx.theme().foreground)
            .on_action(cx.listener(|this, _: &Increment, _, cx| this.increment(cx)))
            .child("WGPUI native acceptance")
            .child(Input::new(&self.input).id("input").w(px(420.)))
            .child(format!("Count: {}", self.count))
            .child(self.status.clone())
            .child(
                Button::new("increment")
                    .primary()
                    .label("Increment")
                    .on_click(cx.listener(|this, _, _, cx| this.increment(cx))),
            )
            .child(
                Button::new("copy")
                    .label("Copy input")
                    .on_click(cx.listener(|this, _, _, cx| {
                        let value = this.input.read(cx).value().to_string();
                        cx.write_to_clipboard(ClipboardItem::new_string(value.clone()));
                        println!("NATIVE copied={value:?}");
                        this.status = "Copied input".into();
                        cx.notify();
                    })),
            )
            .child(
                Button::new("read")
                    .label("Read clipboard")
                    .on_click(cx.listener(|this, _, _, cx| this.read_clipboard(cx))),
            )
            .child(
                Button::new("file")
                    .label("Select one file")
                    .on_click(cx.listener(|this, _, _, cx| this.pick(true, false, false, cx))),
            )
            .child(
                Button::new("files")
                    .label("Select multiple files")
                    .on_click(cx.listener(|this, _, _, cx| this.pick(true, false, true, cx))),
            )
            .child(
                Button::new("folder")
                    .label("Select a folder")
                    .on_click(cx.listener(|this, _, _, cx| this.pick(false, true, false, cx))),
            )
            .child(
                Button::new("save")
                    .label("Select a save path")
                    .on_click(cx.listener(|this, _, _, cx| this.save(cx))),
            )
            .child(
                Button::new("second")
                    .label("Open another window")
                    .on_click(|_, _, cx| open_window(cx, "WGPUI second window")),
            )
            .child(
                Button::new("close")
                    .label("Close this window")
                    .on_click(|_, window, _| window.remove_window()),
            )
            .child(
                Button::new("quit")
                    .label("Quit")
                    .on_click(|_, _, cx| cx.quit()),
            )
    }
}

fn open_window(cx: &mut App, title: &'static str) {
    cx.spawn(async move |cx| {
        cx.open_window(
            WindowOptions {
                titlebar: Some(TitlebarOptions {
                    title: Some(title.into()),
                    ..Default::default()
                }),
                window_bounds: Some(WindowBounds::Windowed(Bounds {
                    origin: point(px(40.), px(40.)),
                    size: size(px(620.), px(780.)),
                })),
                ..Default::default()
            },
            |window, cx| {
                let view = cx.new(|cx| NativeAcceptance::new(window, cx));
                cx.new(|cx| base::Root::new(view, window, cx))
            },
        )?;
        println!("NATIVE window=opened title={title:?}");
        Ok::<(), wgpui_kit::private::anyhow::Error>(())
    })
    .detach_and_log_err(cx);
}

fn main() {
    wgpui_kit::application().with_assets(Assets).run(|cx| {
        wgpui_kit::init(cx);
        cx.on_action(|_: &Quit, cx| cx.quit());
        let (increment_binding, quit_binding) = if cx.platform_info().is_mac() {
            ("cmd-i", "cmd-q")
        } else {
            ("ctrl-i", "ctrl-q")
        };
        cx.bind_keys([
            KeyBinding::new(increment_binding, Increment, None),
            KeyBinding::new(quit_binding, Quit, None),
        ]);
        cx.set_menus(vec![Menu {
            name: "Acceptance".into(),
            items: vec![
                MenuItem::action("Increment", Increment),
                MenuItem::action("Quit", Quit),
            ],
            ..Default::default()
        }]);
        open_window(cx, "WGPUI native acceptance");
    });
}
