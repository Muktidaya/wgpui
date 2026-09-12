# wgpui-platform

Native application entry for WGPUI Kit. `application()` returns
`wgpui::Application::new()`, using WGPUI's wgpu/winit backend. The optional
`test-support` feature forwards WGPUI's test support. No separate native
backend or browser platform is implemented by this shim.
