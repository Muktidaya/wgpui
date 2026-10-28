# wgpui

`wgpui` is a hybrid-immediate/retained and GPU-accelerated UI framework for WGPU. It started as a community fork of [GPUI](https://gpui.rs) / [GPUI-CE 0.3.3](https://crates.io/crates/gpui-ce/0.3.3) and keeps a GPUI-shaped programming model (`App`, `Entity`, `Window`, flexbox `div`s, actions) and renders through a **unified wgpu + winit** backend. However, it is **not** a drop-in for GPUI-CE git main or Zed GPUI: those trees have split crates, AccessKit, and different platform APIs. WGPUI versions on its own line.

The discriminating API is [`WgpuSurface`](src/elements/wgpu_surface.rs): a double-buffered wgpu texture that lives in the UI tree. A CAD or 3D shell can render into `WgpuSurfaceHandle::back_buffer_view()`, call `present()`, and let WGPUI composite the result with quads, text, and paths. Surface resizes preserve the logical handle while versioning replacement texture views, so compositor bind groups remain valid through window and full-screen transitions.

Text services use a platform interface. Linux builds require Pango 1.50 or later, Cairo 1.18 or later, HarfBuzz and Fontconfig development libraries. The DirectWrite adapter requires Windows 10 build 15063 or later. Desktop text uses Core Text on macOS, DirectWrite on Windows, and Pango on Linux; [release status](STATUS.md) records acceptance. Layout uses [taffy](https://github.com/DioxusLabs/taffy). UI and monospace text default to the operating system's font families. Applications can override either family; framework assets contain no bundled text fonts.

Programming-model notes: [docs/contexts.md](docs/contexts.md), [docs/key_dispatch.md](docs/key_dispatch.md).

This consolidated workspace also contains **[WGPUI Kit](docs/kit.md)**, an application façade and component library adapted from [GPUI Kit 0.8.1](https://github.com/longbridge/gpui-kit). See [STATUS.md](STATUS.md) for validation and release state.

## Usage

```toml
[dependencies]
wgpui = { version = "0.3.6" }
```

See `examples/learn/wgpu_surface.rs` for embedding a wgpu render target, `examples/learn/paths.rs` for GPU paths, `examples/learn/custom_drawing.rs` for canvas drawing, and `examples/legacy/hello_world.rs` for a window.

The source candidate is WGPUI 0.4.0 with Kit 0.8.1 and requires Rust 1.95. See [source compatibility](docs/kit.md#source-candidate-compatibility) for the stable Ropey, HTTP, constructor and grammar API changes. Published release versions remain listed above and in [release status](STATUS.md).

## License

Apache-2.0. Original GPUI is copyright Nathan Sobo and Zed Industries contributors. This tree is maintained independently.
