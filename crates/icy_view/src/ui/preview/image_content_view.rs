//! Image content view wrapper implementing ContentView trait
//!
//! Wraps the ImageViewer to provide the ContentView interface.

use super::content_view::ContentView;
use super::image_viewer::ImageViewer;

/// Wrapper around ImageViewer that implements ContentView
pub struct ImageContentView<'a> {
    viewer: &'a mut ImageViewer,
}

impl<'a> ImageContentView<'a> {
    /// Create a new image content view wrapper
    pub fn new(viewer: &'a mut ImageViewer) -> Self {
        Self { viewer }
    }
}

impl ContentView for ImageContentView<'_> {
    fn scroll_y(&self) -> f32 {
        self.viewer.scroll_y()
    }

    fn scroll_x(&self) -> f32 {
        self.viewer.scroll_x()
    }

    fn scroll_y_to(&mut self, _y: f32) {
        // scroll_area handles scrolling - no-op
    }

    fn scroll_x_to(&mut self, _x: f32) {
        // scroll_area handles scrolling - no-op
    }

    fn scroll_by(&mut self, _dx: f32, _dy: f32) {
        // scroll_area handles scrolling - no-op
    }

    fn scroll_by_smooth(&mut self, _dx: f32, _dy: f32) {
        // scroll_area handles scrolling - no-op
    }

    fn scroll_to_smooth(&mut self, _x: f32, _y: f32) {
        // scroll_area handles scrolling - no-op
    }

    fn sync_scrollbar(&mut self) {
        // scroll_area handles scrollbars
    }

    fn update_animations(&mut self, _dt: f32) {
        // scroll_area handles animations
    }
}
