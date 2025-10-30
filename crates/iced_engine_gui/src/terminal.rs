use std::sync::{Arc, Mutex};

use iced::{Color, widget::canvas::Cache};
use icy_engine::editor::EditState;

pub struct Terminal {
    pub edit_state: Arc<Mutex<EditState>>,
    pub font_size: f32,
    pub char_width: f32,
    pub char_height: f32,
    pub cache: Cache,
}

impl Terminal {
    pub fn new(edit_state: Arc<Mutex<EditState>>) -> Self {
        Self {
            edit_state,
            font_size: 16.0,
            char_width: 9.6, // Approximate for monospace
            char_height: 20.0,
            cache: Cache::default(),
        }
    }

    pub fn reset_caret_blink(&mut self) {}

    pub fn redraw(&mut self) {
        self.cache.clear();
    }

    // Helper function to convert buffer color to iced Color
    pub fn buffer_color_to_iced(color: icy_engine::Color) -> Color {
        let (r, g, b) = color.get_rgb_f32();
        Color::from_rgb(r, g, b)
    }
}
