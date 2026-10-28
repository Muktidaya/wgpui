# Optional theme catalog

Adapted from the historical GPUI Component/Kit theme catalog. Each JSON file
preserves its original theme name, author and source URL. The source snapshot
and original histories are recorded in [upstream provenance](../../../docs/kit/upstream-provenance.md).
The retained [Apache-2.0 license](LICENSE-APACHE) accompanies the catalog adaptation.
[Source-specific notices and open scope findings](SOURCE-NOTICES.md) apply to
the underlying palettes; schema compatibility is separate from license review.

These themes are optional data, not an automatically loaded bundle. Use
`ThemeRegistry::load_themes_from_str` or an application-owned watched directory.
The current `ThemeSet`/`ThemeConfig` types define compatibility; missing new
roles use current defaults. The source-tree production-loader test validates all 21 files. The Cargo package
ships 13 catalogs; eight source-only catalogs remain in Git pending individual
license-scope review. [catalog-index.json](catalog-index.json) records both
rosters. The packaged test validates every shipped catalog with the same strict
role, color, provenance, uniqueness and loading assertions. Missing source
catalogs fail the source test rather than being skipped.

```sh
# Full repository source: all 21, including source-only catalogs
cargo +1.95.0 test -p wgpui-component --test source_theme_catalog
# Repository source or unpacked crate: all 13 shipped catalogs
cargo +1.95.0 test -p wgpui-component --test packaged_theme_catalog
```

This is a Cargo packaging distinction. GitHub source snapshots still contain
all 21 catalogs. The built-in default theme and explicit loading APIs are unchanged.

Legacy `link.*.foreground`, chart-direction and window-border names are mapped
to current roles. Retired panel, action-bar, tiles and accordion-hover roles have
no current equivalent and use current semantic defaults; their original values
remain recoverable at the source pin. Every remaining role and color expression
is checked, so serde's unknown-field fallback cannot hide an incompatible key.
