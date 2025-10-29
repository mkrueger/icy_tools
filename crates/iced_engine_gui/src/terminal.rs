use std::{
    sync::{Arc, Mutex},
    time::Instant,
};

use iced::{Color, widget::canvas::Cache};
use icy_engine::editor::EditState;

use crate::{Blink, Message};

pub struct Terminal {
    pub edit_state: Arc<Mutex<EditState>>,
    pub font_size: f32,
    pub char_width: f32,
    pub char_height: f32,
    pub cache: Cache,

    pub caret_blink: Blink,
    pub character_blink: Blink,
    pub start_time: Instant,
}

impl Terminal {
    pub fn new(edit_state: Arc<Mutex<EditState>>) -> Self {
        Self {
            edit_state,
            font_size: 16.0,
            char_width: 9.6, // Approximate for monospace
            char_height: 20.0,
            cache: Cache::default(),
            caret_blink: Blink::new((1000.0 / 1.875) as u128 / 2),
            character_blink: Blink::new((1000.0 / 1.8) as u128),
            start_time: Instant::now(),
        }
    }

    pub fn reset_caret_blink(&mut self) {
        let cur_ms = self.start_time.elapsed().as_millis();
        self.caret_blink.reset(cur_ms);
    }

    pub fn check_blink_timers(&mut self) {
        let cur_ms = self.start_time.elapsed().as_millis();
        self.caret_blink.update(cur_ms);
        self.character_blink.update(cur_ms);
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

    // Helper function to convert buffer color to iced Color
    pub fn buffer_color_to_iced(color: icy_engine::Color) -> Color {
        let (r, g, b) = color.get_rgb_f32();
        Color::from_rgb(r, g, b)
    }
}
