use eframe::egui::{self, Layout};
use egui_modal::Modal;
use i18n_embed_fl::fl;
use icy_engine::TextPane;

use crate::{AnsiEditor, Message, ModalDialog, TerminalResult};

pub struct SetCanvasSizeDialog {
    should_commit: bool,
    width: i32,
    height: i32,
    resize_layer: bool,
}

impl SetCanvasSizeDialog {
    pub fn new(buf: &icy_engine::Buffer) -> Self {
        SetCanvasSizeDialog {
            should_commit: false,
            width: buf.get_width(),
            height: buf.get_height(),
            resize_layer: true,
        }
    }
}

impl ModalDialog for SetCanvasSizeDialog {
    fn show(&mut self, ctx: &egui::Context) -> bool {
        let mut result = false;
        let modal = Modal::new(ctx, "set_canvas_size_dialog");

        modal.show(|ui| {
            ui.set_width(250.);

            modal.title(ui, fl!(crate::LANGUAGE_LOADER, "edit-canvas-size-title"));

            modal.frame(ui, |ui| {
                egui::Grid::new("some_unique_id").num_columns(2).spacing([4.0, 8.0]).show(ui, |ui| {
                    ui.with_layout(Layout::right_to_left(egui::Align::Center), |ui| {
                        ui.label(fl!(crate::LANGUAGE_LOADER, "edit-canvas-size-width-label"));
                    });
                    let mut tmp_str = self.width.to_string();
                    ui.add(egui::TextEdit::singleline(&mut tmp_str).char_limit(35));
                    if let Ok(new_width) = tmp_str.parse::<i32>() {
                        self.width = new_width;
                    }
                    ui.end_row();

                    ui.with_layout(Layout::right_to_left(egui::Align::Center), |ui| {
                        ui.label(fl!(crate::LANGUAGE_LOADER, "edit-canvas-size-height-label"));
                    });
                    let mut tmp_str = self.height.to_string();
                    ui.add(egui::TextEdit::singleline(&mut tmp_str).char_limit(35));
                    if let Ok(new_height) = tmp_str.parse::<i32>() {
                        self.height = new_height;
                    }
                    ui.end_row();

                    ui.label("");
                    ui.checkbox(&mut self.resize_layer, fl!(crate::LANGUAGE_LOADER, "edit-canvas-size-resize_layers-label"));
                    ui.end_row();
                });
                ui.add_space(4.0);
            });

            modal.buttons(ui, |ui| {
                if ui.button(fl!(crate::LANGUAGE_LOADER, "edit-canvas-size-resize")).clicked() {
                    self.should_commit = true;
                    result = true;
                }
                if ui.button(fl!(crate::LANGUAGE_LOADER, "new-file-cancel")).clicked() {
                    result = true;
                }
            });
        });
        modal.open();
        result
    }

    fn should_commit(&self) -> bool {
        self.should_commit
    }

    fn commit(&self, _editor: &mut AnsiEditor) -> TerminalResult<Option<Message>> {
        Ok(Some(Message::ResizeBuffer(self.resize_layer, self.width, self.height)))
    }
}
