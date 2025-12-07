//! Channels view component
//!
//! Shows FG/BG channel toggles and color selection.

use std::sync::Arc;

use iced::{Element, Length, Task, widget::{button, column, row, text}};
use icy_engine_edit::EditState;
use parking_lot::Mutex;

/// Messages for the channels view
#[derive(Clone, Debug)]
pub enum ChannelsMessage {
    /// Toggle foreground channel
    ToggleForeground,
    /// Toggle background channel
    ToggleBackground,
}

/// Channels view state
pub struct ChannelsView {
    /// Use foreground when drawing
    pub use_foreground: bool,
    /// Use background when drawing
    pub use_background: bool,
}

impl Default for ChannelsView {
    fn default() -> Self {
        Self::new()
    }
}

impl ChannelsView {
    pub fn new() -> Self {
        Self {
            use_foreground: true,
            use_background: true,
        }
    }

    /// Update the channels view state
    pub fn update(&mut self, message: ChannelsMessage) -> Task<ChannelsMessage> {
        match message {
            ChannelsMessage::ToggleForeground => {
                self.use_foreground = !self.use_foreground;
                Task::none()
            }
            ChannelsMessage::ToggleBackground => {
                self.use_background = !self.use_background;
                Task::none()
            }
        }
    }

    /// Render the channels view
    pub fn view<'a>(&'a self, edit_state: &'a Arc<Mutex<EditState>>) -> Element<'a, ChannelsMessage> {
        let state = edit_state.lock();
        let caret = state.get_caret();
        let fg_color = caret.attribute.get_foreground();
        let bg_color = caret.attribute.get_background();
        
        row![
            column![
                button(text(format!("FG {}", if self.use_foreground { "✓" } else { "○" })).size(10))
                .on_press(ChannelsMessage::ToggleForeground)
                .padding(2),
                text(format!("{}", fg_color)).size(10),
            ]
            .spacing(2)
            .width(Length::FillPortion(1)),
            column![
                button(text(format!("BG {}", if self.use_background { "✓" } else { "○" })).size(10))
                .on_press(ChannelsMessage::ToggleBackground)
                .padding(2),
                text(format!("{}", bg_color)).size(10),
            ]
            .spacing(2)
            .width(Length::FillPortion(1)),
        ]
        .spacing(8)
        .into()
    }
}
