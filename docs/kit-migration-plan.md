# Kit workspace architecture

WGPUI Kit is the application-facing facade over the framework, Base behavior, styled components and bundled assets. The current source candidate ports upstream Kit v0.8.1 (commit `99576db58ec8103417c41a092e891b4022d24e7e`) onto WGPUI's wgpu and winit backend. The preceding imported baseline was v0.6.1 (commit `36b51819deb52c947a79f8de29e0e9175eda7464`).

Supporting crates live under `crates/`; examples live under `examples/`. The framework and derive source candidate use version 0.4.0, the Kit family and FPS package use 0.8.1, and the HTTP facade uses 0.1.0. Published release evidence remains in STATUS.md. The published derive name remains `wgpui_derive`; its directory and dependency key use `wgpui-derive`.

The parity target includes the Kit API, component behavior, assets, test helpers, profiling, speech and syntax-highlighting features on the existing native backend. Alternative GPUI engines and browser or mobile integrations are outside this backend selection. Source import and compilation do not establish completed behavior or rendering acceptance.

See [Kit documentation](kit.md), [the acceptance inventory](kit-migration-parity.json), and [release status](../STATUS.md) for the supported API and validation results. Unported sources in `upstream-kit/` remain historical references, excluded from the active workspace and published packages.

Public project documents describe this repository's architecture, supported behavior and reproducible verification. Operator-specific migration plans and deployment context are maintained separately.
