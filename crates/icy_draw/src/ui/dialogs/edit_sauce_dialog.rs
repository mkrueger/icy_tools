use bstr::BString;
use eframe::egui::{self, Layout};
use egui_modal::Modal;
use i18n_embed_fl::fl;
use icy_sauce::SauceMetaInformation;

use crate::{to_message, AnsiEditor, Message, ModalDialog, TerminalResult};

pub struct EditSauceDialog {
    pub should_commit: bool,
    pub sauce_data: SauceMetaInformation,
    pub comments: String,
}

impl EditSauceDialog {
    pub fn new(buf: &icy_engine::Buffer) -> Self {
        let mut comments = String::new();

        for s in &buf.get_sauce_meta().comments {
            comments.push_str(&s.to_string());
            comments.push('\n');
        }

        EditSauceDialog {
            should_commit: false,
            sauce_data: buf.get_sauce_meta().clone(),
            comments,
        }
    }
}

impl ModalDialog for EditSauceDialog {
    fn show(&mut self, ctx: &egui::Context) -> bool {
        let mut result = false;
        let modal: Modal = Modal::new(ctx, "edit_sauce_dialog");

        modal.show(|ui| {
            modal.title(ui, fl!(crate::LANGUAGE_LOADER, "edit-sauce-title"));

            modal.frame(ui, |ui| {
                egui::Grid::new("some_unique_id").num_columns(2).spacing([4.0, 8.0]).show(ui, |ui| {
                    ui.with_layout(Layout::right_to_left(egui::Align::Center), |ui| {
                        ui.label(fl!(crate::LANGUAGE_LOADER, "edit-sauce-title-label"));
                    });
                    ui.horizontal(|ui| {
                        let mut tmp_str = self.sauce_data.title.to_string();
                        ui.add(egui::TextEdit::singleline(&mut tmp_str).char_limit(35));
                        self.sauce_data.title = BString::from(tmp_str);
                        ui.label(fl!(crate::LANGUAGE_LOADER, "edit-sauce-title-label-length"));
                    });
                    ui.end_row();

                    ui.with_layout(Layout::right_to_left(egui::Align::Center), |ui| {
                        ui.label(fl!(crate::LANGUAGE_LOADER, "edit-sauce-author-label"));
                    });

                    ui.horizontal(|ui| {
                        let mut tmp_str = self.sauce_data.author.to_string();
                        ui.add(egui::TextEdit::singleline(&mut tmp_str).char_limit(20));
                        self.sauce_data.author = BString::from(tmp_str);
                        ui.label(fl!(crate::LANGUAGE_LOADER, "edit-sauce-author-label-length"));
                    });
                    ui.end_row();

                    ui.with_layout(Layout::right_to_left(egui::Align::Center), |ui| {
                        ui.label(fl!(crate::LANGUAGE_LOADER, "edit-sauce-group-label"));
                    });
                    ui.horizontal(|ui| {
                        let mut tmp_str = self.sauce_data.group.to_string();
                        ui.add(egui::TextEdit::singleline(&mut tmp_str).char_limit(20));
                        self.sauce_data.group = BString::from(tmp_str);
                        ui.label(fl!(crate::LANGUAGE_LOADER, "edit-sauce-group-label-length"));
                    });
                    ui.end_row();
                    /*
                    ui.with_layout(Layout::right_to_left(egui::Align::Center), |ui| {
                        ui.label(fl!(crate::LANGUAGE_LOADER, "edit-sauce-letter-spacing"));
                    });
                    ui.horizontal(|ui| {
                        ui.checkbox(&mut self.sauce_data.use_letter_spacing, "");
                    });
                    ui.end_row();

                    ui.with_layout(Layout::right_to_left(egui::Align::Center), |ui| {
                        ui.label(fl!(crate::LANGUAGE_LOADER, "edit-sauce-aspect-ratio"));
                    });
                    ui.horizontal(|ui| {
                        ui.checkbox(&mut self.sauce_data.use_aspect_ratio, "");
                    });
                    ui.end_row();*/
                });
                ui.add_space(16.0);
                ui.label(fl!(crate::LANGUAGE_LOADER, "edit-sauce-comments-label"));
                ui.add_space(4.0);
                self.sauce_data.comments.clear();
                egui::ScrollArea::vertical().max_height(180.0).show(ui, |ui| {
                    ui.add(egui::TextEdit::multiline(&mut self.comments).desired_rows(6).desired_width(f32::INFINITY));
                    let mut new_comments = String::new();
                    for line in self.comments.lines() {
                        if line.len() > 64 {
                            new_comments.push_str(&line[..64]);
                        } else {
                            new_comments.push_str(line);
                        }
                        new_comments.push('\n');
                    }
                    self.comments = new_comments;
                });
            });

            modal.buttons(ui, |ui| {
                if ui.button(fl!(crate::LANGUAGE_LOADER, "new-file-ok")).clicked() {
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

    fn commit(&self, editor: &mut AnsiEditor) -> TerminalResult<Option<Message>> {
        let mut data = self.sauce_data.clone();
        let mut number = 0;
        for line in self.comments.lines() {
            data.comments.push(BString::from(line));
            number += 1;
            // limit to 255 chars which is the maximum for sauce comment lines.
            if number > 255 {
                break;
            }
        }

        let bv = &mut editor.buffer_view.lock();
        Ok(to_message(bv.get_edit_state_mut().update_sauce_data(data)))
    }
}
