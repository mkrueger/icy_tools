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

    /// Get current horizontal scroll position
    fn scroll_x(&self) -> f32;

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
}
