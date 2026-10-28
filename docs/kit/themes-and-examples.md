# Themes and selected examples

The optional JSON catalog in `crates/wgpui-component/themes/` preserves the
original theme names, authors and source URLs. It is not loaded automatically.
Load a chosen file through `ThemeRegistry::load_themes_from_str`, or watch an
application-owned theme directory through `ThemeRegistry::watch_dir`. The current
`ThemeSet`/`ThemeConfig` Rust types define compatibility; old mutable upstream
schema links were removed from the copied catalog.

Source catalog validation parses all 21 sets, checks color keys and values, and
loads them through the production registry. Cargo source archives contain 13
catalogs with the same strict validation in a package-safe test. Eight catalogs
remain in Git/source only while their license scope is reviewed; the
[catalog index](../../crates/wgpui-component/themes/catalog-index.json) identifies
both rosters. No missing-file skip or relaxed schema check is used. New theme roles may use current defaults
when an older catalog does not supply them. Theme attribution, applicable source-specific terms and open findings are in the
[catalog notices](../../crates/wgpui-component/themes/SOURCE-NOTICES.md); the
retained Apache notice does not relicense the underlying palettes.

Two native examples were selected from the retired snapshot and adapted to Kit's
current window/root setup:

```sh
cargo +1.95.0 run -p wgpui-kit-hello-world --bin tooltip_top_edge
cargo +1.95.0 run -p wgpui-kit-hello-world --bin custom_title_bar
```

The tooltip fixture deliberately uses physical edge geometry to exercise flipping.
The title-bar fixture demonstrates custom native window chrome; platform behavior
still requires interactive verification. Both use current Kit icon assets and
require no retired gallery, font bundle, JavaScript runtime or shell project.
The package's default command remains the existing hello-world example.
