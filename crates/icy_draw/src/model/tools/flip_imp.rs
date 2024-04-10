use eframe::egui;
use i18n_embed_fl::fl;
use icy_engine::attribute;

use crate::{paint::ColorMode, AnsiEditor, Message};

use super::{Position, Tool};
pub struct FlipTool {
    flip_horizontal: bool,
    cur_pos: Position,
    color_mode: ColorMode,
}

impl Default for FlipTool {
    fn default() -> Self {
        Self {
            flip_horizontal: false,
            cur_pos: Position::new(-1, -1),
            color_mode: ColorMode::None,
        }
    }
}

impl Tool for FlipTool {
    fn get_icon(&self) -> &egui::Image<'static> {
        &super::icons::FLIP_TOOL_SVG
    }

    fn tool_name(&self) -> String {
        fl!(crate::LANGUAGE_LOADER, "tool-flip_name")
    }

    fn tooltip(&self) -> String {
        fl!(crate::LANGUAGE_LOADER, "tool-flip_tooltip")
    }

    fn use_caret(&self, _editor: &AnsiEditor) -> bool {
        false
    }

    fn use_selection(&self) -> bool {
        false
    }

    fn show_ui(&mut self, _ctx: &egui::Context, ui: &mut egui::Ui, _editor_opt: Option<&mut AnsiEditor>) -> Option<Message> {
        self.color_mode.show_ui(ui);

        ui.radio_value(&mut self.flip_horizontal, true, fl!(crate::LANGUAGE_LOADER, "tool-flip_horizontal"));
        ui.radio_value(&mut self.flip_horizontal, false, fl!(crate::LANGUAGE_LOADER, "tool-flip_vertical"));

        None
    }

    fn handle_no_hover(&mut self, editor: &mut AnsiEditor) {
        self.cur_pos = Position::new(-1, -1);

        let lock: &mut eframe::epaint::mutex::MutexGuard<'_, icy_engine_gui::BufferView> = &mut editor.buffer_view.lock();
        let get_edit_state_mut = lock.get_edit_state_mut();
        if !get_edit_state_mut.get_tool_overlay_mask_mut().is_empty() {
            get_edit_state_mut.get_tool_overlay_mask_mut().clear();
            get_edit_state_mut.set_is_buffer_dirty();
        }
    }

    fn handle_hover(&mut self, _ui: &egui::Ui, response: egui::Response, editor: &mut AnsiEditor, _cur: Position, cur_abs: Position) -> egui::Response {
        if self.cur_pos != cur_abs {
            self.cur_pos = cur_abs;
            let lock = &mut editor.buffer_view.lock();
            let get_tool_overlay_mask_mut = lock.get_edit_state_mut().get_tool_overlay_mask_mut();
            get_tool_overlay_mask_mut.clear();
            get_tool_overlay_mask_mut.set_is_selected(cur_abs, true);
            lock.get_edit_state_mut().set_is_buffer_dirty();
        }

        response.on_hover_cursor(egui::CursorIcon::Crosshair)
    }

    fn handle_click(&mut self, editor: &mut AnsiEditor, button: i32, pos: Position, _pos_abs: Position, _response: &egui::Response) -> Option<Message> {
        if button == 1 {
            let mut ch = editor.get_char_from_cur_layer(pos);
            ch.attribute.attr &= !attribute::INVISIBLE;
            if self.color_mode.use_fore() {
                ch.attribute
                    .set_foreground(editor.buffer_view.lock().get_caret().get_attribute().get_foreground());
            }
            if self.color_mode.use_back() {
                ch.attribute
                    .set_background(editor.buffer_view.lock().get_caret().get_attribute().get_background());
            }
            if self.flip_horizontal {
                if ch.ch as u8 == 223 {
                    ch.ch = '\u{00DC}';
                } else if ch.ch as u8 == 220 {
                    ch.ch = '\u{00DB}';
                } else {
                    ch.ch = '\u{00DF}';
                }
            } else {
                // vertical
                if ch.ch as u8 == 222 {
                    ch.ch = '\u{00DD}';
                } else if ch.ch as u8 == 221 {
                    ch.ch = '\u{00DB}';
                } else {
                    ch.ch = '\u{00DE}';
                }
            }

            editor.set_char(pos, ch);
        }
        None
    }
} //   [176, 177, 178, 219, 223, 220, 221, 222, 254, 250 ],
