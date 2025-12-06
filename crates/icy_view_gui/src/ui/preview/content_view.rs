//! ContentView trait for unified scroll API across image and terminal views
//!
//! This module provides a common interface for scrollable content views,
//! enabling auto-scroll and shuffle mode to work consistently with both
//! terminal (ANSI) and image content.

/// Trait for content views that support scrolling
/// Both ImageViewer and Terminal implement this through wrappers
pub trait ContentView {
    /// Get current vertical scroll position
    fn scroll_y(&self) -> f32;

    /// Get maximum vertical scroll position
    fn max_scroll_y(&self) -> f32;

    /// Get current horizontal scroll position
    fn scroll_x(&self) -> f32;

    /// Get maximum horizontal scroll position
    fn max_scroll_x(&self) -> f32;

    /// Scroll to absolute Y position (immediate, no animation)
    fn scroll_y_to(&mut self, y: f32);

    /// Scroll to absolute X position (immediate, no animation)
    fn scroll_x_to(&mut self, x: f32);

    /// Scroll to absolute position (immediate, no animation)
    fn scroll_to(&mut self, x: f32, y: f32) {
        self.scroll_x_to(x);
        self.scroll_y_to(y);
    }

    /// Scroll by delta (immediate, no animation)
    fn scroll_by(&mut self, dx: f32, dy: f32);

    /// Scroll by delta with smooth animation
    fn scroll_by_smooth(&mut self, dx: f32, dy: f32);

    /// Scroll to absolute position with smooth animation
    fn scroll_to_smooth(&mut self, x: f32, y: f32);

    /// Synchronize scrollbar state with viewport
    fn sync_scrollbar(&mut self);

    /// Update animations (viewport smooth scroll + scrollbar fade)
    fn update_animations(&mut self, dt: f32);

    /// Check if animation updates are needed
    fn needs_animation(&self) -> bool;

    /// Check if scroll has reached the bottom
    fn is_at_bottom(&self) -> bool {
        let max_y = self.max_scroll_y();
        let current_y = self.scroll_y();
        max_y <= 0.0 || current_y >= max_y - 1.0
    }

    /// Scroll to top
    fn scroll_home(&mut self) {
        self.scroll_to_smooth(0.0, 0.0);
    }

    /// Scroll to bottom
    fn scroll_end(&mut self) {
        let max_x = self.max_scroll_x();
        let max_y = self.max_scroll_y();
        self.scroll_to_smooth(max_x, max_y);
    }

    /// Get visible height
    fn visible_height(&self) -> f32;

    /// Get visible width
    fn visible_width(&self) -> f32;
}
