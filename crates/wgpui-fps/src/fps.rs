//! A realtime performance HUD for WGPUI applications: frames per second, a
//! rolling frame time chart, and this process' GPU, CPU and memory usage.
//!
//! Frame data comes from WGPUI's frame trace
//! ([`gpui::FrameTimingCollector`]). The observed rate uses each frame's
//! presentation-submission timestamp; it does not measure physical scanout or
//! GPU completion. The frame cost measures the framework's CPU draw work. The
//! headline estimates redraw capacity from that cost and the reported display
//! refresh interval, without driving the application's frame loop.
//!
//! Render it wherever it should appear, guarded by your own flag:
//!
//! ```no_run
//! # use gpui::{prelude::*, *};
//! # use wgpui_fps::fps_monitor;
//! # struct Example { show_fps: bool }
//! # impl Render for Example {
//! fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
//!     div()
//!         .relative()
//!         .size_full()
//!         .child("your app")
//!         .when(self.show_fps, |this| this.child(fps_monitor(window, cx)))
//! }
//! # }
//! ```
//!
//! The returned overlay can change its corner and its frame budget. A custom palette or an
//! embedded rather than overlaid HUD is built by composing [`FpsMonitor`] and
//! [`FpsOverlay`] directly.
//!
//! The UI uses WGPUI directly; native resource probes read operating-system
//! process counters.

#[cfg(not(target_family = "wasm"))]
mod gpu;
#[cfg(not(target_family = "wasm"))]
mod memory;
mod monitor;
mod overlay;
mod refresh;
mod sampler;
mod style;

pub use monitor::FpsMonitor;
pub use overlay::FpsOverlay;

use std::collections::HashMap;

use gpui::{App, AppContext as _, Entity, Global, Window, WindowId};

/// The performance HUD, pinned to the top right of its parent.
///
/// The parent element must be `relative()`, since the HUD positions itself
/// absolutely. Call this at most once per window — a second call renders the
/// same monitor twice.
///
/// Whether the HUD is on screen is the caller's to decide; render this only
/// when it should be visible:
///
/// ```no_run
/// # use gpui::{prelude::*, *};
/// # use wgpui_fps::fps_monitor;
/// # struct Example { show_fps: bool }
/// # impl Render for Example {
/// fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
///     div()
///         .relative()
///         .size_full()
///         .child("your app")
///         .when(self.show_fps, |this| this.child(fps_monitor(window, cx)))
/// }
/// # }
/// ```
///
/// The monitor behind it is created on first use and reused afterwards, one per
/// window, so this can be called straight from `render` every frame.
pub fn fps_monitor(window: &mut Window, cx: &mut App) -> FpsOverlay {
    let window_id = window.window_handle().window_id();
    let existing = cx
        .try_global::<Monitors>()
        .and_then(|state| state.0.get(&window_id).cloned());
    let monitor = match existing {
        Some(monitor) => monitor,
        None => {
            let monitor = cx.new(|cx| FpsMonitor::new(window, cx));
            cx.default_global::<Monitors>()
                .0
                .insert(window_id, monitor.clone());
            monitor
        }
    };

    FpsOverlay::new(&monitor)
}

/// The monitor [`fps_monitor`] reuses for each window.
///
/// Entries outlive their window; the leak is one small entity per window that
/// ever showed the HUD, which is not worth tracking window closes for.
#[derive(Default)]
struct Monitors(HashMap<WindowId, Entity<FpsMonitor>>);

impl Global for Monitors {}

/// Keeps an independent profiler trace scope alive for a monitor. The host's
/// explicit trace setting is preserved even when it changes while the HUD is
/// visible.
pub(crate) struct FrameTraceGuard {
    _scope: gpui::profiler::TraceGuard,
}

impl FrameTraceGuard {
    pub(crate) fn acquire() -> Self {
        Self {
            _scope: gpui::profiler::trace_scope(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn dropping_an_inner_guard_keeps_tracing_on_for_the_outer_guard() {
        let outer = FrameTraceGuard::acquire();
        let inner = FrameTraceGuard::acquire();
        assert!(gpui::profiler::trace_enabled());

        drop(inner);
        assert!(
            gpui::profiler::trace_enabled(),
            "the outer guard still needs the trace"
        );

        drop(outer);
    }
    #[test]
    fn host_can_enable_tracing_while_a_monitor_scope_is_active() {
        let monitor = FrameTraceGuard::acquire();
        let changed = gpui::profiler::set_trace_enabled(true);
        drop(monitor);
        assert!(gpui::profiler::trace_enabled());
        if changed {
            gpui::profiler::set_trace_enabled(false);
        }
    }
}
