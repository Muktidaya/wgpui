use std::time::Duration;
use wgpui::PlatformDisplay;

/// Uses the backend's real monitor information rather than interpreting its
/// enumeration index as a native monitor handle.
pub(crate) fn display_refresh_rate(display: &dyn PlatformDisplay) -> Option<Duration> {
    display.refresh_interval()
}
