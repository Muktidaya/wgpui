#!/usr/bin/env python3
"""Compile and run public macro consumers with sole Kit or framework dependencies."""
import json
import os
from pathlib import Path
import shutil
import subprocess
import tempfile

root = Path(__file__).resolve().parents[1]
toolchain = os.environ.get("RUSTUP_TOOLCHAIN", "1.95.0")
source = '''use UI::{App, AppContext as _, Context, IntoElement, Render, RenderOnce, TestAppContext, Window, WindowOptions, div};
use UI::prelude::*;
UI::actions!(consumer, [Activate]);
static STARTED: std::sync::atomic::AtomicBool = std::sync::atomic::AtomicBool::new(false);
#[UI::ctor(unsafe)]
fn initialize_consumer() {
    STARTED.store(true, std::sync::atomic::Ordering::SeqCst);
}
#[derive(UI::IntoElement)]
struct Badge;
impl RenderOnce for Badge {
    fn render(self, _: &mut Window, _: &mut App) -> impl IntoElement { div().child("Kit") }
}
#[test]
fn runtime_borrowed_strings_keep_their_owned_value() {
    let shared: UI::SharedString = {
        let temporary = String::from("Δ🦀");
        temporary.as_str().into()
    };
    let optional = Some(shared);
    let borrowed: Option<&str> = optional.as_deref();
    assert_eq!(borrowed, Some("Δ🦀"));
}
struct View;
impl Render for View {
    fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
        div().child(Badge)
    }
}
#[UI::test]
fn facade_initializes_and_renders(cx: &mut TestAppContext) {
    assert!(STARTED.load(std::sync::atomic::Ordering::SeqCst));
    cx.update(UI::init);
    cx.update(|cx| UI::open_window(WindowOptions::default(), cx, |_, cx| cx.new(|_| View)))
        .expect("open Kit root");
    cx.run_until_parked();
    assert_eq!(UI::Action::name(&Activate), "consumer::Activate");
}
'''
framework_source = '''use FRAMEWORK::{App, Context, IntoElement, Render, RenderOnce, TestAppContext, Window, div};
use FRAMEWORK::prelude::*;

FRAMEWORK::actions!(framework_consumer, [Activate]);

#[derive(Clone, Debug, PartialEq, FRAMEWORK::Action)]
#[action(namespace = framework_consumer)]
struct DerivedActivate;

static STARTED: std::sync::atomic::AtomicBool = std::sync::atomic::AtomicBool::new(false);
static RENDERED: std::sync::atomic::AtomicBool = std::sync::atomic::AtomicBool::new(false);
#[FRAMEWORK::ctor(unsafe)]
fn initialize_consumer() {
    STARTED.store(true, std::sync::atomic::Ordering::SeqCst);
}

#[derive(FRAMEWORK::IntoElement)]
struct Badge;
impl RenderOnce for Badge {
    fn render(self, _: &mut Window, _: &mut App) -> impl IntoElement {
        RENDERED.store(true, std::sync::atomic::Ordering::SeqCst);
        div()
    }
}

struct View;
impl Render for View {
    fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
        div().child(Badge)
    }
}

#[test]
fn runtime_borrowed_strings_keep_their_owned_value() {
    let shared: FRAMEWORK::SharedString = {
        let temporary = String::from("Δ🦀");
        temporary.as_str().into()
    };
    let optional = Some(shared);
    let borrowed: Option<&str> = optional.as_deref();
    assert_eq!(borrowed, Some("Δ🦀"));
}

#[FRAMEWORK::test]
fn direct_framework_macros_run_without_caller_aliases(cx: &mut TestAppContext) {
    assert!(STARTED.load(std::sync::atomic::Ordering::SeqCst));
    let window = cx.add_window(|_, _| View);
    cx.update_window(window.into(), |_, window, cx| window.draw(cx).clear())
        .expect("draw framework view");
    cx.run_until_parked();
    assert!(RENDERED.load(std::sync::atomic::Ordering::SeqCst));
    cx.update(|cx| {
        let action = cx.build_action("framework_consumer::Activate", None)
            .expect("actions macro registers its action");
        assert!(action.as_any().is::<Activate>());
        let derived = cx.build_action("framework_consumer::DerivedActivate", None)
            .expect("Action derive registers its action");
        assert!(derived.as_any().is::<DerivedActivate>());
    });
}
'''
style_source = '''use FRAMEWORK::{CursorStyle, Overflow, Position, StyleRefinement, Visibility};

pub trait ProbeStyle: Sized {
    fn style(&mut self) -> &mut StyleRefinement;
    wgpui_derive::style_helpers!();
    wgpui_derive::visibility_style_methods!();
    wgpui_derive::margin_style_methods!();
    wgpui_derive::padding_style_methods!();
    wgpui_derive::position_style_methods!();
    wgpui_derive::overflow_style_methods!();
    wgpui_derive::cursor_style_methods!();
    wgpui_derive::border_style_methods!();
    wgpui_derive::box_shadow_style_methods!();
}

#[derive(Default)]
struct Probe { style: StyleRefinement }
impl ProbeStyle for Probe {
    fn style(&mut self) -> &mut StyleRefinement { &mut self.style }
}

#[test]
fn renamed_style_macro_paths_apply_real_refinements() {
    let probe = Probe::default().invisible().absolute().overflow_hidden()
        .cursor_pointer().border_1().m_2().p_4().rounded_md().shadow_sm();
    assert_eq!(probe.style.visibility, Some(Visibility::Hidden));
    assert_eq!(probe.style.position, Some(Position::Absolute));
    assert_eq!(probe.style.overflow.x, Some(Overflow::Hidden));
    assert_eq!(probe.style.mouse_cursor, Some(CursorStyle::PointingHand));
}
'''
with tempfile.TemporaryDirectory(prefix="wgpui-kit-consumers-") as temporary:
    for alias, defaults in [("wgpui_kit", True), ("ui", True), ("ui", False)]:
        directory = Path(temporary) / (alias + ("-default" if defaults else "-base"))
        (directory / "src").mkdir(parents=True)
        consumer_source = source
        if defaults:
            consumer_source += """
use UI::component::plot::{Plot, IntoPlot};
#[test]
fn borrowed_component_labels_outlive_the_source() {
    let button = {
        let text = String::from("Borrowed Δ🦀");
        UI::component::button::Button::new("borrowed-label").label(text.as_str())
    };
    drop(button);
}
#[derive(IntoPlot)]
struct PlotFixture;
impl Plot for PlotFixture {
    fn paint(&mut self, _: UI::Bounds<UI::Pixels>, _: &mut Window, _: &mut App) {}
}
#[test]
fn plot_macro_resolves_the_facade() {
    let _: UI::component::plot::PlotElement<PlotFixture> = PlotFixture.into_element();
}
"""
        (directory / "src/lib.rs").write_text(consumer_source.replace("UI", alias), encoding="utf-8")
        (directory / "Cargo.toml").write_text(
            '[package]\nname = "kit-consumer-check"\nversion = "0.0.0"\nedition = "2024"\npublish = false\n'
            '[dependencies]\n' + alias + ' = { package = "wgpui-kit", path = '
            + json.dumps(str(root / 'crates/wgpui-kit'))
            + ', default-features = ' + str(defaults).lower() + ', features = ["test-support"] }\n', encoding="utf-8"
        )
        shutil.copy2(root / "Cargo.lock", directory / "Cargo.lock")
        environment = dict(os.environ)
        environment.setdefault("CARGO_TARGET_DIR", str(root / "target"))
        subprocess.run(["cargo", "+" + toolchain, "test", "--offline", "--manifest-path",
                        str(directory / "Cargo.toml"), "--lib"], env=environment, check=True, cwd=directory)
        print(f"PASS sole Kit dependency: alias={alias}, defaults={defaults}", flush=True)

    for alias in ["wgpui", "engine"]:
        directory = Path(temporary) / ("framework-" + alias)
        (directory / "src").mkdir(parents=True)
        (directory / "src/lib.rs").write_text(framework_source.replace("FRAMEWORK", alias), encoding="utf-8")
        (directory / "Cargo.toml").write_text(
            '[package]\nname = "framework-consumer-check"\nversion = "0.0.0"\nedition = "2024"\npublish = false\n'
            '[dependencies]\n' + alias + ' = { package = "wgpui", path = '
            + json.dumps(str(root)) + ', features = ["test-support"] }\n', encoding="utf-8"
        )
        shutil.copy2(root / "Cargo.lock", directory / "Cargo.lock")
        environment = dict(os.environ)
        environment.setdefault("CARGO_TARGET_DIR", str(root / "target"))
        subprocess.run(["cargo", "+" + toolchain, "test", "--offline", "--manifest-path",
                        str(directory / "Cargo.toml"), "--lib"], env=environment, check=True, cwd=directory)
        print(f"PASS sole framework dependency: alias={alias}", flush=True)

    # Explicit macro dependency fixtures, separate from sole-facade consumer checks.
    for alias, package, path in [("engine", "wgpui", root), ("ui", "wgpui-kit", root / "crates/wgpui-kit")]:
        directory = Path(temporary) / ("styles-" + alias)
        (directory / "src").mkdir(parents=True)
        (directory / "src/lib.rs").write_text(style_source.replace("FRAMEWORK", alias), encoding="utf-8")
        (directory / "Cargo.toml").write_text(
            '[package]\nname = "style-consumer-check"\nversion = "0.0.0"\nedition = "2024"\npublish = false\n'
            '[dependencies]\n' + alias + ' = { package = ' + json.dumps(package)
            + ', path = ' + json.dumps(str(path)) + ', default-features = false }\n'
            + 'wgpui_derive = { path = ' + json.dumps(str(root / 'crates/wgpui-derive')) + ' }\n', encoding="utf-8"
        )
        shutil.copy2(root / "Cargo.lock", directory / "Cargo.lock")
        environment = dict(os.environ)
        environment.setdefault("CARGO_TARGET_DIR", str(root / "target"))
        subprocess.run(["cargo", "+" + toolchain, "test", "--offline", "--manifest-path",
                        str(directory / "Cargo.toml"), "--lib"], env=environment, check=True, cwd=directory)
        print(f"PASS renamed style macros: package={package}, alias={alias}", flush=True)
