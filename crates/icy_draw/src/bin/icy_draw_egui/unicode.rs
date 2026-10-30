use super::DrawApp;
use eframe::egui;
use icy_draw::fl;
use icy_engine_gui::egui::appearance::{self, labels, DialogButton, DialogSize};

const BLOCKS: [(&str, u32); 10] = [
    ("Latin", 0x0000),
    ("Greek", 0x0300),
    ("Cyrillic", 0x0400),
    ("Symbols", 0x2100),
    ("Arrows", 0x2200),
    ("Box Drawing / Block Elements", 0x2500),
    ("Braille", 0x2800),
    ("CJK", 0x4E00),
    ("Emoji", 0x1F600),
    ("Symbols for Legacy Computing", 0x1FB00),
];

pub(super) fn glyph_button(ui: &mut egui::Ui, character: char) -> egui::Response {
    let response = character_button(ui, Some(character), false, egui::vec2(32.0, 28.0));
    if ui.is_rect_visible(response.rect) {
        icy_engine_gui::request_unicode_ui_fonts(ui.ctx(), &[character]);
    }
    response
}

fn character_button(ui: &mut egui::Ui, character: Option<char>, selected: bool, size: egui::Vec2) -> egui::Response {
    let response = ui.add_enabled(character.is_some(), egui::Button::new("").min_size(size).selected(selected));
    if ui.is_rect_visible(response.rect) {
        if let Some(character) = character {
            ui.painter().with_clip_rect(response.rect.intersect(ui.clip_rect())).text(
                response.rect.center(),
                egui::Align2::CENTER_CENTER,
                character,
                egui::FontId::monospace(20.0),
                ui.style().interact_selectable(&response, selected).text_color(),
            );
        }
    }
    response
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn character_buttons_only_layout_visible_glyphs() {
        let context = egui::Context::default();
        let mut output = context.run_ui(egui::RawInput::default(), |ui| {
            egui::CentralPanel::default_margins().show(ui, |ui| {
                let top = ui.cursor().min;
                ui.set_clip_rect(egui::Rect::from_min_size(top, egui::vec2(80.0, 28.0)));
                for character in ['A', 'B', 'C', 'D'] {
                    character_button(ui, Some(character), false, egui::vec2(31.0, 27.0));
                }
            });
        });
        output.textures_delta.clear();
        let characters: Vec<_> = output
            .shapes
            .iter()
            .filter_map(|shape| match &shape.shape {
                egui::Shape::Text(text) if !text.galley.text().is_empty() => Some(text.galley.text().to_owned()),
                _ => None,
            })
            .collect();
        assert_eq!(characters, ["A"]);
    }
}

impl DrawApp {
    /// Makes `character` the current character; the text tool also types it once at the caret.
    pub(super) fn choose_character(&mut self, character: char) {
        self.document.brush.paint_char = character;
        if self.document.tool == super::Tool::Click {
            self.insert_character(character);
        }
    }

    /// Types `character` at the caret, like an F-key.
    pub(super) fn insert_character(&mut self, character: char) {
        if self.document.can_paint() {
            self.edit(|state| state.type_key(character));
            self.canvas_focus = true;
        }
    }

    pub(super) fn unicode_character_dialog(&mut self, context: &egui::Context) -> bool {
        #[derive(Clone, Copy)]
        enum Action {
            Cancel,
            Select,
        }

        let id = egui::Id::new("unicode-character-picker");
        let initial = self.document.brush.paint_char as u32;
        let (mut page, mut code) = context
            .data(|data| data.get_temp::<(u32, String)>(id))
            .unwrap_or((initial / 256, format!("{initial:04X}")));
        let mut chosen = None;
        let mut parsed = u32::from_str_radix(code.trim().trim_start_matches("U+"), 16)
            .ok()
            .and_then(char::from_u32)
            .filter(|ch| !ch.is_control());
        let response = appearance::Dialog::new("unicode-characters")
            .title(fl!("characters-title"))
            .size(DialogSize::Width(620.0))
            .fixed_height(700.0)
            .show(context, |dialog| {
                dialog.content(|ui| {
                    appearance::form_row(ui, &fl!("unicode-block"), |ui| {
                        egui::ComboBox::from_id_salt("unicode-block")
                            .selected_text(format!("U+{:04X} - U+{:04X}", page * 256, page * 256 + 255))
                            .show_ui(ui, |ui| {
                                for (name, start) in BLOCKS {
                                    ui.selectable_value(&mut page, start / 256, name);
                                }
                            });
                        ui.add(egui::DragValue::new(&mut page).range(0..=0x10FF));
                    });
                    appearance::form_row(ui, &fl!("unicode-codepoint"), |ui| {
                        ui.label("U+");
                        if ui.text_edit_singleline(&mut code).changed() {
                            parsed = u32::from_str_radix(code.trim().trim_start_matches("U+"), 16)
                                .ok()
                                .and_then(char::from_u32)
                                .filter(|ch| !ch.is_control());
                            if let Some(character) = parsed {
                                page = character as u32 / 256;
                            }
                        }
                        if let Some(character) = parsed {
                            ui.monospace(character.to_string());
                        } else {
                            ui.colored_label(ui.visuals().error_fg_color, fl!("unicode-invalid-codepoint"));
                        }
                    });
                    let mut visible_characters = Vec::new();
                    ui.scope(|ui| {
                        ui.spacing_mut().button_padding = egui::vec2(2.0, 1.0);
                        egui::Grid::new("unicode-glyphs").spacing(egui::vec2(2.0, 2.0)).show(ui, |ui| {
                            for offset in 0..256 {
                                let scalar = page * 256 + offset;
                                let character = char::from_u32(scalar).filter(|ch| !ch.is_control());
                                let response = character_button(ui, character, character.is_some() && character == parsed, egui::vec2(31.0, 27.0))
                                    .on_hover_text(format!("U+{scalar:04X}"));
                                if ui.is_rect_visible(response.rect) {
                                    visible_characters.extend(character);
                                }
                                if response.clicked() {
                                    chosen = character;
                                }
                                if offset % 16 == 15 {
                                    ui.end_row();
                                }
                            }
                        });
                        icy_engine_gui::request_unicode_ui_fonts(context, &visible_characters);
                    });
                    ui.weak(fl!("unicode-mode-hint"));
                });
                dialog.buttons([
                    DialogButton::cancel(labels::cancel(), Action::Cancel),
                    DialogButton::primary(fl!("select-font-dialog-select"), Action::Select),
                ]);
            });
        if matches!(response.action, Some(Action::Select)) {
            chosen = parsed;
        }
        if let Some(character) = chosen {
            self.choose_character(character);
            context.data_mut(|data| data.remove::<(u32, String)>(id));
            self.canvas_focus = true;
            true
        } else if response.dismissed || matches!(response.action, Some(Action::Cancel)) {
            context.data_mut(|data| data.remove::<(u32, String)>(id));
            self.canvas_focus = true;
            true
        } else {
            context.data_mut(|data| data.insert_temp(id, (page, code)));
            false
        }
    }
}
