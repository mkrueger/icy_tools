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

mod icons;

use std::sync::Arc;

use eframe::egui::{self, Response};
use egui::mutex::Mutex;
use i18n_embed_fl::fl;
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

    fn get_toolbar_location_text(&self, editor: &AnsiEditor) -> String {
        toolbar_pos_sel_text(editor, true)
    }
}

fn toolbar_pos_sel_text(editor: &AnsiEditor, show_selection: bool) -> String {
    let pos = editor.get_caret_position();
    let sel = if show_selection { editor.buffer_view.lock().get_selection() } else { None };

    if let Some(sel) = sel {
        let r = sel.as_rectangle();
        fl!(crate::LANGUAGE_LOADER, "toolbar-size", colums = r.size.width, rows = r.size.height)
    } else {
        fl!(crate::LANGUAGE_LOADER, "toolbar-position", line = (pos.y + 1), column = (pos.x + 1))
    }
}
