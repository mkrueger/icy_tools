mod main_window;

use eframe::egui;
pub use main_window::*;

mod document;
pub use document::*;

mod palette_editor;
pub use palette_editor::*;

mod tool_switcher;
pub use tool_switcher::*;

mod icons;
pub use icons::*;

mod settings;
pub use settings::*;

mod dialogs;
pub use dialogs::*;

mod editor;
pub use editor::*;

mod document_docking;
pub use document_docking::*;

mod tool_docking;
pub use tool_docking::*;

mod top_bar;
pub use top_bar::*;
mod messages;
pub use messages::*;

mod tools;
pub use tools::*;

mod commands;
pub use commands::*;

pub type TerminalResult<T> = anyhow::Result<T>;

pub trait ModalDialog {
    fn show(&mut self, ctx: &egui::Context) -> bool;

    fn should_commit(&self) -> bool;

    fn commit(&self, _editor: &mut AnsiEditor) -> TerminalResult<Option<Message>> {
        Ok(None)
    }

    fn commit_self(&self, _ctx: &egui::Context, _window: &mut MainWindow<'_>) -> TerminalResult<Option<Message>> {
        Ok(None)
    }
}
