use eframe::egui;
use i18n_embed_fl::fl;
use icy_engine_gui::TerminalCalc;

use crate::{
    paint::{draw_line, BrushMode, ColorMode},
    AnsiEditor, Event, Message,
};

use super::{Position, Tool};

pub struct LineTool {
    draw_mode: BrushMode,
    color_mode: ColorMode,

    pub char_code: std::rc::Rc<std::cell::RefCell<char>>,

    pub old_pos: Position,
}

impl Default for LineTool {
    fn default() -> Self {
        Self {
            draw_mode: BrushMode::HalfBlock,
            color_mode: crate::paint::ColorMode::Both,
            char_code: std::rc::Rc::new(std::cell::RefCell::new('\u{00B0}')),
            old_pos: Position::default(),
        }
    }
}

// block tools:
// copy/moxe
// fill, delete
impl Tool for LineTool {
    fn get_icon(&self) -> &egui::Image<'static> {
        &super::icons::LINE_SVG
    }

    fn tool_name(&self) -> String {
        fl!(crate::LANGUAGE_LOADER, "tool-line_name")
    }

    fn tooltip(&self) -> String {
        fl!(crate::LANGUAGE_LOADER, "tool-line_tooltip")
    }

    fn use_caret(&self, _editor: &AnsiEditor) -> bool {
        false
    }

    fn use_selection(&self) -> bool {
        false
    }

    fn show_ui(&mut self, _ctx: &egui::Context, ui: &mut egui::Ui, editor_opt: Option<&mut AnsiEditor>) -> Option<Message> {
        self.color_mode.show_ui(ui);
        self.draw_mode
            .show_ui(ui, editor_opt, self.char_code.clone(), crate::paint::BrushUi::HideOutline)
    }

    fn handle_click(&mut self, editor: &mut AnsiEditor, button: i32, pos: Position, _pos_abs: Position, _response: &egui::Response) -> Option<Message> {
        if button == 1 {
            editor.set_caret_position(pos);
        }
        None
    }

    fn handle_hover(&mut self, _ui: &egui::Ui, response: egui::Response, _editor: &mut AnsiEditor, _cur: Position, _cur_abs: Position) -> egui::Response {
        response.on_hover_cursor(egui::CursorIcon::Crosshair)
    }

    fn handle_drag_begin(&mut self, _editor: &mut AnsiEditor, _response: &egui::Response) -> Event {
        self.old_pos = Position::new(-1, -1);
        Event::None
    }

    fn handle_drag(&mut self, _ui: &egui::Ui, response: egui::Response, editor: &mut AnsiEditor, _calc: &TerminalCalc) -> egui::Response {
        let p2 = editor.half_block_click_pos;
        if self.old_pos == p2 {
            return response;
        }
        self.old_pos = p2;

        editor.clear_overlay_layer();
        draw_line(
            &mut editor.buffer_view.lock(),
            editor.drag_pos.start_half_block,
            p2,
            self.draw_mode.clone(),
            self.color_mode,
        );
        response
    }

    fn handle_drag_end(&mut self, editor: &mut AnsiEditor) -> Option<Message> {
        if editor.drag_pos.start == editor.drag_pos.cur {
            editor.buffer_view.lock().get_buffer_mut().remove_overlay();
        } else {
            editor.join_overlay(fl!(crate::LANGUAGE_LOADER, "undo-line"));
        }
        None
    }
}
