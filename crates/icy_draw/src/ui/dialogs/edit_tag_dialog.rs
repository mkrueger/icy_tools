use std::fmt::Alignment;

use eframe::egui::{self, Layout};
use egui::{FontFamily, FontId, Label, RichText, ScrollArea, TextEdit};
use egui_modal::Modal;
use i18n_embed_fl::fl;
use icy_engine::Tag;

use crate::{AnsiEditor, Message, ModalDialog, TerminalResult};

struct TagDescr {
    example: String,
    tag: String,
    description: String,
}
pub struct EditTagDialog {
    should_commit: bool,
    tag_index: i32,
    tag: Tag,
    filter: String,
    show_replacements: bool,
    tag_list: Vec<TagDescr>,
}

impl EditTagDialog {
    pub fn new(tag: Tag, tag_index: i32) -> Self {
        let mut tag_list = Vec::new();
        for line in include_str!("../../../data/tags/pcboard.csv").lines() {
            if !line.contains(',') {
                continue;
            }
            let mut parts = line.split(',');
            let example = parts.next().unwrap().trim();
            let tag = parts.next().unwrap().trim();
            let description = parts.next().unwrap().trim();
            tag_list.push(TagDescr {
                example: example.to_string(),
                tag: tag.to_string(),
                description: description.to_string(),
            });
        }

        EditTagDialog {
            should_commit: false,
            show_replacements: false,
            filter: String::new(),
            tag_index,
            tag,
            tag_list,
        }
    }
}

impl ModalDialog for EditTagDialog {
    fn show(&mut self, ctx: &egui::Context) -> bool {
        let mut result = false;
        let modal = Modal::new(ctx, "edit_tag_dialog");

        modal.show(|ui| {
            ui.set_width(350.);

            modal.title(ui, fl!(crate::LANGUAGE_LOADER, "edit-tag-title"));

            modal.frame(ui, |ui| {
                if self.show_replacements {
                    ui.horizontal(|ui| {
                        ui.label("PCBoard Tags:");
                        ui.add(TextEdit::singleline(&mut self.filter).hint_text(fl!(crate::LANGUAGE_LOADER, "edit-tag-filter")));
                    });
                    let font_size = 12.0;
                    ui.separator();
                    ScrollArea::vertical().max_width(350.0).max_height(400.0).show(ui, |ui| {
                        for tag in &self.tag_list {
                            if self.filter.is_empty()
                                || tag.tag.to_lowercase().contains(&self.filter.to_lowercase())
                                || tag.description.to_lowercase().contains(&self.filter.to_lowercase())
                            {
                                ui.horizontal(|ui| {
                                    if ui
                                        .button(RichText::new(&tag.tag).font(FontId::new(font_size, FontFamily::Proportional)))
                                        .clicked()
                                    {
                                        self.tag.preview = tag.example.clone();
                                        self.tag.replacement_value = tag.tag.clone();
                                        self.show_replacements = false;
                                    }
                                    ui.add(Label::new(RichText::new(&tag.description).font(FontId::new(font_size, FontFamily::Proportional))).wrap(false));
                                });
                            }
                        }
                    });
                    ui.separator();
                    if ui.button(fl!(crate::LANGUAGE_LOADER, "tab-context-menu-close")).clicked() {
                        self.show_replacements = false;
                    }
                    ui.add_space(4.0);
                    return;
                }
                egui::Grid::new("some_unique_id").num_columns(2).spacing([4.0, 8.0]).show(ui, |ui| {
                    ui.with_layout(Layout::right_to_left(egui::Align::Center), |ui| {
                        ui.label(fl!(crate::LANGUAGE_LOADER, "edit-tag-preview-label"));
                    });
                    ui.add(egui::TextEdit::singleline(&mut self.tag.preview).char_limit(35));
                    ui.end_row();

                    ui.with_layout(Layout::right_to_left(egui::Align::Center), |ui| {
                        ui.label(fl!(crate::LANGUAGE_LOADER, "edit-tag-replacement-label"));
                    });
                    ui.add(egui::TextEdit::singleline(&mut self.tag.replacement_value).char_limit(35));
                    if ui.button("…").clicked() {
                        self.show_replacements = true;
                    }
                    ui.end_row();

                    ui.with_layout(Layout::right_to_left(egui::Align::Center), |ui| {
                        ui.label(fl!(crate::LANGUAGE_LOADER, "edit-tag-length-label"));
                    });
                    let mut tmp_str = self.tag.length.to_string();
                    ui.add(egui::TextEdit::singleline(&mut tmp_str).char_limit(35));
                    if let Ok(new_length) = tmp_str.parse::<usize>() {
                        self.tag.length = new_length;
                    }
                    ui.end_row();

                    ui.with_layout(Layout::right_to_left(egui::Align::Center), |ui| {
                        ui.label(fl!(crate::LANGUAGE_LOADER, "edit-tag-alignment-label"));
                    });
                    egui::ComboBox::from_id_source("combobox1")
                        .width(150.)
                        .selected_text(RichText::new(match self.tag.alignment {
                            Alignment::Left => fl!(crate::LANGUAGE_LOADER, "edit-tag-alignment-left"),
                            Alignment::Right => fl!(crate::LANGUAGE_LOADER, "edit-tag-alignment-right"),
                            Alignment::Center => fl!(crate::LANGUAGE_LOADER, "edit-tag-alignment-right"),
                        }))
                        .show_ui(ui, |ui| {
                            ui.selectable_value(&mut self.tag.alignment, Alignment::Left, fl!(crate::LANGUAGE_LOADER, "edit-tag-alignment-left"));
                            ui.selectable_value(
                                &mut self.tag.alignment,
                                Alignment::Right,
                                fl!(crate::LANGUAGE_LOADER, "edit-tag-alignment-right"),
                            );
                            ui.selectable_value(
                                &mut self.tag.alignment,
                                Alignment::Center,
                                fl!(crate::LANGUAGE_LOADER, "edit-tag-alignment-right"),
                            );
                        });
                    ui.end_row();
                    ui.label("");
                    ui.checkbox(&mut self.tag.is_enabled, fl!(crate::LANGUAGE_LOADER, "edit-layer-dialog-is-visible-checkbox"));
                    ui.end_row();
                });
                ui.add_space(4.0);
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

    fn commit(&self, _editor: &mut AnsiEditor) -> TerminalResult<Option<Message>> {
        if self.tag_index < 0 {
            Ok(Some(Message::AddNewTag(self.tag.clone().into())))
        } else {
            Ok(Some(Message::UpdateTag(self.tag.clone().into(), self.tag_index as usize)))
        }
    }
}
