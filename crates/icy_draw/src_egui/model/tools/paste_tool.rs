use super::{Event, MKey, Position, Tool};
use crate::{AnsiEditor, Message, to_message};
use eframe::egui::{self, Key};
use i18n_embed_fl::fl;
use icy_engine_gui::TerminalCalc;

#[derive(Default)]
pub struct PasteTool {
    start_offset: Position,
    drag_started: bool,
    drag_offset: Position,
    last_tool: usize,
    closed: bool,
}

impl PasteTool {
    pub(crate) fn new(selected_tool: usize) -> Self {
        Self {
            last_tool: selected_tool,
            ..Default::default()
        }
    }
}

impl Tool for PasteTool {
    fn get_icon(&self) -> &egui::Image<'static> {
        &super::icons::MOVE_SVG
    }

    fn tool_name(&self) -> String {
        String::new()
    }

    fn tooltip(&self) -> String {
        String::new()
    }

    fn use_caret(&self, _editor: &AnsiEditor) -> bool {
        false
    }

    fn is_visible(&self) -> bool {
        false
    }

    fn is_exclusive(&self) -> bool {
        true
    }

    fn use_selection(&self) -> bool {
        false
    }

    fn show_ui(&mut self, ctx: &egui::Context, ui: &mut egui::Ui, editor_opt: Option<&mut AnsiEditor>) -> Option<Message> {
        let mut result = None;
        if let Some(editor) = editor_opt {
            if let Some(layer) = editor.buffer_view.lock().get_edit_state().get_cur_layer() {
                self.closed = !layer.role.is_paste();
            }
        } else {
            self.closed = true;
        }

        if self.closed {
            return Some(Message::SelectTool(self.last_tool));
        }

        ui.label(fl!(crate::LANGUAGE_LOADER, "paste_mode-description"));
        ui.add_space(8.0);
        egui::Grid::new("paste_mode_grid").num_columns(2).spacing([4.0, 4.0]).show(ui, |ui| {
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                ui.strong("S -");
            });
            if ui.button(fl!(crate::LANGUAGE_LOADER, "paste_mode-stamp")).clicked() || ui.input(|i| i.key_pressed(Key::S)) {
                result = Some(Message::StampLayerDown);
            }
            ui.end_row();
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                ui.strong("R -");
            });
            if ui.button(fl!(crate::LANGUAGE_LOADER, "paste_mode-rotate")).clicked() || ui.input(|i| i.key_pressed(Key::R)) {
                result = Some(Message::RotateLayer);
            }
            ui.end_row();
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                ui.strong("X -");
            });
            if ui.button(fl!(crate::LANGUAGE_LOADER, "paste_mode-flipx")).clicked() || ui.input(|i| i.key_pressed(Key::X)) {
                result = Some(Message::FlipX);
            }
            ui.end_row();
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                ui.strong("Y -");
            });
            if ui.button(fl!(crate::LANGUAGE_LOADER, "paste_mode-flipy")).clicked() || ui.input(|i| i.key_pressed(Key::Y)) {
                result = Some(Message::FlipY);
            }
            ui.end_row();
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                ui.strong("T -");
            });
            if ui.button(fl!(crate::LANGUAGE_LOADER, "paste_mode-transparent")).clicked() || ui.input(|i| i.key_pressed(Key::T)) {
                result = Some(Message::MakeLayerTransparent);
            }
            ui.end_row();
        });

        if ctx.input(|i| i.key_pressed(Key::Escape)) {
            return Some(Message::RemoveFloatingLayer);
        }
        result
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
