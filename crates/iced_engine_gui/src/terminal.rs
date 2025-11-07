use std::sync::{Arc, Mutex};

use iced::{Color, widget};
use icy_engine::editor::EditState;

pub struct Terminal {
    pub edit_state: Arc<Mutex<EditState>>,
    pub font_size: f32,
    pub char_width: f32,
    pub char_height: f32,
    pub id: widget::Id,
    pub has_focus: bool,
    pub picture_data: Option<(icy_engine::Size, Vec<u8>)>,
}

impl Terminal {
    pub fn new(edit_state: Arc<Mutex<EditState>>) -> Self {
        Self {
            edit_state,
            font_size: 16.0,
            char_width: 9.6, // Approximate for monospace
            char_height: 20.0,
            id: widget::Id::unique(),
            has_focus: false,
            picture_data: None,
        }
    }

    pub fn reset_caret_blink(&mut self) {}

    // Helper function to convert buffer color to iced Color
    pub fn buffer_color_to_iced(color: icy_engine::Color) -> Color {
        let (r, g, b) = color.get_rgb_f32();
        Color::from_rgb(r, g, b)
    }

    pub fn update_picture(&mut self, size: icy_engine::Size, data: Vec<u8>) {
        self.picture_data = Some((size, data));
    }
}
