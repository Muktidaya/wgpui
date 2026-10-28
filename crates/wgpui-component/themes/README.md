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
roles use current defaults. The release working tree and Cargo archives contain
the same 13 catalogs. Eight catalogs were removed from the release working tree
pending individual license-scope review, with their original bytes recoverable
in Git history and their scoped notices retained. [catalog-index.json](catalog-index.json)
records shipped and history-only names. Both tests use the same strict role,
color, provenance, uniqueness and production-loader assertions; the source test
also checks the exact directory roster. Missing retained catalogs fail rather
than being skipped.

```sh
# Release working tree: exact 13-file directory roster
cargo +1.95.0 test -p wgpui-component --test source_theme_catalog
# Release working tree or unpacked crate: all 13 shipped catalogs
cargo +1.95.0 test -p wgpui-component --test packaged_theme_catalog
```

Earlier commits and snapshots still contain the eight removed files. Their
removal is a distribution-scope precaution, not a licensing clearance claim.
The built-in default theme and explicit loading APIs are unchanged.

Legacy `link.*.foreground`, chart-direction and window-border names are mapped
to current roles. Retired panel, action-bar, tiles and accordion-hover roles have
no current equivalent and use current semantic defaults; their original values
remain recoverable at the source pin. Every remaining role and color expression
is checked, so serde's unknown-field fallback cannot hide an incompatible key.

The repository also retains a source-only dock-export boundary test that reads
both Base and Component source modules. It is excluded from Cargo archives;
standalone package tests do not require sibling workspace source directories.
