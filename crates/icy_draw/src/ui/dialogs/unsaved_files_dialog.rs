use eframe::egui;
use egui_modal::{Modal, ModalStyle};
use egui_tiles::{Tile, TileId};
use i18n_embed_fl::fl;

use crate::{MainWindow, Message, TerminalResult};

pub struct UnsavedFilesDialog {
    do_commit: bool,
    save: bool,

    id: Vec<TileId>,
    path: Vec<std::path::PathBuf>,
}

impl UnsavedFilesDialog {
    pub(crate) fn new(path: Vec<std::path::PathBuf>, id: Vec<TileId>) -> Self {
        Self {
            save: false,
            do_commit: false,
            path,
            id,
        }
    }
}

impl crate::ModalDialog for UnsavedFilesDialog {
    fn show(&mut self, ctx: &egui::Context) -> bool {
        let mut result = false;
        let style = ModalStyle {
            default_width: Some(500.0),
            ..Default::default()
        };
        let modal = Modal::new(ctx, "ask_unsaved_files_dialog").with_style(&style);

        modal.show(|ui| {
            modal.frame(ui, |ui| {
                ui.strong(fl!(crate::LANGUAGE_LOADER, "ask_unsaved_file_dialog-description", number = self.path.len()));
                ui.vertical(|ui| {
                    ui.small("");
                    for p in self.path.iter() {
                        let file_name = if p.file_name().is_none() {
                            fl!(crate::LANGUAGE_LOADER, "unsaved-title")
                        } else {
                            if let Some(file_name) = p.file_name() {
                                format!("{}", file_name.to_string_lossy().to_string())
                            } else {
                                format!("{}", p.display())
                            }
                        };
                        ui.label(file_name);
                    }
                    ui.small("");
                });
                ui.small(fl!(crate::LANGUAGE_LOADER, "ask_unsaved_file_dialog-subdescription"));
            });

            modal.buttons(ui, |ui| {
                if ui.button(fl!(crate::LANGUAGE_LOADER, "ask_unsaved_file_dialog-save_all_button")).clicked() {
                    self.save = true;
                    self.do_commit = true;
                    result = true;
                }
                if ui.button(fl!(crate::LANGUAGE_LOADER, "new-file-cancel")).clicked() {
                    result = true;
                }
                if ui.button(fl!(crate::LANGUAGE_LOADER, "ask_unsaved_file_dialog-dont_save_button")).clicked() {
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

    fn commit_self(&self, ctx: &egui::Context, window: &mut MainWindow<'_>) -> TerminalResult<Option<Message>> {
        if self.save {
            for id in &self.id {
                if let Some(Tile::Pane(pane)) = window.document_tree.tiles.get_mut(*id) {
                    pane.save();
                }
            }
        }
        window.request_close = false;
        window.allowed_to_close = true;
        ctx.send_viewport_cmd(egui::ViewportCommand::Close);
        Ok(None)
    }
}
