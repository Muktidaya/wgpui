#!/usr/bin/env python3
"""Compile and run tests with Kit as the consumer's sole direct dependency."""
import json
import os
from pathlib import Path
import shutil
import subprocess
import tempfile

root = Path(__file__).resolve().parents[1]
source = '''use UI::{App, Context, IntoElement, Render, RenderOnce, TestAppContext, Window, div};
use UI::prelude::*;
UI::actions!(consumer, [Activate]);
#[derive(UI::IntoElement)]
struct Badge;
impl RenderOnce for Badge {
    fn render(self, _: &mut Window, _: &mut App) -> impl IntoElement { div().child("Kit") }
}
struct View;
impl Render for View {
    fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
        div().child(Badge)
    }
}
#[UI::test]
fn facade_initializes_and_renders(cx: &mut TestAppContext) {
    cx.update(UI::init);
    cx.add_window(|_, _| View);
    cx.run_until_parked();
    assert_eq!(UI::Action::name(&Activate), "consumer::Activate");
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
#[derive(IntoPlot)]
struct PlotFixture;
impl Plot for PlotFixture {
    fn paint(&mut self, _: UI::Bounds<UI::Pixels>, _: &mut Window, _: &mut App) {}
}
#[test]
fn plot_macro_resolves_the_facade() {
    let _: PlotFixture = PlotFixture.into_element();
}
"""
        (directory / "src/lib.rs").write_text(consumer_source.replace("UI", alias))
        (directory / "Cargo.toml").write_text(
            '[package]\nname = "kit-consumer-check"\nversion = "0.0.0"\nedition = "2024"\npublish = false\n'
            '[dependencies]\n' + alias + ' = { package = "wgpui-kit", path = '
            + json.dumps(str(root / 'crates/wgpui-kit'))
            + ', default-features = ' + str(defaults).lower() + ', features = ["test-support"] }\n'
        )
        shutil.copy2(root / "Cargo.lock", directory / "Cargo.lock")
        environment = dict(os.environ)
        environment.setdefault("CARGO_TARGET_DIR", str(root / "target"))
        subprocess.run(["cargo", "+1.94.0", "test", "--offline", "--manifest-path",
                        str(directory / "Cargo.toml"), "--lib"], env=environment, check=True, cwd=directory)
        print(f"PASS sole Kit dependency: alias={alias}, defaults={defaults}", flush=True)
