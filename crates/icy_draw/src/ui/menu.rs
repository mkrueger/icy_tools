//! Menu system for icy_draw
//!
//! Temporary simple menu bar until iced_aw version conflicts are resolved.
//! Will use iced_aw's MenuBar when available.

use iced::{
    Element,
    widget::{button, row, text},
};

use super::main_window::Message;
// Use command definitions from icy_engine_gui (LazyLock<CommandDef>)
use icy_engine_gui::commands::cmd;

/// Menu builder that creates menu bars using CommandDef for translations
pub struct MenuBuilder;

impl MenuBuilder {
    pub fn new() -> Self {
        Self
    }

    /// Build a simple button-based menu bar (temporary until iced_aw works)
    ///
    /// Uses CommandDef.label_menu which provides translated menu labels
    /// loaded lazily from the correct i18n loader.
    pub fn build(&self) -> Element<'_, Message> {
        row![
            button(text(&cmd::FILE_NEW.label_menu)).on_press(Message::NewFile).padding([4, 8]),
            button(text(&cmd::FILE_OPEN.label_menu)).on_press(Message::OpenFile).padding([4, 8]),
            button(text(&cmd::FILE_SAVE.label_menu)).on_press(Message::SaveFile).padding([4, 8]),
            text(" | ").size(16),
            button(text(&cmd::EDIT_UNDO.label_menu)).on_press(Message::Undo).padding([4, 8]),
            button(text(&cmd::EDIT_REDO.label_menu)).on_press(Message::Redo).padding([4, 8]),
            text(" | ").size(16),
            button(text(&cmd::VIEW_ZOOM_IN.label_menu)).on_press(Message::ZoomIn).padding([4, 8]),
            button(text(&cmd::VIEW_ZOOM_OUT.label_menu)).on_press(Message::ZoomOut).padding([4, 8]),
            button(text(&cmd::VIEW_ZOOM_RESET.label_menu)).on_press(Message::ZoomReset).padding([4, 8]),
        ]
        .spacing(4)
        .padding(4)
        .into()
    }
}
