//! Scroll gesture helpers used by component libraries.

use std::time::Duration;

use crate::{Axis, Instant, IsZero, Pixels, Point, TouchPhase, px};

const SCROLL_EVENT_SEPARATION: Duration = Duration::from_millis(28);

/// Tracks the dominant axis across the events in a scroll gesture.
#[derive(Clone, Copy, Debug, Default)]
pub struct OngoingScroll {
    last_event: Option<Instant>,
    axis: Option<Axis>,
}

impl OngoingScroll {
    /// Filters the given delta to the dominant axis of the current scroll gesture.
    pub fn filter(&mut self, delta: &mut Point<Pixels>, touch_phase: TouchPhase) {
        self.filter_at(delta, touch_phase, Instant::now())
    }

    fn filter_at(&mut self, delta: &mut Point<Pixels>, touch_phase: TouchPhase, now: Instant) {
        const UNLOCK_PERCENT: f32 = 1.9;
        const UNLOCK_LOWER_BOUND: Pixels = px(6.);

        if matches!(touch_phase, TouchPhase::Ended | TouchPhase::Cancelled) {
            self.last_event = None;
            self.axis = None;
            return;
        }

        let x = delta.x.abs();
        let y = delta.y.abs();
        if x.is_zero() && y.is_zero() {
            if touch_phase == TouchPhase::Started {
                self.last_event = None;
                self.axis = None;
            }
            return;
        }

        let starts_new_gesture = touch_phase == TouchPhase::Started
            || self
                .last_event
                .is_none_or(|last_event| now.duration_since(last_event) >= SCROLL_EVENT_SEPARATION);
        let mut axis = self.axis;
        if starts_new_gesture {
            axis = if x <= y {
                Some(Axis::Vertical)
            } else {
                Some(Axis::Horizontal)
            };
        } else if x.max(y) >= UNLOCK_LOWER_BOUND {
            match axis {
                Some(Axis::Vertical) if x > y && x >= y * UNLOCK_PERCENT => {
                    axis = None;
                }
                Some(Axis::Horizontal) if y > x && y >= x * UNLOCK_PERCENT => {
                    axis = None;
                }
                _ => {}
            }
        }

        self.last_event = Some(now);
        self.axis = axis;
        match axis {
            Some(Axis::Vertical) => delta.x = Pixels::ZERO,
            Some(Axis::Horizontal) => delta.y = Pixels::ZERO,
            None => {}
        }
    }
}
