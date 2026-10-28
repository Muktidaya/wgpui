# Native acceptance harness

This unpublished workspace package contains deterministic integration tests and a native acceptance application.

Run the native application with:

    cargo +1.95.0 run -p wgpui-kit-harness --bin wgpui-native-acceptance

It exercises actual WGPUI windows and the selected native backend. The controls cover pointer actions, keyboard editing, menu/key actions, clipboard round trips, file/folder/multiple/save path selection, cancellation, secondary windows and window closing. The application emits NATIVE records for the operations it observes, including entered text and selected paths. Picker operations also emit a foreground responsiveness record while pending.

For repeatable validation, use disposable text and file fixtures. On Linux run the same application under both X11 and Wayland, with a working desktop portal. A passing layout-only or offscreen-rendering test does not replace these native service checks.

The component test helpers and the real offscreen rendering acceptance suite live in wgpui-kit. The source candidate's overall acceptance state is recorded in docs/kit-migration-parity.json.

The native harness uses `extern crate wgpui_kit as wgpui` because its package also declares the framework as `wgpui` for development targets. This gives generated action and derive paths a crate-root name in the normal binary. The consumer gate in `scripts/check-kit-consumers.py` separately covers ordinary and renamed sole Kit/framework dependencies for `actions!`, `Action`, `IntoElement`, `ctor` and the test macro, plus `IntoPlot` with Kit components enabled. Native service acceptance records the operations exercised in the harness configuration.
