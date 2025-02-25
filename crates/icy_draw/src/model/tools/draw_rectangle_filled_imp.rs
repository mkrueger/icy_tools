use eframe::egui;
use i18n_embed_fl::fl;
use icy_engine::Position;
use icy_engine_gui::TerminalCalc;

use crate::{
    AnsiEditor, Event, Message,
    paint::{BrushMode, ColorMode, fill_rectangle},
};

use super::Tool;

pub struct DrawRectangleFilledTool {
    draw_mode: BrushMode,
    color_mode: ColorMode,
    char_code: std::rc::Rc<std::cell::RefCell<char>>,
}

impl Default for DrawRectangleFilledTool {
    fn default() -> Self {
        Self {
            draw_mode: BrushMode::HalfBlock,
            color_mode: crate::paint::ColorMode::Both,
            char_code: std::rc::Rc::new(std::cell::RefCell::new('\u{00B0}')),
        }
    }
}

impl Tool for DrawRectangleFilledTool {
    fn get_icon(&self) -> &egui::Image<'static> {
        &super::icons::RECTANGLE_FILLED_SVG
    }

    fn tool_name(&self) -> String {
        fl!(crate::LANGUAGE_LOADER, "tool-filled_rectangle_name")
    }

    fn tooltip(&self) -> String {
        fl!(crate::LANGUAGE_LOADER, "tool-filled_rectangle_tooltip")
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

    fn handle_hover(&mut self, _ui: &egui::Ui, response: egui::Response, _editor: &mut AnsiEditor, _cur: Position, _cur_abs: Position) -> egui::Response {
        response.on_hover_cursor(egui::CursorIcon::Crosshair)
    }

    fn handle_click(&mut self, editor: &mut AnsiEditor, button: i32, _pos: Position, _pos_abs: Position, _response: &egui::Response) -> Option<Message> {
        super::tool::handle_click(self, editor, button, fl!(crate::LANGUAGE_LOADER, "undo-draw-rectangle"))
    }

    fn handle_drag_begin(&mut self, editor: &mut AnsiEditor, _response: &egui::Response) -> Event {
        super::tool::handle_drag_begin(self, editor)
    }

    fn handle_drag(&mut self, _ui: &egui::Ui, response: egui::Response, editor: &mut AnsiEditor, _calc: &TerminalCalc) -> egui::Response {
        super::tool::handle_drag(self, editor);
        response
    }

    fn handle_drag_end(&mut self, editor: &mut AnsiEditor) -> Option<Message> {
        super::tool::handle_drag_end(editor, fl!(crate::LANGUAGE_LOADER, "undo-draw-rectangle"))
    }

    fn handle_key(&mut self, editor: &mut AnsiEditor, key: super::MKey, modifier: super::MModifiers) -> Event {
        super::tool::handle_key(editor, key, modifier)
    }

    fn render_shape(&mut self, editor: &mut AnsiEditor, p2: Position) {
        editor.clear_overlay_layer();
        let p1 = editor.drag_pos.start_half_block;
        let start = Position::new(p1.x.min(p2.x), p1.y.min(p2.y));
        let end = Position::new(p1.x.max(p2.x), p1.y.max(p2.y));
        fill_rectangle(&mut editor.buffer_view.lock(), start, end, self.draw_mode.clone(), self.color_mode);
    }
}
