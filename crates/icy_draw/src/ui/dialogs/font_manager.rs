use std::sync::Arc;

use crate::{AnsiEditor, Message, TerminalResult};
use eframe::{
    egui::{self, Button, Sense, TextStyle, TopBottomPanel, WidgetText},
    epaint::{FontFamily, FontId, Rounding},
};
use egui_modal::Modal;
use i18n_embed_fl::fl;

pub struct FontManager {
    selected: usize,
    replace_with: usize,
    do_select: bool,
    buffer_view: Arc<eframe::epaint::mutex::Mutex<icy_engine_gui::BufferView>>,
    used_fonts: Vec<usize>,
}

impl FontManager {
    pub fn new(editor: &AnsiEditor) -> Self {
        let used_fonts = icy_engine::analyze_font_usage(editor.buffer_view.lock().get_buffer());
        Self {
            selected: 0,
            do_select: false,
            replace_with: 0,
            buffer_view: editor.buffer_view.clone(),
            used_fonts,
        }
    }

    fn update_used_fonts(&mut self) {
        let lock = &self.buffer_view.lock();
        self.used_fonts = icy_engine::analyze_font_usage(lock.get_buffer());
    }
}

impl crate::ModalDialog for FontManager {
    fn show(&mut self, ctx: &egui::Context) -> bool {
        let mut result = false;
        let modal = Modal::new(ctx, "manage_font_dialog");
        modal.show(|ui| {
            ui.set_height(320.);
            ui.set_width(600.);

            modal.title(ui, fl!(crate::LANGUAGE_LOADER, "manage-font-dialog-title"));
            modal.frame(ui, |ui| {
                egui::CentralPanel::default().show_inside(ui, |ui| {
                    let row_height = 24.0;
                    ui.label(fl!(crate::LANGUAGE_LOADER, "manage-font-used_font_label"));
                    egui::ScrollArea::vertical().id_salt("bitfont_scroll_area").show(ui, |ui| {
                        for (i, font) in self.buffer_view.lock().get_buffer().font_iter() {
                            let is_selected = *i == self.selected;

                            let (id, rect) = ui.allocate_space([ui.available_width(), row_height].into());
                            let response = ui.interact(rect, id, Sense::click());
                            if response.hovered() {
                                ui.painter()
                                    .rect_filled(rect.expand(1.0), Rounding::same(4.0), ui.style().visuals.widgets.active.bg_fill);
                            } else if is_selected {
                                ui.painter()
                                    .rect_filled(rect.expand(1.0), Rounding::same(4.0), ui.style().visuals.extreme_bg_color);
                            }

                            let font_id = FontId::new(12.0, FontFamily::Monospace);
                            let text: WidgetText = format!("{i:-3}.").into();
                            let galley = text.into_galley(ui, Some(egui::TextWrapMode::Truncate), f32::INFINITY, font_id.clone());
                            let size = galley.size();
                            let mut title_rect = rect;
                            title_rect.set_left(title_rect.left() + 4.0);
                            title_rect.set_top(title_rect.bottom() - size.y - 8.0);
                            let text_color = if is_selected {
                                ui.style().visuals.strong_text_color()
                            } else {
                                ui.style().visuals.text_color()
                            };
                            ui.painter().galley_with_override_text_color(
                                egui::Align2::LEFT_TOP.align_size_within_rect(galley.size(), title_rect.shrink(4.0)).min,
                                galley,
                                text_color,
                            );

                            let font_id = TextStyle::Button.resolve(ui.style());
                            let text: WidgetText = font.name.clone().into();
                            let galley = text.into_galley(ui, Some(egui::TextWrapMode::Truncate), f32::INFINITY, font_id);
                            let mut title_rect = rect;
                            title_rect.set_left(title_rect.left() + 4.0 + size.x + 4.0);
                            ui.painter().galley_with_override_text_color(
                                egui::Align2::LEFT_TOP.align_size_within_rect(galley.size(), title_rect.shrink(4.0)).min,
                                galley,
                                text_color,
                            );

                            let font_id = TextStyle::Button.resolve(ui.style());
                            let text: WidgetText = format!("{}x{}", font.size.width, font.size.height).into();
                            let galley = text.into_galley(ui, Some(egui::TextWrapMode::Truncate), f32::INFINITY, font_id);
                            let mut title_rect = rect;
                            title_rect.set_left(title_rect.left() + 399.0);
                            ui.painter().galley_with_override_text_color(
                                egui::Align2::LEFT_TOP.align_size_within_rect(galley.size(), title_rect.shrink(4.0)).min,
                                galley,
                                text_color,
                            );

                            let font_id = TextStyle::Button.resolve(ui.style());
                            let text: WidgetText = if self.used_fonts.contains(i) {
                                fl!(crate::LANGUAGE_LOADER, "manage-font-used_label")
                            } else {
                                fl!(crate::LANGUAGE_LOADER, "manage-font-not_used_label")
                            }
                            .into();
                            let galley = text.into_galley(ui, Some(egui::TextWrapMode::Truncate), f32::INFINITY, font_id);
                            let mut title_rect = rect;
                            title_rect.set_left(title_rect.left() + 480.0);
                            ui.painter().galley_with_override_text_color(
                                egui::Align2::LEFT_TOP.align_size_within_rect(galley.size(), title_rect.shrink(4.0)).min,
                                galley,
                                text_color,
                            );

                            if response.clicked() {
                                self.selected = *i;
                            }
                        }
                    });
                });
                TopBottomPanel::bottom("font_manager_bottom_panel")
                    .exact_height(24.0)
                    .resizable(false)
                    .show_inside(ui, |ui| {
                        ui.add_space(8.0);

                        ui.horizontal(|ui| {
                            ui.label(fl!(crate::LANGUAGE_LOADER, "manage-font-replace_label"));
                            let mut tmp_str = self.replace_with.to_string();
                            ui.add(egui::TextEdit::singleline(&mut tmp_str).desired_width(80.0).char_limit(4));
                            if let Ok(new_width) = tmp_str.parse::<usize>() {
                                self.replace_with = new_width;
                            } else {
                                self.replace_with = 0;
                            }
                            let enabled = self.buffer_view.lock().get_buffer().get_font(self.replace_with).is_some();
                            if ui
                                .add_enabled(enabled, Button::new(fl!(crate::LANGUAGE_LOADER, "manage-font-replace_font_button")))
                                .clicked()
                            {
                                if let Err(err) = self
                                    .buffer_view
                                    .lock()
                                    .get_edit_state_mut()
                                    .replace_font_usage(self.selected, self.replace_with)
                                {
                                    log::error!("Error replacing font {}->{}: {err}", self.selected, self.replace_with);
                                }
                                self.update_used_fonts();
                                self.selected = 0;
                                self.replace_with = 0;
                                self.do_select = true;
                            }

                            if ui
                                .add_enabled(!enabled, Button::new(fl!(crate::LANGUAGE_LOADER, "manage-font-change_font_slot_button")))
                                .clicked()
                            {
                                if let Err(err) = self.buffer_view.lock().get_edit_state_mut().change_font_slot(self.selected, self.replace_with) {
                                    log::error!("Error change font {}->{}: {err}", self.selected, self.replace_with);
                                }
                                self.update_used_fonts();
                                self.selected = 0;
                                self.replace_with = 0;
                                self.do_select = true;
                            }
                        });
                    });
            });
            ui.add_space(8.0);
            modal.buttons(ui, |ui| {
                if ui.button(fl!(crate::LANGUAGE_LOADER, "new-file-ok")).clicked() {
                    result = true;
                }

                let copy_font_button = ui.add(Button::new(fl!(crate::LANGUAGE_LOADER, "manage-font-copy_font_button")));
                let copy_font_button = copy_font_button.on_hover_ui(|ui| {
                    ui.label(fl!(crate::LANGUAGE_LOADER, "manage-font-copy_font_button-tooltip"));
                });

                if copy_font_button.clicked() {
                    let lock = &self.buffer_view.lock();
                    if let Some(font) = lock.get_buffer().get_font(self.selected) {
                        ui.output_mut(|o| o.copied_text = font.encode_as_ansi(self.selected));
                    }
                }
                let remove_font_button = &ui.add_enabled(self.selected > 0, Button::new(fl!(crate::LANGUAGE_LOADER, "manage-font-remove_font_button")));
                if remove_font_button.clicked() {
                    if let Err(err) = self.buffer_view.lock().get_edit_state_mut().remove_font(self.selected) {
                        log::error!("Error removing font {}: {err}", self.selected);
                    }
                    self.update_used_fonts();
                    self.replace_with = 0;
                    self.do_select = true;
                }
            });
        });
        modal.open();
        result
    }

    fn should_commit(&self) -> bool {
        self.do_select
    }

    fn commit(&self, _editor: &mut AnsiEditor) -> TerminalResult<Option<Message>> {
        Ok(None)
    }
}
