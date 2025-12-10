//! Re-export of the export dialog from icy_engine_gui
//!
//! This module re-exports the shared export dialog implementation and provides
//! the necessary adapter functions for icy_term.

use iced::Element;
use icy_engine::{BufferType, Screen};
use parking_lot::Mutex;
use std::sync::Arc;

pub use icy_engine_gui::ui::{ExportDialogMessage, ExportDialogState};
use icy_engine_gui::StateResult;

use crate::ui::MainWindowMode;

/// Extension trait to add icy_term specific functionality to ExportDialogState
pub trait ExportDialogExt {
    fn update_icy_term(&mut self, message: ExportDialogMessage) -> Option<crate::ui::Message>;
    fn view_icy_term<'a>(&'a self, terminal_content: Element<'a, crate::ui::Message>) -> Element<'a, crate::ui::Message>;
}

impl ExportDialogExt for ExportDialogState {
    fn update_icy_term(&mut self, message: ExportDialogMessage) -> Option<crate::ui::Message> {
        match self.handle_message(message) {
            StateResult::Success(_) | StateResult::Close => {
                Some(crate::ui::Message::CloseDialog(Box::new(MainWindowMode::ShowTerminal)))
            }
            StateResult::None => None,
        }
    }

    fn view_icy_term<'a>(&'a self, terminal_content: Element<'a, crate::ui::Message>) -> Element<'a, crate::ui::Message> {
        let dialog_view = self.view(|msg| crate::ui::Message::ExportDialog(msg));
        crate::ui::modal(terminal_content, dialog_view, crate::ui::Message::ExportDialog(ExportDialogMessage::Cancel))
    }
}

/// Create a new export dialog state with icy_term defaults
pub fn new_export_dialog(initial_path: String, buffer_type: BufferType, screen: Arc<Mutex<Box<dyn Screen>>>) -> ExportDialogState {
    ExportDialogState::new(initial_path, buffer_type, screen).with_default_directory_fn(|| crate::data::Options::default_capture_directory())
}
