# Release status

WGPUI and wgpui_derive 0.3.6, and WGPUI Kit/Base/Component/assets/macros/platform 0.6.1 are published on crates.io. Source release tags are linked below. The repository history has been sanitized; release source code is unchanged.

- [WGPUI 0.3.6](https://github.com/Muktidaya/wgpui/releases/tag/v0.3.6)
- [WGPUI Kit 0.6.1](https://github.com/Muktidaya/wgpui/releases/tag/kit-v0.6.1)
- [Supported API and backend boundaries](docs/kit.md)
- [Feature acceptance inventory](docs/kit-migration-parity.json)

Validation includes Linux/macOS/Windows CI, 1,624 all-feature workspace tests, strict Clippy, packaged-source verification, and registry-only consumers using normal, renamed and Base-only Kit dependencies. The native example was rendered and exercised on macOS.

The published 0.6.1 line uses native wgpu + winit. Its documented boundaries excluded WASM, webview/shell integration, upstream WindowProfiler, Metal-specific test infrastructure and full native accessibility.


## Historical native menu window routing validation

Native menu validation and dispatch can occur outside the winit event callback.
`App::active_window` now falls back to the OS focus state already retained by its
windows when the platform lookup has no active context. A regression covers this
fallback and clears the result after a focus-loss notification. At that checkpoint, all 102 framework
unit tests passed with Rust 1.94. Native macOS consumer checks confirmed menu command
availability, a native path picker, focused text editing and save dispatch. These checks predate the current dependency candidate and do not validate it.

Native activations now enqueue an owned action in `CrossEvent::MenuAction` before
executing the application callback. This supplies the event-loop context needed
by window lifecycle operations and preserves the chosen action if menus are
rebuilt before dispatch. Consumer verification at that checkpoint confirmed Close Window
terminates the process and the application reopens. The 102 framework tests were
rerun successfully after this correction.


## Dependency and Kit 0.8.1 source candidate

The current source candidate uses framework/derive 0.4.0, Kit-family 0.8.1 and the stable HTTP facade 0.1.0 with a common Rust 1.95 minimum. It ports Kit behavior onto WGPUI's existing wgpu and winit backend.

The October 28 dependency metadata audit covers all 13 workspace packages and 166 active direct registry package names. Of these, 164 select their reviewed latest stable release. The latest `windows` SDK crate requires `windows-core` 0.62.2 and `windows-numerics` 0.3.1; their separately published 0.100.0 releases have incompatible Rust types and remain explicit SDK compatibility exceptions. The resolved graph contains 869 registry instances, including 166 below their absolute latest release under recorded owning constraints. No selected package is a prerelease or yanked. Registry metadata and the lockfile agree; metadata does not establish build or native behavior acceptance.

Kit 0.8.1 integration is under validation. Its source includes the new components, Base root/plugin lifecycle, touch and input behavior, frame profiler, performance HUD and upstream regression suite. The acceptance inventory records the migration state; the prior release's test results above do not validate this candidate. No new release is published by these local changes.

The text-service migration replaces Cosmic Text with Core Text on macOS, DirectWrite on Windows and Pango on Linux. The Core Text adapter has passed framework compilation, six native conformance tests and fifteen Kit offscreen rendering cases at its recorded source checkpoint. The Pango adapter has passed framework compilation, six native conformance tests, an isolated X11 clipboard case and fifteen Kit offscreen rendering cases with Cairo 1.18.6. The DirectWrite adapter is integrated in source; its adapter and eight native conformance cases compile against the Windows SDK APIs with Rust 1.95 in an isolated contract harness. The paragraph-direction isolate algorithm also passes two portable regression checks. These checks do not establish native Windows execution. Full framework and native Windows validation remain pending. Cosmic Text has been removed from the active source and dependency declarations. UI and monospace defaults resolve through OS font settings. Bundled text-font files have been removed. These source changes require new native text and rendering acceptance before release.

At the native-font checkpoints, Base passed 1,512 library and two integration tests on macOS and 1,513 library and two integration tests on Linux. Component passed 780 library and 50 integration tests on macOS, and 772 library and 50 integration tests on Linux. Its separate Syntect-only configuration passed 733 tests on macOS and 728 on Linux. Focused Core Text acceptance covers OS font identity, Unicode and fallback, raster weight/scale/phase/color, concurrent access, four-cell editor tabs and application font registration. The corrected profiler and renderer passed 206 framework CPU tests on each of macOS and Linux, the separate macOS GPU case and fifteen Kit pixel cases, including pure RTL and mixed-direction logical foreground ranges. The complete Linux workspace compiles with all features and all targets. Four declared Nix systems evaluate; full Nix build and remaining native acceptance are separate gates.
