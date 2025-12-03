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
    /// Last time the scrollbar was interacted with
    pub last_interaction: Option<Instant>,
    /// Whether the scrollbar is currently being dragged
    pub is_dragging: bool,
    /// Whether the mouse is hovering over the scrollbar
    pub is_hovered: bool,
    /// Animation progress (0.0 = hidden, 1.0 = fully visible)
    pub visibility: f32,
    /// Target visibility for animation
    target_visibility: f32,
    /// Visibility value at the start of the current animation
    anim_from: f32,
    /// Start time of the current animation
    anim_start: Option<Instant>,
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
            is_dragging: false,
            is_hovered: false,
            visibility: 0.15, // Start with thin line visible
            target_visibility: 0.15,
            anim_from: 0.15,
            anim_start: None,
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

    /// Update animation state using time-based easing.
    pub fn update_animation(&mut self) {
        let Some(start_time) = self.anim_start else {
            return;
        };

        let now = Instant::now();

        // Respect fade-out delay: while within delay, keep current visibility
        if self.target_visibility < self.visibility {
            if let Some(last) = self.last_interaction {
                let since_interaction = now.duration_since(last);
                if since_interaction < self.fade_out_delay {
                    // Still in delay period, don't animate yet
                    return;
                } else {
                    // Delay just expired, restart animation from now
                    let expected_start = last + self.fade_out_delay;
                    if start_time < expected_start {
                        self.anim_start = Some(expected_start);
                        self.anim_from = self.visibility;
                        return; // Come back next frame to start animating
                    }
                }
            }
        }

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

    /// Check if scrollbar is currently animating
    pub fn is_animating(&self) -> bool {
        self.anim_start.is_some() && (self.visibility - self.target_visibility).abs() > 0.001
    }

    /// Mark that the scrollbar was interacted with
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

    /// Check if the scrollbar needs animation updates
    pub fn needs_animation(&self) -> bool {
        self.is_animating() || self.is_hovered || self.is_dragging || {
            if let Some(last) = self.last_interaction {
                // Keep animating during the fade-out delay period + animation time
                Instant::now().duration_since(last) < (self.fade_out_delay + Duration::from_millis(500))
            } else {
                false
            }
        }
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

    /// Set dragging state
    pub fn set_dragging(&mut self, dragging: bool) {
        if self.is_dragging != dragging {
            self.is_dragging = dragging;
            self.mark_interaction(dragging);
        }
    }
}
