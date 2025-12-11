use super::{Event, MKey, Position, Tool};
use crate::{AnsiEditor, Message, to_message};
use eframe::egui;
use i18n_embed_fl::fl;
use icy_engine_gui::TerminalCalc;

#[derive(Default)]
pub struct MoveLayer {
    start_offset: Position,
    drag_started: bool,
    drag_offset: Position,
}

impl MoveLayer {}

impl Tool for MoveLayer {
    fn get_icon(&self) -> &egui::Image<'static> {
        &super::icons::MOVE_SVG
    }

    fn tool_name(&self) -> String {
        fl!(crate::LANGUAGE_LOADER, "tool-move_layer_name")
    }

    fn tooltip(&self) -> String {
        fl!(crate::LANGUAGE_LOADER, "tool-move_layer_tooltip")
    }

    fn use_caret(&self, _editor: &AnsiEditor) -> bool {
        false
    }
    fn use_selection(&self) -> bool {
        false
    }
    fn show_ui(&mut self, _ctx: &egui::Context, _ui: &mut egui::Ui, _editor_opt: Option<&mut AnsiEditor>) -> Option<Message> {
        None
    }

    fn handle_drag_begin(&mut self, editor: &mut AnsiEditor, _response: &egui::Response) -> Event {
        self.drag_started = false;

        if let Some(layer) = editor.buffer_view.lock().get_edit_state_mut().get_cur_layer_mut() {
            self.start_offset = layer.offset();
            self.drag_offset = self.start_offset;
            self.drag_started = true;
        }
        Event::None
    }

    fn handle_drag(&mut self, _ui: &egui::Ui, response: egui::Response, editor: &mut AnsiEditor, _calc: &TerminalCalc) -> egui::Response {
        if !self.drag_started {
            return response;
        }
        if let Some(layer) = editor.buffer_view.lock().get_edit_state_mut().get_cur_layer_mut() {
            self.drag_offset = self.start_offset + editor.drag_pos.cur_abs - editor.drag_pos.start_abs;
            layer.set_preview_offset(Some(self.drag_offset));
        }
        response.on_hover_cursor(egui::CursorIcon::Grabbing)
    }

    fn handle_hover(&mut self, _ui: &egui::Ui, response: egui::Response, _editor: &mut AnsiEditor, _cur: Position, _cur_abs: Position) -> egui::Response {
        response.on_hover_cursor(egui::CursorIcon::Move)
    }

    fn handle_drag_end(&mut self, editor: &mut AnsiEditor) -> Option<Message> {
        if !self.drag_started {
            return None;
        }
        to_message(editor.buffer_view.lock().get_edit_state_mut().move_layer(self.drag_offset))
    }

    fn handle_key(&mut self, editor: &mut AnsiEditor, key: MKey, modifier: super::MModifiers) -> Event {
        let offset = if let Some(layer) = editor.buffer_view.lock().get_edit_state_mut().get_cur_layer_mut() {
            layer.offset()
        } else {
            return Event::None;
        };
        let i = if matches!(modifier, super::MModifiers::Shift) { 2 } else { 1 };

        match key {
            MKey::Down => {
                let _ = editor.buffer_view.lock().get_edit_state_mut().move_layer(offset + Position::new(0, i));
            }
            MKey::Up => {
                let _ = editor.buffer_view.lock().get_edit_state_mut().move_layer(offset - Position::new(0, i));
            }
            MKey::Left => {
                let _ = editor.buffer_view.lock().get_edit_state_mut().move_layer(offset - Position::new(i, 0));
            }
            MKey::Right => {
                let _ = editor.buffer_view.lock().get_edit_state_mut().move_layer(offset + Position::new(i, 0));
            }
            _ => {}
        }
        Event::None
    }
}
