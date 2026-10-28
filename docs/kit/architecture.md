# Architecture and ownership

WGPUI Kit is the application facade. `crates/wgpui-kit` reexports the native
framework, Base, styled components, platform setup and optional icon assets.
It does not contain a second copy of their implementations.

```text
application
    -> wgpui-kit
        -> wgpui-component (styled controls)
            -> wgpui-base (behavior and geometry)
                -> wgpui (entities, input, layout, native rendering)
        -> wgpui-kit-assets / wgpui-kit-macros / wgpui-platform
```

Base owns reusable interaction, focus, editing, selection, positioning,
virtualization, pane reconciliation and the geometry needed for those behaviors.
Applications and Component own product colors, typography, density, variants,
icons and final composition. Base must not depend on Component themes or facade
classes. Internal `gpui` Cargo aliases do not change these public package names.

Domain values belong to the narrowest feature/model owner. Rendered elements
request semantic changes through callbacks; owners update their values and
notify. Keep entity handles, subscriptions, focus handles and scroll state alive
in an owner rather than reconstructing them on every render.

Use `wgpui_kit::init` before constructing components and `wgpui_kit::open_window`
for the Base root lifecycle and enabled component plugins. One window has one
root; a page or tab does not create another root. Explicit `Root` construction
remains available for hosts that deliberately manage that lifecycle themselves.

Dock trees hold persistent pane structure; entity handles implement the current
view, while the styled Dock skin chooses chrome. Use the public Dock API rather
than duplicating its private reconciliation or depending on retired tiles code.

The active workspace has no browser, webview or shell implementation. Old story,
JavaScript, webview, shell and FPS sources were retired rather than copied into
the facade. See [provenance](upstream-provenance.md) for recovery information and
[the package map](../kit.md) for current paths.
