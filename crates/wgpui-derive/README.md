# wgpui_derive

Procedural macros used by [WGPUI](https://github.com/Muktidaya/wgpui):
`Action`, `IntoElement`, `Render`, `AppContext`, `VisualContext`,
`register_action!`, test and benchmark attributes, constructor attributes,
style helpers, and optional inspector reflection.

The current source candidate is **wgpui_derive 0.4.0** for **WGPUI 0.4.0**
and the **WGPUI Kit 0.8.1** facade, with **Rust 1.95** as the minimum.
Candidate validation and publication are in progress. The published
framework/derive 0.3.6 and Kit 0.6.1 releases remain separate; see
[release status](../../STATUS.md).

Applications normally depend on WGPUI or Kit and use their macro reexports.
The corresponding candidate dependency declarations, once published, are:

```toml
[dependencies]
wgpui = "0.4.0"
```

Or, for the Kit facade:

```toml
[dependencies]
wgpui-kit = "0.8.1"
```

Until publication, use path dependencies on this consolidated repository:
the framework is at the root, derive is at `crates/wgpui-derive`, and Kit
is at `crates/wgpui-kit`.

The shared `gpui_path` resolver looks for the `wgpui` package first, then
`wgpui-kit`, and uses that dependency's Cargo key, including a renamed key.
It is used by the `Action`, `IntoElement`, `Render`, `AppContext`, and
`VisualContext` derives, `register_action!`, `#[test]`, `#[bench]`,
`#[ctor]`, and the inspector-reflection attribute. For example, a dependency
declared as `engine = { package = "wgpui", ... }` is resolved through
`engine`, and applications depending only on Kit can use its reexports.
Property-test and style-helper generators retain canonical `wgpui` paths;
the resolver contract does not extend to every macro in this package.

The constructor wrapper resolves the framework's private ctor reexport,
so consumers do not need a direct ctor dependency. Ctor 1 requires an
explicit acknowledgement of initialization safety:

```rust
#[wgpui::ctor(unsafe)]
fn initialize_before_main() {
    // Only operations valid before the application runtime starts belong here.
}
```

An `unsafe fn` with `#[wgpui::ctor]` is also supported. The wrapper forwards
the safety contract rather than supplying an implicit acknowledgement.
See the [Kit compatibility guide](../../docs/kit.md#source-candidate-compatibility)
for the constructor and public dependency contracts.

Licensed under Apache-2.0. [LICENSE-APACHE](LICENSE-APACHE) preserves the
upstream Zed Industries attribution and full license text.
