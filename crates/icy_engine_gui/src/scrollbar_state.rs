//! macOS-style overlay scrollbar state tracking
//!
//! This module provides state management for overlay scrollbars (vertical and horizontal) with animation.
//! The actual rendering should be integrated into your custom widget implementation.

use std::time::{Duration, Instant};

/// State for tracking scrollbar animation and interaction
#[derive(Debug)]
pub struct ScrollbarState {
    /// Current vertical scroll position (0.0 to 1.0)
    pub scroll_position: f32,
    /// Current horizontal scroll position (0.0 to 1.0)
    pub scroll_position_x: f32,
    /// Last time the vertical scrollbar was interacted with
    pub last_interaction: Option<Instant>,
    /// Last time the horizontal scrollbar was interacted with
    pub last_interaction_x: Option<Instant>,
    /// Whether the vertical scrollbar is currently being dragged
    pub is_dragging: bool,
    /// Whether the horizontal scrollbar is currently being dragged
    pub is_dragging_x: bool,
    /// Whether the mouse is hovering over the vertical scrollbar
    pub is_hovered: bool,
    /// Whether the mouse is hovering over the horizontal scrollbar
    pub is_hovered_x: bool,
    /// Vertical scrollbar animation progress (0.0 = hidden, 1.0 = fully visible)
    pub visibility: f32,
    /// Horizontal scrollbar animation progress (0.0 = hidden, 1.0 = fully visible)
    pub visibility_x: f32,
    /// Target visibility for vertical scrollbar animation
    target_visibility: f32,
    /// Target visibility for horizontal scrollbar animation
    target_visibility_x: f32,
    /// Visibility value at the start of the current vertical animation
    anim_from: f32,
    /// Visibility value at the start of the current horizontal animation
    anim_from_x: f32,
    /// Start time of the current vertical animation
    anim_start: Option<Instant>,
    /// Start time of the current horizontal animation
    anim_start_x: Option<Instant>,
    /// Duration of the current animation
    anim_duration: Duration,
    /// Delay before starting fade out (milliseconds)
    fade_out_delay: Duration,
}

impl Default for ScrollbarState {
    fn default() -> Self {
        Self {
            scroll_position: 0.0,
            scroll_position_x: 0.0,
            last_interaction: None,
            last_interaction_x: None,
            is_dragging: false,
            is_dragging_x: false,
            is_hovered: false,
            is_hovered_x: false,
            visibility: 0.15, // Start with thin line visible
            visibility_x: 0.15,
            target_visibility: 0.15,
            target_visibility_x: 0.15,
            anim_from: 0.15,
            anim_from_x: 0.15,
            anim_start: None,
            anim_start_x: None,
            anim_duration: Duration::from_millis(180),  // default, overridden per animation
            fade_out_delay: Duration::from_millis(400), // Shorter delay (was 800ms)
        }
    }
}

impl ScrollbarState {
    pub fn new() -> Self {
        Self::default()
    }

    /// Easing function for smooth animation (ease-out cubic for natural deceleration)
    fn ease_out_cubic(t: f32) -> f32 {
        1.0 - (1.0 - t).powi(3)
    }

