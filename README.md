# wgpui

WGPUI is an independent GPU UI framework for Rust. It keeps a GPUI-shaped programming model (`App`, `Entity`, `Window`, flexbox `div`s, actions) and renders through a **unified wgpu + winit** backend.

It started as a community fork of [GPUI](https://gpui.rs) / [GPUI-CE 0.3.3](https://crates.io/crates/gpui-ce/0.3.3). It is **not** a drop-in for GPUI-CE git main or Zed GPUI: those trees have split crates, AccessKit, and different platform APIs. WGPUI versions on its own line.

The discriminating API is [`WgpuSurface`](src/elements/wgpu_surface.rs): a double-buffered wgpu texture that lives in the UI tree. A CAD or 3D shell can render into `WgpuSurfaceHandle::back_buffer_view()`, call `present()`, and let WGPUI composite the result with quads, text, and paths.

Text is shaped with [cosmic-text](https://github.com/pop-os/cosmic-text). Layout uses [taffy](https://github.com/DioxusLabs/taffy).

Programming-model notes: [docs/contexts.md](docs/contexts.md), [docs/key_dispatch.md](docs/key_dispatch.md).

## Usage

```toml
[dependencies]
wgpui = { version = "0.3.4" }
```

See `examples/learn/wgpu_surface.rs` for embedding a wgpu render target, `examples/learn/paths.rs` for GPU paths, `examples/learn/custom_drawing.rs` for canvas drawing, and `examples/legacy/hello_world.rs` for a window.

## License

Apache-2.0. Original GPUI is copyright Nathan Sobo and Zed Industries contributors. This tree is maintained independently.
