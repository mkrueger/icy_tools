use eframe::egui;
use egui_modal::Modal;
use egui_tiles::TileId;
use i18n_embed_fl::fl;

use crate::{MainWindow, Message, SaveFileDialog, TerminalResult, util::autosave::remove_autosave};

pub struct AskCloseFileDialog {
    do_commit: bool,
    save: bool,

    id: TileId,
    path: Option<std::path::PathBuf>,
}

impl AskCloseFileDialog {
    pub(crate) fn new(path: Option<std::path::PathBuf>, id: TileId) -> Self {
        Self {
            save: false,
            do_commit: false,
            path,
            id,
        }
    }
}

impl crate::ModalDialog for AskCloseFileDialog {
    fn show(&mut self, ctx: &egui::Context) -> bool {
        let mut result = false;
        let modal = Modal::new(ctx, "ask_close_file_dialog");

        modal.show(|ui| {
            let file_name = if let Some(file_name) = &self.path {
                file_name.file_name().unwrap().to_string_lossy().to_string()
            } else {
                fl!(crate::LANGUAGE_LOADER, "unsaved-title")
            };

            modal.frame(ui, |ui| {
                ui.strong(fl!(crate::LANGUAGE_LOADER, "ask_close_file_dialog-description", filename = file_name));
                ui.small(fl!(crate::LANGUAGE_LOADER, "ask_close_file_dialog-subdescription"));
            });

            modal.buttons(ui, |ui| {
                if ui.button(fl!(crate::LANGUAGE_LOADER, "ask_close_file_dialog-save_button")).clicked() {
                    self.save = true;
                    self.do_commit = true;
                    result = true;
                }
                if ui.button(fl!(crate::LANGUAGE_LOADER, "new-file-cancel")).clicked() {
                    result = true;
                }
                if ui.button(fl!(crate::LANGUAGE_LOADER, "ask_close_file_dialog-dont_save_button")).clicked() {
                    self.save = false;
                    self.do_commit = true;
                    result = true;
                }
            });
        });
        modal.open();
        result
    }

    fn should_commit(&self) -> bool {
        self.do_commit
    }

    fn commit_self(&self, _ctx: &egui::Context, window: &mut MainWindow<'_>) -> TerminalResult<Option<Message>> {
        let mut msg = None;
        if self.save {
            if self.path.is_none() {
                window.open_dialog(SaveFileDialog::new(None));
                return Ok(None);
            }
            if let Some(egui_tiles::Tile::Pane(pane)) = window.document_tree.tiles.get_mut(self.id) {
                // TODO: potential message clash. Maybe we should return a Vec<Message> instead? OR a Vec MessageType?
                msg = pane.save();
                let msg2 = pane.destroy(&window.gl);
                if msg2.is_some() {
                    msg = msg2;
                }
            }
        } else if let Some(egui_tiles::Tile::Pane(pane)) = window.document_tree.tiles.get_mut(self.id) {
            msg = pane.destroy(&window.gl);
        }
        window.document_tree.tiles.remove(self.id);
        if let Some(path) = &self.path {
            remove_autosave(path);
        }
        Ok(msg)
    }
}
