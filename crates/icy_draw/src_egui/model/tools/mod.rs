pub mod brush_imp;
pub mod click_imp;
pub mod draw_ellipse_filled_imp;
pub mod draw_ellipse_imp;
pub mod draw_rectangle_filled_imp;
pub mod draw_rectangle_imp;
pub mod erase_imp;
pub mod fill_imp;
pub mod flip_imp;
pub mod font_imp;
pub mod line_imp;
pub mod move_layer_imp;
pub mod paste_tool;
pub mod pencil_imp;
pub mod pipette_imp;
pub mod select_imp;
pub mod tag_imp;

mod icons;

use std::sync::Arc;

use eframe::egui::{self, Response};
use egui::mutex::Mutex;
use icy_engine::Position;
use icy_engine_gui::TerminalCalc;

use crate::{AnsiEditor, Document, Event, Message};

#[derive(Copy, Clone, Debug)]
pub enum MKey {
    Character(u16),
    Down,
    Up,
    Left,
    Right,
    PageDown,
    PageUp,
    Home,
    End,
    Return,
    Delete,
    Insert,
    Backspace,
    Tab,
    Escape,
    F1,
    F2,
    F3,
    F4,
    F5,
    F6,
    F7,
    F8,
    F9,
    F10,
    F11,
    F12,
}

#[derive(Copy, Clone, Debug)]
pub enum MModifiers {
    None,
    Shift,
    Alt,
    Control,
}

impl MModifiers {
    pub fn is_shift(self) -> bool {
        matches!(self, MModifiers::Shift)
    }

    pub fn is_alt(self) -> bool {
        matches!(self, MModifiers::Alt)
    }

    pub fn is_control(self) -> bool {
        matches!(self, MModifiers::Control)
    }
}

#[derive(Default, Clone, Copy)]
pub struct DragPos {
    pub start_abs: Position,
    pub cur_abs: Position,
    pub start: Position,
    pub cur: Position,

    pub start_half_block: Position,
}

pub trait Tool {
    fn get_icon(&self) -> &egui::Image<'static>;

    fn tool_name(&self) -> String;

    fn tooltip(&self) -> String;

    fn use_caret(&self, _editor: &AnsiEditor) -> bool {
        true
    }

    fn is_visible(&self) -> bool {
        true
    }

    fn is_exclusive(&self) -> bool {
        false
    }

    fn use_selection(&self) -> bool {
        true
    }

    fn has_context_menu(&self) -> bool {
        false
    }

    fn show_ui(&mut self, ctx: &egui::Context, ui: &mut egui::Ui, editor_opt: Option<&mut AnsiEditor>) -> Option<Message>;

    fn show_doc_ui(&mut self, _ctx: &egui::Context, _ui: &mut egui::Ui, _doc: Arc<Mutex<Box<dyn Document>>>) -> Option<Message> {
        None
    }

    fn handle_key(&mut self, _editor: &mut AnsiEditor, _key: MKey, _modifier: MModifiers) -> Event {
        Event::None
    }

    fn handle_click(&mut self, _editor: &mut AnsiEditor, _button: i32, _pos: Position, _pos_abs: Position, _response: &Response) -> Option<Message> {
        None
    }

    fn handle_drag_begin(&mut self, _editor: &mut AnsiEditor, _response: &egui::Response) -> Event {
        Event::None
    }

    fn handle_drag(&mut self, _ui: &egui::Ui, response: Response, _editor: &mut AnsiEditor, _calc: &TerminalCalc) -> Response {
        response
    }

    fn handle_hover(&mut self, _ui: &egui::Ui, response: Response, _editor: &mut AnsiEditor, _cur: Position, _cur_abs: Position) -> Response {
        response
    }

    fn handle_no_hover(&mut self, _editor: &mut AnsiEditor) {}

    fn handle_drag_end(&mut self, _editor: &mut AnsiEditor) -> Option<Message> {
        None
    }

    fn draw_shape(&mut self, editor: &mut AnsiEditor, p2: Position, flip_colors: bool) {
        editor.clear_overlay_layer();
        let attr = editor.get_caret_attribute();
        if flip_colors {
            let mut flipped = attr;
            let tmp = flipped.foreground();
            flipped.set_foreground(flipped.background());
            flipped.set_background(tmp);
            editor.set_caret_attribute(flipped);
        }
        self.render_shape(editor, p2);
        if flip_colors {
            editor.set_caret_attribute(attr);
        }
    }

    fn render_shape(&mut self, _editor: &mut AnsiEditor, _p2: Position) {}
}

pub mod tool {
    use super::{MKey, MModifiers, Tool};
    use crate::{AnsiEditor, DragMode, Event};

    pub(crate) fn handle_key(editor: &mut AnsiEditor, key: MKey, _modifier: MModifiers) -> Event {
        match key {
            MKey::Escape => {
                editor.clear_overlay_layer();
                editor.drag_started = crate::DragMode::Off;
            }
            _ => {}
        }
        Event::None
    }

    pub(crate) fn handle_drag_end(editor: &mut AnsiEditor, fl: String) -> Option<crate::Message> {
        editor.join_overlay(fl);
        None
    }

    pub(crate) fn handle_drag(tool: &mut dyn Tool, editor: &mut AnsiEditor) {
        let p2 = editor.half_block_click_pos;
        tool.draw_shape(editor, p2, matches!(editor.drag_started, DragMode::Secondary));
    }

    pub(crate) fn handle_drag_begin(tool: &mut dyn Tool, editor: &mut AnsiEditor) -> Event {
        let p2 = editor.half_block_click_pos;
        tool.draw_shape(editor, p2, matches!(editor.drag_started, DragMode::Secondary));
        Event::None
    }

    pub(crate) fn handle_click(tool: &mut dyn Tool, editor: &mut AnsiEditor, button: i32, fl: String) -> Option<crate::Message> {
        let p2 = editor.half_block_click_pos;
        tool.draw_shape(editor, p2, button != 1);
        editor.join_overlay(fl);
        None
    }
}
