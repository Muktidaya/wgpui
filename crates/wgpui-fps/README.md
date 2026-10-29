# WGPUI FPS

A performance HUD adapted from GPUI Kit v0.8.1 for WGPUI applications. It displays frame rate, draw duration, a frame-time chart, coalesced invalidations, and the current process's CPU, GPU and memory measurements.

Frame measurements come from WGPUI's draw and presentation instrumentation. The HUD uses each frame's own timestamp, filters events by window, and excludes frames caused solely by its readout clock. MAX FPS estimates redraw capacity from measured draw cost and the monitor's reported refresh interval. The FPS mode counts frames submitted for presentation; neither value establishes physical scanout timing.

This source candidate is version 0.8.1 and requires Rust 1.95 or newer. It accompanies WGPUI 0.4.0 and Kit 0.8.1; use the package from this workspace until it is published.

Usage:
    use wgpui_kit::*;
    use wgpui_fps::fps_monitor;

Render fps_monitor(window, cx) once per window when it should be visible. The parent element must have relative positioning. The monitor is reused for that window. Right-click changes the headline mode; click collapses the HUD. Applications own the visibility flag. FpsMonitor and FpsOverlay are also available for custom composition.

The package enables WGPUI's optional profiler feature and keeps trace collection active while a monitor uses it. Monitor refresh information comes from the WGPUI platform. Unsupported process counters remain unavailable instead of being inferred from unrelated activity. Native resource queries use the operating system's own per-process accounting.

The HUD runs on WGPUI's wgpu and winit backend. Its framework types must resolve to the same WGPUI version as the application.
