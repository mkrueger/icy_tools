use eframe::{
    emath::Align2,
    epaint::{Color32, FontId, Vec2},
};
use egui::{CornerRadius, Image, TextureHandle, Widget};
use i18n_embed_fl::fl;
use icy_engine::{AttributedChar, Buffer, TextAttribute};

use crate::{create_image, AnsiEditor, Message};

use super::{Position, Tool};

pub static mut CUR_CHAR: Option<AttributedChar> = None;

#[derive(Default)]
pub struct PipetteTool {
    ch: Option<char>,
    char_image: Option<TextureHandle>,
    cur_pos: Option<Position>,
    take_fg: bool,
    take_bg: bool,
}

impl Tool for PipetteTool {
    fn get_icon(&self) -> &egui::Image<'static> {
        &super::icons::DROPPER_SVG
    }

    fn tool_name(&self) -> String {
        fl!(crate::LANGUAGE_LOADER, "tool-pipette_name")
    }

    fn tooltip(&self) -> String {
        fl!(crate::LANGUAGE_LOADER, "tool-pipette_tooltip")
    }

    fn use_caret(&self, _editor: &AnsiEditor) -> bool {
        false
    }

    fn use_selection(&self) -> bool {
        false
    }

    fn show_ui(&mut self, ctx: &egui::Context, ui: &mut egui::Ui, editor_opt: Option<&mut AnsiEditor>) -> Option<Message> {
        let Some(editor) = editor_opt else {
            return None;
        };

        if let Some(ch) = unsafe { CUR_CHAR } {
            ui.vertical_centered(|ui| {
                ui.label(fl!(crate::LANGUAGE_LOADER, "pipette_tool_char_code", code = (ch.ch as u32)));

                if self.ch.is_none() || !self.ch.unwrap().eq(&ch.ch) {
                    self.ch = Some(ch.ch);

                    let mut buf = Buffer::new((1, 1));
                    buf.clear_font_table();
                    if let Some(font) = editor.buffer_view.lock().get_buffer().get_font(ch.get_font_page()) {
                        buf.set_font(0, font.clone());
                    } else {
                        log::error!("Pipette tool: font page {} not found", ch.get_font_page());
                    }
                    buf.layers[0].set_char((0, 0), AttributedChar::new(ch.ch, TextAttribute::default()));
                    self.char_image = Some(create_image(ctx, &buf));
                }

                if let Some(image) = &self.char_image {
                    let image = Image::from_texture(image).fit_to_original_size(2.0);
                    image.ui(ui);
                }

                self.take_fg = !ui.input(|i| i.modifiers.ctrl) || ui.input(|i| i.modifiers.shift);
                if self.take_fg {
                    ui.label(fl!(crate::LANGUAGE_LOADER, "pipette_tool_foreground", fg = ch.attribute.get_foreground()));
                    paint_color(ui, &editor.buffer_view.lock().get_buffer().palette.get_color(ch.attribute.get_foreground()));
                }

                self.take_bg = !ui.input(|i| i.modifiers.shift) || ui.input(|i| i.modifiers.ctrl);

                if self.take_bg {
                    ui.label(fl!(crate::LANGUAGE_LOADER, "pipette_tool_background", bg = ch.attribute.get_background()));
                    paint_color(ui, &editor.buffer_view.lock().get_buffer().palette.get_color(ch.attribute.get_background()));
                }
            });

            ui.add_space(4.0);
            ui.horizontal(|ui| {
                ui.add_space(8.0);
                ui.label(fl!(crate::LANGUAGE_LOADER, "pipette_tool_keys"));
            });
        }
        None
    }

    fn handle_hover(&mut self, _ui: &egui::Ui, response: egui::Response, editor: &mut AnsiEditor, cur: Position, cur_abs: Position) -> egui::Response {
        unsafe {
            CUR_CHAR = Some(editor.get_char(cur_abs));
        }
        if self.cur_pos != Some(cur) {
            self.cur_pos = Some(cur);
            let lock = &mut editor.buffer_view.lock();
            let get_tool_overlay_mask_mut = lock.get_edit_state_mut().get_tool_overlay_mask_mut();
            get_tool_overlay_mask_mut.clear();
            get_tool_overlay_mask_mut.set_is_selected(cur_abs, true);
            lock.get_edit_state_mut().set_is_buffer_dirty();
        }

        response.on_hover_cursor(egui::CursorIcon::Crosshair)
    }

    fn handle_no_hover(&mut self, editor: &mut AnsiEditor) {
        unsafe {
            CUR_CHAR = None;
        }
        self.cur_pos = None;

        let lock: &mut eframe::epaint::mutex::MutexGuard<'_, icy_engine_gui::BufferView> = &mut editor.buffer_view.lock();
        let get_edit_state_mut = lock.get_edit_state_mut();
        if !get_edit_state_mut.get_tool_overlay_mask_mut().is_empty() {
            get_edit_state_mut.get_tool_overlay_mask_mut().clear();
            get_edit_state_mut.set_is_buffer_dirty();
        }
    }

    fn handle_click(&mut self, editor: &mut AnsiEditor, button: i32, _pos: Position, _pos_abs: Position, _response: &egui::Response) -> Option<Message> {
        if button == 1 {
            unsafe {
                if let Some(ch) = CUR_CHAR {
                    let mut attr = editor.buffer_view.lock().get_caret_mut().get_attribute();
                    if self.take_fg {
                        attr.set_foreground(ch.attribute.get_foreground());
                    }

                    if self.take_bg {
                        attr.set_background(ch.attribute.get_background());
                    }
                    editor.set_caret_attribute(attr);
                }
            }
            return Some(Message::SelectPreviousTool);
        }
        None
    }
}

fn paint_color(ui: &mut egui::Ui, color: &icy_engine::Color) {
    let (_, stroke_rect) = ui.allocate_space(Vec2::new(100.0, 32.0));

    let painter = ui.painter_at(stroke_rect);

    let (r, g, b) = color.get_rgb();
    painter.rect_filled(stroke_rect, CornerRadius::ZERO, Color32::BLACK);
    painter.rect_filled(stroke_rect.shrink(1.0), CornerRadius::ZERO, Color32::WHITE);
    let color = Color32::from_rgb(r, g, b);
    painter.rect_filled(stroke_rect.shrink(2.0), CornerRadius::ZERO, color);

    let text_color = if (r as f32 * 0.299 + g as f32 * 0.587 + b as f32 * 0.114) > 186.0 {
        Color32::BLACK
    } else {
        Color32::WHITE
    };

    let text = format!("#{r:02x}{g:02x}{b:02x}");
    let font_id: eframe::epaint::FontId = FontId::monospace(16.0);
    painter.text(stroke_rect.center(), Align2::CENTER_CENTER, text, font_id, text_color);
}
