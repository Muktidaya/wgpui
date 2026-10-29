# GPU surfaces and frame capture in WGPUI 0.4.0

Processed `RenderImage` frames and color-glyph atlas uploads use straight-alpha
RGBA bytes in `Rgba8Unorm` textures. Image decoders retain RGBA. Core Text and
SVG output is unpremultiplied; Cairo native-endian ARGB words and Direct2D BGRA
pixels are converted at their platform boundary and unpremultiplied. Native
presentation surfaces may use another supported format; texture sampling
interprets that format without changing the processed-image contract.

## Embedded rendering

Create a surface with `Window::create_wgpu_surface`, render into the handle's
back buffer, submit GPU work, then call `WgpuSurfaceHandle::present`. Presentation
posts a coalesced event to the owner window. A pending surface frame can be
composited from the retained scene without relaying out or repainting the UI.
An idle window with no pending work sleeps. Buffer resizing preserves surface
identity while replacing the texture generation. Each painted placement has
independent bounds, including repeated placements of one handle.

`App::wgpu_gpu` returns cloned handles to the framework's instance, adapter,
device and queue, or `None` on a platform without that renderer. An application
creating an independent native surface must use that instance and check the
surface's capabilities against the adapter. The shared device's lifetime remains
owned by the framework. This API does not supply a separate event loop or native
window implementation.

The device enables adapter-supported `TEXTURE_FORMAT_16BIT_NORM`,
`FLOAT32_FILTERABLE` and `TIMESTAMP_QUERY`. Inspect `device.features()` before
using an optional capability; absence is supported. Enabling timestamp queries
does not create query sets or collect timings automatically.

## Complete-scene capture

`Window::request_frame_capture` requests one presentation and returns an optional
oneshot receiver for `Result<FrameCapture, FrameCaptureError>`. The copy includes
the compositor's UI and embedded surfaces. Capture bytes have tightly packed
rows; inspect `FrameCapture::format` for RGBA versus BGRA and linear versus sRGB
storage. This native-surface readback format is separate from processed-image
RGBA input. Alpha follows the window's compositor mode.

A new pending request supersedes an earlier pending request and cancels its
receiver. Once a readback is in flight, another request returns `Busy`. A native
surface without `COPY_SRC` support continues rendering and returns
`UnsupportedUsage` for capture. No copy or readback worker runs without a request.
Readback polling is bounded; mapping, device and format failures are returned.
If a window stops composing before a pending request is served, callers should
apply a timeout. `None` means that platform has no capture implementation.

Failed native surface configuration is retried through the event loop, starting
at 16 ms and capping the interval at 250 ms. Successful configuration removes the
retry wakeup. This does not make permanent device loss recoverable.

## Validation

The portable framework tests cover handler reentry, cached input slots, pinch
dispatch, capture request ownership, capability selection and retry backoff.
Run the real renderer regressions explicitly with an available GPU:

```sh
cargo +1.95.0 test --locked -p wgpui --all-features --lib platform::renderer::offscreen_tests -- --ignored --test-threads=1
cargo +1.95.0 test --locked -p wgpui-kit --all-features --test rendering -- --test-threads=1
```

Offscreen results do not establish native multi-display recovery, trackpad input,
menu reentry or surviving-handle window closure. Those require native interaction
validation. See [release status](../STATUS.md) for acceptance boundaries.