    /// Update vertical scrollbar animation state using time-based easing.
    pub fn update_animation(&mut self) {
        // Update vertical scrollbar
        if let Some(start_time) = self.anim_start {
            let now = Instant::now();

            // Respect fade-out delay: while within delay, keep current visibility
            if self.target_visibility < self.visibility {
                if let Some(last) = self.last_interaction {
                    let since_interaction = now.duration_since(last);
                    if since_interaction < self.fade_out_delay {
                        // Still in delay period, don't animate yet
                    } else {
                        // Delay just expired, restart animation from now
                        let expected_start = last + self.fade_out_delay;
                        if start_time < expected_start {
                            self.anim_start = Some(expected_start);
                            self.anim_from = self.visibility;
                        } else {
                            self.animate_vertical(now, start_time);
                        }
                    }
                } else {
                    self.animate_vertical(now, start_time);
                }
            } else {
                self.animate_vertical(now, start_time);
            }
        }

        // Update horizontal scrollbar
        if let Some(start_time_x) = self.anim_start_x {
            let now = Instant::now();

            // Respect fade-out delay: while within delay, keep current visibility
            if self.target_visibility_x < self.visibility_x {
                if let Some(last) = self.last_interaction_x {
                    let since_interaction = now.duration_since(last);
                    if since_interaction < self.fade_out_delay {
                        // Still in delay period, don't animate yet
                    } else {
                        // Delay just expired, restart animation from now
                        let expected_start = last + self.fade_out_delay;
                        if start_time_x < expected_start {
                            self.anim_start_x = Some(expected_start);
                            self.anim_from_x = self.visibility_x;
                        } else {
                            self.animate_horizontal(now, start_time_x);
                        }
                    }
                } else {
                    self.animate_horizontal(now, start_time_x);
                }
            } else {
                self.animate_horizontal(now, start_time_x);
            }
        }
    }

    fn animate_vertical(&mut self, now: Instant, start_time: Instant) {
        let elapsed = now.saturating_duration_since(start_time);
        let duration = self.anim_duration.max(Duration::from_millis(1));

        let t = (elapsed.as_secs_f32() / duration.as_secs_f32()).clamp(0.0, 1.0);
        let eased_t = Self::ease_out_cubic(t);

        let from = self.anim_from;
        let to = self.target_visibility;
        self.visibility = (from + (to - from) * eased_t).clamp(0.0, 1.0);
        if t >= 1.0 {
            self.visibility = self.target_visibility;
            self.anim_start = None;
        }
    }

    fn animate_horizontal(&mut self, now: Instant, start_time: Instant) {
        let elapsed = now.saturating_duration_since(start_time);
        let duration = self.anim_duration.max(Duration::from_millis(1));

        let t = (elapsed.as_secs_f32() / duration.as_secs_f32()).clamp(0.0, 1.0);
        let eased_t = Self::ease_out_cubic(t);

        let from = self.anim_from_x;
        let to = self.target_visibility_x;
        self.visibility_x = (from + (to - from) * eased_t).clamp(0.0, 1.0);
        if t >= 1.0 {
            self.visibility_x = self.target_visibility_x;
            self.anim_start_x = None;
        }
    }

    /// Check if vertical scrollbar is currently animating
    pub fn is_animating(&self) -> bool {
        (self.anim_start.is_some() && (self.visibility - self.target_visibility).abs() > 0.001)
            || (self.anim_start_x.is_some() && (self.visibility_x - self.target_visibility_x).abs() > 0.001)
    }

    /// Mark that the vertical scrollbar was interacted with
    pub fn mark_interaction(&mut self, fade_in: bool) {
        let now = Instant::now();
        self.last_interaction = Some(now);

        // Set start of animation from current visibility
        self.anim_from = self.visibility;

        // Different target values for different states
        self.target_visibility = if fade_in { 1.0 } else { 0.0 };

        // Configure animation duration: faster fade-in, slightly slower fade-out
        self.anim_duration = if self.target_visibility > self.anim_from {
            Duration::from_millis(140)
        } else {
            Duration::from_millis(220)
        };

        self.anim_start = Some(now);
    }

    /// Mark that the horizontal scrollbar was interacted with
    pub fn mark_interaction_x(&mut self, fade_in: bool) {
        let now = Instant::now();
        self.last_interaction_x = Some(now);

        // Set start of animation from current visibility
        self.anim_from_x = self.visibility_x;

        // Different target values for different states
        self.target_visibility_x = if fade_in { 1.0 } else { 0.0 };

        // Configure animation duration: faster fade-in, slightly slower fade-out
        self.anim_duration = if self.target_visibility_x > self.anim_from_x {
            Duration::from_millis(140)
        } else {
            Duration::from_millis(220)
        };

        self.anim_start_x = Some(now);
    }

