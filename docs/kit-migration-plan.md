# Kit workspace architecture

WGPUI Kit is the application-facing facade over the framework, Base behavior, styled components and bundled assets. The workspace preserves the component adapter history and imports upstream Kit v0.6.1 (`36b51819deb52c947a79f8de29e0e9175eda7464`).

Supporting crates live under `crates/`; examples live under `examples/`. The framework and derive package use version 0.3.6; the Kit family uses 0.6.1. The published derive name remains `wgpui_derive`; its directory and dependency key use `wgpui-derive`.

See [Kit documentation](kit.md), [the acceptance inventory](kit-migration-parity.json), and [release status](../STATUS.md) for the supported API and validation results. Unported upstream sources remain in `upstream-kit/` and are excluded from the active workspace and published packages.

Public project documents describe this repository's architecture, supported behavior and reproducible verification. Operator-specific migration plans and deployment context are maintained separately.
