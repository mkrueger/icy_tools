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
        self.terminal.viewport.read().scroll_y
    }

    fn scroll_x(&self) -> f32 {
        self.terminal.viewport.read().scroll_x
    }

    fn scroll_y_to(&mut self, y: f32) {
        self.terminal.scroll_y_to(y);
        self.terminal.sync_scrollbar_with_viewport();
    }

    fn scroll_x_to(&mut self, x: f32) {
        self.terminal.scroll_x_to(x);
        self.terminal.sync_scrollbar_with_viewport();
    }

    fn scroll_by(&mut self, dx: f32, dy: f32) {
        self.terminal.scroll_x_by(dx);
        self.terminal.scroll_y_by(dy);
        self.terminal.sync_scrollbar_with_viewport();
    }

    fn scroll_by_smooth(&mut self, dx: f32, dy: f32) {
        self.terminal.scroll_x_by_smooth(dx);
        self.terminal.scroll_y_by_smooth(dy);
        self.terminal.sync_scrollbar_with_viewport();
    }

    fn scroll_to_smooth(&mut self, x: f32, y: f32) {
        self.terminal.scroll_x_to_smooth(x);
        self.terminal.scroll_y_to_smooth(y);
        self.terminal.sync_scrollbar_with_viewport();
    }

    fn sync_scrollbar(&mut self) {
        self.terminal.sync_scrollbar_with_viewport();
    }

    fn update_animations(&mut self, _dt: f32) {
        self.terminal.update_animations();
    }
}
