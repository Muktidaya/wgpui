# Retained upstream examples

This directory preserves GPUI Base example sources for API and visual reference.
The native and WebAssembly packages are excluded from WGPUI's workspace and the
published Base package. Their GPUI platform setup requires a separate port to run
on WGPUI.

Run the current native Kit example from the repository root:

```sh
cargo run --locked -p wgpui-kit-hello-world
```

The retained showcase shares component presentations through `showcase/mod.rs`.
The native and WebAssembly entry points select from that catalog. The motion
examples demonstrate transitions, springs, keyframes, presence and stagger.
The dock showcase illustrates how a presentation layer draws a workspace around
Base's layout and interaction model.

Browser and WebAssembly integration remain outside the current desktop backend
selection. See [the Kit guide](../../../docs/kit.md) for supported entry points.
