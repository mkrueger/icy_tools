use std::sync::{Arc, Mutex};

use iced::{Color, widget};
use icy_engine::Screen;

use crate::Viewport;

pub struct Terminal {
    pub screen: Arc<Mutex<Box<dyn Screen>>>,
    pub original_screen: Option<Arc<Mutex<Box<dyn Screen>>>>,
    pub viewport: Viewport,
    pub font_size: f32,
    pub char_width: f32,
    pub char_height: f32,
    pub id: widget::Id,
    pub has_focus: bool,
}

impl Terminal {
    pub fn new(screen: Arc<Mutex<Box<dyn Screen>>>) -> Self {
        // Initialize viewport with screen size
        let viewport = if let Ok(scr) = screen.lock() {
            let virtual_size = scr.virtual_size();
            let resolution = scr.get_resolution();
            Viewport::new(resolution, virtual_size)
        } else {
            Viewport::default()
        };

        Self {
            screen,
            original_screen: None,
            viewport,
            font_size: 16.0,
            char_width: 9.6, // Approximate for monospace
            char_height: 20.0,
            id: widget::Id::unique(),
            has_focus: false,
        }
    }

    /// Update viewport when screen size changes
    pub fn update_viewport_size(&mut self) {
        if let Ok(scr) = self.screen.lock() {
            let virtual_size = scr.virtual_size();
            // Only update content size, not visible size (which is the widget size, not screen size)
            self.viewport.set_content_size(virtual_size.width as f32, virtual_size.height as f32);
        }
    }

    pub fn is_in_scrollback_mode(&self) -> bool {
        self.original_screen.is_some()
    }

    pub fn enter_scrollback_mode(&mut self, scrollback: Arc<Mutex<Box<dyn Screen>>>) {
        if self.original_screen.is_none() {
            // Save the original screen
            self.original_screen = Some(self.screen.clone());
            // Switch to scrollback
            self.screen = scrollback;
            // Update viewport for scrollback content
            self.update_viewport_size();

            // Get the resolution to use as visible size for scrolling calculations
            if let Ok(scr) = self.screen.lock() {
                let resolution = scr.get_resolution();
                // Use resolution as visible size and scroll to bottom immediately (no animation)
                let max_scroll_y = (self.viewport.content_height * self.viewport.zoom - resolution.height as f32).max(0.0);
                self.viewport.scroll_to_immediate(0.0, max_scroll_y);
                // Clamp with the correct visible size
                self.viewport.clamp_scroll_with_size(resolution.width as f32, resolution.height as f32);
            }
        }
    }

    pub fn exit_scrollback_mode(&mut self) {
        if let Some(original) = self.original_screen.take() {
            self.screen = original;
            // Update viewport back to normal content
            self.update_viewport_size();
        }
    }

    pub fn reset_caret_blink(&mut self) {}

    // Helper function to convert buffer color to iced Color
    pub fn buffer_color_to_iced(color: icy_engine::Color) -> Color {
        let (r, g, b) = color.get_rgb_f32();
        Color::from_rgb(r, g, b)
    }
}
