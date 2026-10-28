# Themes and selected examples

The optional JSON catalog in `crates/wgpui-component/themes/` preserves the
original theme names, authors and source URLs. It is not loaded automatically.
Load a chosen file through `ThemeRegistry::load_themes_from_str`, or watch an
application-owned theme directory through `ThemeRegistry::watch_dir`. The current
`ThemeSet`/`ThemeConfig` Rust types define compatibility; old mutable upstream
schema links were removed from the copied catalog.

The release working tree and Cargo source archives contain the same 13 catalogs.
Both validation targets parse every set, check color keys and values, and load
it through the production registry. Eight catalogs with unresolved scope were
removed from the release working tree; the [catalog index](../../crates/wgpui-component/themes/catalog-index.json)
records their names for recovery in Git history. Their scoped notices remain.
No missing-file skip or relaxed schema check is used. New theme roles may use
current defaults when an older catalog does not supply them. Theme attribution,
applicable source-specific terms and historical findings are in the
[catalog notices](../../crates/wgpui-component/themes/SOURCE-NOTICES.md).
Removal is a distribution-scope precaution and does not clear historical rights
questions or relicense the underlying palettes.

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
