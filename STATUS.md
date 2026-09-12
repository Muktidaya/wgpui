# Release status

WGPUI and wgpui_derive 0.3.6, and WGPUI Kit/Base/Component/assets/macros/platform 0.6.1 are published on crates.io. Source release tags are linked below. The repository history has been sanitized; release source code is unchanged.

- [WGPUI 0.3.6](https://github.com/Muktidaya/wgpui/releases/tag/v0.3.6)
- [WGPUI Kit 0.6.1](https://github.com/Muktidaya/wgpui/releases/tag/kit-v0.6.1)
- [Supported API and backend boundaries](docs/kit.md)
- [Feature acceptance inventory](docs/kit-migration-parity.json)

Validation includes Linux/macOS/Windows CI, 1,624 all-feature workspace tests, strict Clippy, packaged-source verification, and registry-only consumers using normal, renamed and Base-only Kit dependencies. The native example was rendered and exercised on macOS.

The backend is native wgpu + winit. WASM, webview/shell integration, upstream WindowProfiler, Metal-specific test infrastructure and full native accessibility are outside the implemented surface.
