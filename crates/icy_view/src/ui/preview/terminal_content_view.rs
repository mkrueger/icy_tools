//! Terminal content view wrapper implementing ContentView trait
//!
//! Wraps the Terminal from icy_engine_gui to provide the ContentView interface.

use icy_engine_gui::Terminal;

use super::content_view::ContentView;

/// Wrapper around Terminal that implements ContentView
pub struct TerminalContentView<'a> {
    terminal: &'a mut Terminal,
}

impl<'a> TerminalContentView<'a> {
    /// Create a new terminal content view wrapper
    pub fn new(terminal: &'a mut Terminal) -> Self {
        Self { terminal }
    }
}

impl ContentView for TerminalContentView<'_> {
    fn scroll_y(&self) -> f32 {
        self.terminal.scroll_y()
    }

    fn scroll_x(&self) -> f32 {
        self.terminal.scroll_x()
    }

    fn scroll_y_to(&mut self, y: f32) {
        let _ = y;
        // scroll_area owns scrolling
    }

    fn scroll_x_to(&mut self, x: f32) {
        let _ = x;
        // scroll_area owns scrolling
    }

    fn scroll_by(&mut self, dx: f32, dy: f32) {
        let _ = (dx, dy);
        // scroll_area owns scrolling
    }

    fn scroll_by_smooth(&mut self, dx: f32, dy: f32) {
        let _ = (dx, dy);
        // scroll_area owns scrolling
    }

    fn scroll_to_smooth(&mut self, x: f32, y: f32) {
        let _ = (x, y);
        // scroll_area owns scrolling
    }

    fn sync_scrollbar(&mut self) {
        // scroll_area owns scrollbars
    }

    fn update_animations(&mut self, _dt: f32) {
        // Terminal-Animationen (Caret-Blink, Smooth-Scroll, Scrollbars) sind widget-intern
        // über `RedrawRequested` + `request_redraw_at` getrieben.
    }
}
