use eframe::egui::{self, color_picker, Layout, RichText};
use egui_modal::Modal;
use i18n_embed_fl::fl;
use icy_engine::{Color, Mode, Properties};

use crate::{AnsiEditor, Message, ModalDialog, TerminalResult};

pub struct EditLayerDialog {
    pub should_commit: bool,

    layer: usize,

    properties: Properties,
}

impl EditLayerDialog {
    pub fn new(buf: &icy_engine::Buffer, layer: usize) -> Self {
        let l = &buf.layers[layer];
        EditLayerDialog {
            should_commit: false,
            layer,
            properties: l.properties.clone(),
        }
    }
}

impl ModalDialog for EditLayerDialog {
    fn show(&mut self, ctx: &egui::Context) -> bool {
        let mut result = false;
        let modal: Modal = Modal::new(ctx, "edit_layer_dialog");

        modal.show(|ui| {
            modal.title(ui, fl!(crate::LANGUAGE_LOADER, "edit-layer-dialog-title"));

            modal.frame(ui, |ui| {
                egui::Grid::new("some_unique_id").num_columns(2).spacing([4.0, 8.0]).show(ui, |ui| {
                    ui.with_layout(Layout::right_to_left(egui::Align::Center), |ui| {
                        ui.label(fl!(crate::LANGUAGE_LOADER, "edit-layer-dialog-name-label"));
                    });
                    ui.add(egui::TextEdit::singleline(&mut self.properties.title));
                    ui.end_row();

                    if self.properties.color.is_some() {
                        let mut use_color = true;

                        if let Some(color) = &mut self.properties.color {
                            ui.label("");
                            ui.horizontal(|ui| {
                                let mut c: [u8; 3] = (color.clone()).into();
                                color_picker::color_edit_button_srgb(ui, &mut c);
                                *color = c.into();

                                ui.checkbox(&mut use_color, "Use Color");
                            });
                            ui.end_row();
                        }

                        if !use_color {
                            self.properties.color = None;
                        }
                    } else {
                        ui.label("");
                        let mut use_color = false;
                        ui.checkbox(&mut use_color, "Use Color");
                        ui.end_row();
                        if use_color {
                            self.properties.color = Some(Color::new(255, 255, 255));
                        }
                    }

                    ui.label("");
                    ui.checkbox(
                        &mut self.properties.is_visible,
                        fl!(crate::LANGUAGE_LOADER, "edit-layer-dialog-is-visible-checkbox"),
                    );
                    ui.end_row();
                    ui.label("");
                    ui.checkbox(
                        &mut self.properties.is_locked,
                        fl!(crate::LANGUAGE_LOADER, "edit-layer-dialog-is-edit-locked-checkbox"),
                    );
                    ui.end_row();
                    ui.label("");
                    ui.checkbox(
                        &mut self.properties.is_position_locked,
                        fl!(crate::LANGUAGE_LOADER, "edit-layer-dialog-is-position-locked-checkbox"),
                    );
                    ui.end_row();

                    ui.label("");
                    ui.checkbox(
                        &mut self.properties.has_alpha_channel,
                        fl!(crate::LANGUAGE_LOADER, "edit-layer-dialog-has-alpha-checkbox"),
                    );
                    ui.end_row();

                    if self.properties.has_alpha_channel {
                        ui.label("");
                        ui.checkbox(
                            &mut self.properties.is_alpha_channel_locked,
                            fl!(crate::LANGUAGE_LOADER, "edit-layer-dialog-is-alpha-locked-checkbox"),
                        );
                        ui.end_row();
                    }

                    ui.label("Mode:");

                    egui::ComboBox::from_id_source("combobox1")
                        .width(150.)
                        .selected_text(RichText::new(match self.properties.mode {
                            Mode::Normal => "Normal",
                            Mode::Chars => "Chars only",
                            Mode::Attributes => "Attribute only",
                        }))
                        .show_ui(ui, |ui| {
                            ui.selectable_value(&mut self.properties.mode, Mode::Normal, "Normal");
                            ui.selectable_value(&mut self.properties.mode, Mode::Chars, "Chars only");
                            ui.selectable_value(&mut self.properties.mode, Mode::Attributes, "Attribute only");
                        });
                    ui.end_row();

                    ui.label(fl!(crate::LANGUAGE_LOADER, "edit-layer-dialog-is-x-offset-label"));
                    ui.add(egui::DragValue::new(&mut self.properties.offset.x));

                    ui.end_row();

                    ui.label(fl!(crate::LANGUAGE_LOADER, "edit-layer-dialog-is-y-offset-label"));
                    ui.add(egui::DragValue::new(&mut self.properties.offset.y));
                    ui.end_row();
                });
                ui.add_space(16.0);
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
        let mut bv = editor.buffer_view.lock();

        if bv.get_buffer_mut().layers[self.layer].properties != self.properties {
            bv.get_edit_state_mut().update_layer_properties(self.layer, self.properties.clone())?;
        }
        Ok(None)
    }
}
