use eframe::egui::{self, Layout};
use egui_modal::Modal;
use i18n_embed_fl::fl;
use icy_sauce::SauceInformation;

pub struct SauceDialog {
    sauce: SauceInformation,
}

pub enum Message {
    CloseDialog,
}

impl SauceDialog {
    pub fn new(sauce: SauceInformation) -> Self {
        Self { sauce }
    }

    pub fn show(&mut self, ctx: &egui::Context) -> Option<Message> {
        let mut message = None;
        let modal = Modal::new(ctx, "protocol_modal");
        modal.show(|ui| {
            modal.title(ui, fl!(crate::LANGUAGE_LOADER, "sauce-dialog-title"));

            modal.frame(ui, |ui: &mut egui::Ui| {
                egui::Grid::new("some_unique_id").num_columns(2).spacing([4.0, 8.0]).show(ui, |ui| {
                    ui.with_layout(Layout::right_to_left(egui::Align::Center), |ui| {
                        ui.label(fl!(crate::LANGUAGE_LOADER, "sauce-dialog-title-label"));
                    });
                    ui.add(egui::TextEdit::singleline(&mut self.sauce.title().to_string().as_str()).char_limit(35));
                    ui.end_row();

                    ui.with_layout(Layout::right_to_left(egui::Align::Center), |ui| {
                        ui.label(fl!(crate::LANGUAGE_LOADER, "sauce-dialog-author-label"));
                    });

                    ui.add(egui::TextEdit::singleline(&mut self.sauce.author().to_string().as_str()).char_limit(20));
                    ui.end_row();

                    ui.with_layout(Layout::right_to_left(egui::Align::Center), |ui| {
                        ui.label(fl!(crate::LANGUAGE_LOADER, "sauce-dialog-group-label"));
                    });
                    ui.add(egui::TextEdit::singleline(&mut self.sauce.group().to_string().as_str()).char_limit(20));
                    ui.end_row();

                    ui.with_layout(Layout::right_to_left(egui::Align::Center), |ui| {
                        ui.label(fl!(crate::LANGUAGE_LOADER, "sauce-dialog-date-label"));
                    });
                    let t = self.sauce.get_date().unwrap().format("%Y-%m-%d").to_string();
                    ui.add(egui::TextEdit::singleline(&mut t.as_str()).char_limit(20));
                    ui.end_row();
                    if let Ok(caps) = self.sauce.get_character_capabilities() {
                        if let Some(font) = &caps.font_opt {
                            ui.with_layout(Layout::right_to_left(egui::Align::Center), |ui| {
                                ui.label(fl!(crate::LANGUAGE_LOADER, "sauce-dialog-font-name"));
                            });
                            ui.add(egui::TextEdit::singleline(&mut font.to_string()).char_limit(20));
                            ui.end_row();
                        }

                        ui.with_layout(Layout::right_to_left(egui::Align::Center), |ui| {
                            ui.label(fl!(crate::LANGUAGE_LOADER, "sauce-dialog-flags-label"));
                        });
                        let mut flags: String = String::new();
                        if caps.use_ice {
                            flags.push_str("ice colors");
                        }

                        if caps.use_letter_spacing {
                            if !flags.is_empty() {
                                flags.push_str(", ");
                            }
                            flags.push_str("letter spacing");
                        }

                        if caps.use_aspect_ratio {
                            if !flags.is_empty() {
                                flags.push_str(", ");
                            }
                            flags.push_str("aspect ratio");
                        }
                        ui.add(egui::TextEdit::singleline(&mut flags.to_string().as_str()).char_limit(20));
                    }

                    ui.end_row();
                });

                let mut tmp_str = String::new();
                for s in self.sauce.comments() {
                    tmp_str.push_str(&s.to_string());
                    tmp_str.push('\n');
                }

                if !tmp_str.is_empty() {
                    ui.add_space(16.0);
                    ui.label(fl!(crate::LANGUAGE_LOADER, "sauce-dialog-comments-label"));
                    ui.add_space(4.0);
                    egui::ScrollArea::vertical().max_height(180.0).show(ui, |ui| {
                        ui.add(egui::TextEdit::multiline(&mut tmp_str.as_str()).desired_rows(6).desired_width(f32::INFINITY));
                    });
                }
            });

            modal.buttons(ui, |ui| {
                if modal.button(ui, fl!(crate::LANGUAGE_LOADER, "button-ok")).clicked() {
                    message = Some(Message::CloseDialog);
                }
            });
        });
        modal.open();

        message
    }
}
