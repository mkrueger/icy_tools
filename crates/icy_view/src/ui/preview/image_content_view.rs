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
        self.viewer.viewport.scroll_y
    }

    fn max_scroll_y(&self) -> f32 {
        self.viewer.viewport.max_scroll_y()
    }

    fn scroll_x(&self) -> f32 {
        self.viewer.viewport.scroll_x
    }

    fn max_scroll_x(&self) -> f32 {
        self.viewer.viewport.max_scroll_x()
    }

    fn scroll_y_to(&mut self, y: f32) {
        self.viewer.scroll_y_to(y);
    }

    fn scroll_x_to(&mut self, x: f32) {
        self.viewer.scroll_x_to(x);
    }

    fn scroll_by(&mut self, dx: f32, dy: f32) {
        self.viewer.scroll(dx, dy);
    }

    fn scroll_by_smooth(&mut self, dx: f32, dy: f32) {
        self.viewer.scroll_smooth(dx, dy);
    }

    fn scroll_to_smooth(&mut self, x: f32, y: f32) {
        self.viewer.scroll_to_smooth(x, y);
    }

    fn sync_scrollbar(&mut self) {
        // ImageViewer syncs scrollbar internally in scroll methods
        // This is called after each scroll operation for consistency
    }

    fn update_animations(&mut self, dt: f32) {
        self.viewer.update_scrollbars(dt);
    }

    fn needs_animation(&self) -> bool {
        self.viewer.needs_animation()
    }

    fn visible_height(&self) -> f32 {
        self.viewer.viewport.visible_height
    }

    fn visible_width(&self) -> f32 {
        self.viewer.viewport.visible_width
    }
}
