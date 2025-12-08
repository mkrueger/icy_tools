//! Minimap view component
//!
//! Shows a small preview of the entire document with a viewport indicator.

use std::sync::Arc;

use iced::{
    Element, Length, Task,
    widget::{container, text},
};
use icy_engine::{Screen, TextPane};
use icy_engine_edit::EditState;
use parking_lot::Mutex;

/// Messages for the minimap view
#[derive(Clone, Debug)]
pub enum MinimapMessage {
    /// Click on minimap to scroll to position
    Click(f32, f32),
    /// Drag on minimap to scroll
    Drag(f32, f32),
}

/// Minimap view state
pub struct MinimapView {
    /// Cached minimap image (to avoid re-rendering every frame)
    cached_image: Option<iced::widget::image::Handle>,
    /// Cache key (buffer hash/version)
    cache_version: u64,
}

impl Default for MinimapView {
    fn default() -> Self {
        Self::new()
    }
}

impl MinimapView {
    pub fn new() -> Self {
        Self {
            cached_image: None,
            cache_version: 0,
        }
    }

    /// Invalidate the cache
    pub fn invalidate_cache(&mut self) {
        self.cached_image = None;
    }

    /// Update the minimap view state
    pub fn update(&mut self, message: MinimapMessage) -> Task<MinimapMessage> {
        match message {
            MinimapMessage::Click(_x, _y) => {
                // TODO: Scroll main view to clicked position
                Task::none()
            }
            MinimapMessage::Drag(_x, _y) => {
                // TODO: Scroll main view while dragging
                Task::none()
            }
        }
    }

    /// Render the minimap view
    pub fn view<'a>(&'a self, screen: &'a Arc<Mutex<Box<dyn Screen>>>) -> Element<'a, MinimapMessage> {
        let mut screen_guard = screen.lock();
        let state = screen_guard.as_any_mut().downcast_mut::<EditState>().expect("Screen should be EditState");
        let buffer = state.get_buffer();

        // TODO: Render actual minimap using buffer.render_to_rgba()
        // For now, show placeholder

        container(text(format!("{}×{}", buffer.get_width(), buffer.get_height())).size(10))
            .width(Length::Fill)
            .height(Length::Fill)
            .center_x(Length::Fill)
            .center_y(Length::Fill)
            .style(container::bordered_box)
            .into()
    }
}