    /// Check if the scrollbar needs animation updates
    pub fn needs_animation(&self) -> bool {
        // Overlay widgets schedule their own `request_redraw_at(...)` precisely via
        // `next_wakeup_instant`, so we only report true while an animation is
        // actively running or while dragging.
        self.is_animating() || self.is_dragging || self.is_dragging_x
    }

    /// Returns the next instant at which the scrollbar needs a redraw to advance
    /// its own animation (fade-in/out delays or easing).
    ///
    /// Intended for overlay widgets to schedule `request_redraw_at(...)` precisely,
    /// instead of relying on a global animation tick.
    pub fn next_wakeup_instant(&self, now: Instant) -> Option<Instant> {
        let frame = now + Duration::from_millis(16);
        let mut next: Option<Instant> = None;

        // If the user is dragging, we want frequent redraws for responsiveness.
        if self.is_dragging || self.is_dragging_x {
            next = Some(frame);
        }

        // Vertical
        if self.anim_start.is_some() {
            // During fade-out delay we don't need 60fps; schedule exactly when the delay expires.
            if self.target_visibility < self.visibility {
                if let Some(last) = self.last_interaction {
                    let expected_start = last + self.fade_out_delay;
                    let t = if now < expected_start { expected_start } else { frame };
                    next = Some(next.map_or(t, |cur| cur.min(t)));
                } else {
                    next = Some(next.map_or(frame, |cur| cur.min(frame)));
                }
            } else {
                next = Some(next.map_or(frame, |cur| cur.min(frame)));
            }
        }

        // Horizontal
        if self.anim_start_x.is_some() {
            if self.target_visibility_x < self.visibility_x {
                if let Some(last) = self.last_interaction_x {
                    let expected_start = last + self.fade_out_delay;
                    let t = if now < expected_start { expected_start } else { frame };
                    next = Some(next.map_or(t, |cur| cur.min(t)));
                } else {
                    next = Some(next.map_or(frame, |cur| cur.min(frame)));
                }
            } else {
                next = Some(next.map_or(frame, |cur| cur.min(frame)));
            }
        }

        next
    }

    /// Set scroll position from external source (e.g., viewport)
    pub fn set_scroll_position(&mut self, position: f32) {
        let new_pos = position.clamp(0.0, 1.0);
        if (self.scroll_position - new_pos).abs() > 0.001 {
            self.scroll_position = new_pos;
        }
    }

    /// Set horizontal scroll position from external source (e.g., viewport)
    pub fn set_scroll_position_x(&mut self, position: f32) {
        let new_pos = position.clamp(0.0, 1.0);
        if (self.scroll_position_x - new_pos).abs() > 0.001 {
            self.scroll_position_x = new_pos;
        }
    }

    /// Update hover state (should be called on mouse move)
    pub fn set_hovered(&mut self, hovered: bool) {
        if self.is_hovered != hovered {
            self.is_hovered = hovered;
            self.mark_interaction(hovered || self.is_dragging);
        }
    }

    /// Update horizontal hover state (should be called on mouse move)
    pub fn set_hovered_x(&mut self, hovered: bool) {
        if self.is_hovered_x != hovered {
            self.is_hovered_x = hovered;
            self.mark_interaction_x(hovered || self.is_dragging_x);
        }
    }

    /// Set dragging state
    pub fn set_dragging(&mut self, dragging: bool) {
        if self.is_dragging != dragging {
            self.is_dragging = dragging;
            self.mark_interaction(dragging);
        }
    }

    /// Set horizontal dragging state
    pub fn set_dragging_x(&mut self, dragging: bool) {
        if self.is_dragging_x != dragging {
            self.is_dragging_x = dragging;
            self.mark_interaction_x(dragging);
        }
    }
}
