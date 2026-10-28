# Optional theme catalog

Adapted from the historical GPUI Component/Kit theme catalog. Each JSON file
preserves its original theme name, author and source URL. The source snapshot
and original histories are recorded in [upstream provenance](../../../docs/kit/upstream-provenance.md).
The retained [Apache-2.0 license](LICENSE-APACHE) accompanies this catalog.

These themes are optional data, not an automatically loaded bundle. Use
`ThemeRegistry::load_themes_from_str` or an application-owned watched directory.
The current `ThemeSet`/`ThemeConfig` types define compatibility; missing new
roles use current defaults. The production-loader tests validate every file.

Legacy `link.*.foreground`, chart-direction and window-border names are mapped
to current roles. Retired panel, action-bar, tiles and accordion-hover roles have
no current equivalent and use current semantic defaults; their original values
remain recoverable at the source pin. Every remaining role and color expression
is checked, so serde's unknown-field fallback cannot hide an incompatible key.
