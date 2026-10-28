# wgpui-kit-assets

Bundled Lucide icons for WGPUI Kit, adapted from Longbridge GPUI Kit v0.8.1.
The current source candidate is **wgpui-kit-assets 0.8.1**, paired with
**WGPUI 0.4.0**, and requires **Rust 1.95**. Validation and publication are
in progress; see [release status](../../STATUS.md). The published assets/Kit
0.6.1 and framework 0.3.6 releases remain separate from this candidate.

Use `wgpui_kit::assets` through Kit's default `assets` feature, or depend on
this package directly. `Assets` embeds the default component bundle on
native platforms; `AllAssets` embeds the full catalog. `IconName` and
`IconNamed` supply icon identities. Use `icon_assets!` to embed a selected
subset.

```rust
use wgpui_kit::assets::{icon_assets, IconName};

icon_assets!(AppAssets, [Search, Check]);
```

A selected subset contains only the named icons. Compose it with `Assets`
when the application also needs the default component icons.

The package lives at `crates/wgpui-kit-assets` in the
[consolidated WGPUI repository](https://github.com/Muktidaya/wgpui).
Its Cargo `links` identity is `wgpui-kit-default-icons`. Earlier
`wgpui-component-assets` releases are unchanged. See the
[Kit guide](../../docs/kit.md) for package paths and backend boundaries.

The Rust implementation is licensed under Apache-2.0; its Longbridge
attribution and terms are in [LICENSE-APACHE](LICENSE-APACHE). Bundled icons
retain the ISC license and the MIT notice for Feather-derived icons in
[LICENSE-LUCIDE](LICENSE-LUCIDE).
