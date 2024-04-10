use eframe::egui;
use egui_modal::Modal;
use i18n_embed_fl::fl;

use crate::{util::autosave::remove_autosave, MainWindow, Message, TerminalResult};

#[derive(Default)]
pub struct AutoSaveDialog {
    finish: bool,
    load_autosave: bool,
    path: std::path::PathBuf,
}
impl AutoSaveDialog {
    pub(crate) fn new(path: std::path::PathBuf) -> Self {
        Self {
            load_autosave: false,
            finish: false,
            path,
        }
    }
}

impl crate::ModalDialog for AutoSaveDialog {
    fn show(&mut self, ctx: &egui::Context) -> bool {
        let mut result = false;
        let modal = Modal::new(ctx, "autosave_dialog");

        modal.show(|ui| {
            modal.title(ui, fl!(crate::LANGUAGE_LOADER, "autosave-dialog-title"));

            modal.frame(ui, |ui| {
                ui.strong(fl!(crate::LANGUAGE_LOADER, "autosave-dialog-description"));
                ui.label(fl!(crate::LANGUAGE_LOADER, "autosave-dialog-question"));
            });

            modal.buttons(ui, |ui| {
                if ui.button(fl!(crate::LANGUAGE_LOADER, "autosave-dialog-load_autosave_button")).clicked() {
                    self.load_autosave = true;
                    self.finish = true;
                    result = true;
                }

                if ui.button(fl!(crate::LANGUAGE_LOADER, "autosave-dialog-discard_autosave_button")).clicked() {
                    remove_autosave(&self.path);
                    self.load_autosave = false;
                    self.finish = true;
                    result = true;
                }
            });
        });
        modal.open();
        result
    }

    fn should_commit(&self) -> bool {
        self.finish
    }

    fn commit_self(&self, _window: &mut MainWindow<'_>) -> TerminalResult<Option<Message>> {
        Ok(Some(Message::LoadFile(self.path.clone(), self.load_autosave)))
    }
}
