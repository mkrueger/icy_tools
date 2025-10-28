use std::sync::{Arc, Mutex};

use iced::{Color, Point, Rectangle, Size, widget::canvas::Cache};
use icy_engine::editor::EditState;
use icy_engine::{Position, TextPane};

use crate::Message;

pub struct Terminal {
    pub edit_state: Arc<Mutex<EditState>>,
    pub font_size: f32,
    pub char_width: f32,
    pub char_height: f32,
    pub cache: Cache,
    last_buffer_hash: u64,
}

impl Terminal {
    pub fn new(edit_state: Arc<Mutex<EditState>>) -> Self {
        Self {
            edit_state,
            font_size: 16.0,
            char_width: 9.6, // Approximate for monospace
            char_height: 20.0,
            cache: Cache::default(),
            last_buffer_hash: 0,
        }
    }

    pub fn update(&mut self, message: Message) {
        match message {
            Message::SetCaret(pos) => {
                if let Ok(mut state) = self.edit_state.lock() {
                    state.get_caret_mut().set_position(pos);
                }
                self.redraw();
            }
            Message::BufferChanged => {
                self.redraw();
            }
            Message::Resize(width, height) => {
                if let Ok(mut state) = self.edit_state.lock() {
                    state.get_buffer_mut().set_size((width, height));
                }
                self.redraw();
            }
        }
    }

    pub fn redraw(&mut self) {
        self.cache.clear();
    }

    pub fn calculate_buffer_hash(&self) -> u64 {
        // Simple hash based on buffer content and caret position
        // In production, you might want a more sophisticated hash
        if let Ok(state) = self.edit_state.lock() {
            let buffer = state.get_buffer();
            let mut hash = 0u64;

            // Hash first few visible lines for quick change detection
            for y in 0..buffer.get_height().min(50) {
                for x in 0..buffer.get_width().min(80) {
                    let ch_attr = buffer.get_char(Position::new(x, y));
                    hash = hash.wrapping_mul(31).wrapping_add(ch_attr.ch as u64);
                    hash = hash
                        .wrapping_mul(31)
                        .wrapping_add(ch_attr.attribute.as_u8(icy_engine::IceMode::Unlimited) as u64);
                }
            }

            // Include caret position in hash
            let caret_pos = state.get_caret().get_position();
            hash = hash.wrapping_mul(31).wrapping_add(caret_pos.x as u64);
            hash = hash.wrapping_mul(31).wrapping_add(caret_pos.y as u64);

            hash
        } else {
            0
        }
    }

    pub fn check_and_update_cache(&mut self) -> bool {
        let current_hash = self.calculate_buffer_hash();
        if current_hash != self.last_buffer_hash {
            self.last_buffer_hash = current_hash;
            self.cache.clear();
            true
        } else {
            false
        }
    }

    // Helper function to convert buffer color to iced Color
    pub fn buffer_color_to_iced(color: icy_engine::Color) -> Color {
        let (r, g, b) = color.get_rgb_f32();
        Color::from_rgb(r, g, b)
    }
}
