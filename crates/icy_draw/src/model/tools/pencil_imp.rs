use std::{cell::RefCell, rc::Rc};

use eframe::egui::{self};
use i18n_embed_fl::fl;
use icy_engine::{editor::AtomicUndoGuard, TextAttribute};
use icy_engine_gui::TerminalCalc;

use crate::{
    paint::{plot_point, BrushMode, ColorMode, PointRole},
    AnsiEditor, Event, Message,
};

use super::{Position, Tool};

pub struct PencilTool {
    char_code: std::rc::Rc<std::cell::RefCell<char>>,
    undo_op: Option<AtomicUndoGuard>,
    draw_mode: BrushMode,
    color_mode: ColorMode,
    pub _attr: TextAttribute,

    last_pos: Position,
    cur_pos: Position,
}

impl Default for PencilTool {
    fn default() -> Self {
        Self {
            undo_op: None,
            draw_mode: BrushMode::HalfBlock,
            color_mode: ColorMode::Both,
            char_code: Rc::new(RefCell::new('\u{00B0}')),
            last_pos: Position::default(),
            cur_pos: Position::default(),
            _attr: icy_engine::TextAttribute::default(),
        }
    }
}

impl Tool for PencilTool {
    fn get_icon(&self) -> &egui::Image<'static> {
        &super::icons::PENCIL_SVG
    }

    fn tool_name(&self) -> String {
        fl!(crate::LANGUAGE_LOADER, "tool-pencil_name")
    }

    fn tooltip(&self) -> String {
        fl!(crate::LANGUAGE_LOADER, "tool-pencil_tooltip")
    }

    fn use_caret(&self, _editor: &AnsiEditor) -> bool {
        false
    }

    fn show_ui(&mut self, _ctx: &egui::Context, ui: &mut egui::Ui, editor_opt: Option<&mut AnsiEditor>) -> Option<Message> {
        self.color_mode.show_ui(ui);
        self.draw_mode
            .show_ui(ui, editor_opt, self.char_code.clone(), crate::paint::BrushUi::HideOutline)
    }

    fn handle_click(&mut self, editor: &mut AnsiEditor, button: i32, pos: Position, _pos_abs: Position, _response: &egui::Response) -> Option<Message> {
        self.last_pos = pos;
        let _op: AtomicUndoGuard = editor.begin_atomic_undo(fl!(crate::LANGUAGE_LOADER, "undo-pencil"));
        editor.clear_overlay_layer();
        let flip_colors = button == 2;
        let attr = editor.get_caret_attribute();
        if flip_colors {
            let mut flipped = attr;
            let tmp = flipped.get_foreground();
            flipped.set_foreground(flipped.get_background());
            flipped.set_background(tmp);
            editor.set_caret_attribute(flipped);
        }
        plot_point(
            &mut editor.buffer_view.lock(),
            editor.half_block_click_pos,
            self.draw_mode.clone(),
            self.color_mode,
            PointRole::Line,
        );
        editor.join_overlay(fl!(crate::LANGUAGE_LOADER, "undo-pencil"));
        if flip_colors {
            editor.set_caret_attribute(attr);
        }
        None
    }
    fn handle_hover(&mut self, _ui: &egui::Ui, response: egui::Response, _editor: &mut AnsiEditor, cur: Position, _cur_abs: Position) -> egui::Response {
        self.cur_pos = cur;
        response.on_hover_cursor(egui::CursorIcon::Crosshair)
    }

    fn handle_drag(&mut self, _ui: &egui::Ui, response: egui::Response, editor: &mut AnsiEditor, _calc: &TerminalCalc) -> egui::Response {
        if self.last_pos == editor.half_block_click_pos {
            return response;
        }
        let flip_colors = matches!(editor.drag_started, crate::DragMode::Secondary);
        let attr = editor.get_caret_attribute();
        if flip_colors {
            let mut flipped = attr;
            let tmp = flipped.get_foreground();
            flipped.set_foreground(flipped.get_background());
            flipped.set_background(tmp);
            editor.set_caret_attribute(flipped);
        }
        plot_point(
            &mut editor.buffer_view.lock(),
            editor.half_block_click_pos,
            self.draw_mode.clone(),
            self.color_mode,
            PointRole::Line,
        );
        if flip_colors {
            editor.set_caret_attribute(attr);
        }
        self.last_pos = editor.half_block_click_pos;
        self.cur_pos = editor.drag_pos.cur;
        editor.buffer_view.lock().get_edit_state_mut().set_is_buffer_dirty();

        response
    }

    fn handle_drag_begin(&mut self, editor: &mut AnsiEditor, _response: &egui::Response) -> Event {
        self.undo_op = Some(editor.begin_atomic_undo(fl!(crate::LANGUAGE_LOADER, "undo-pencil")));
        self.last_pos = editor.half_block_click_pos;
        self.cur_pos = editor.drag_pos.cur;
        editor.clear_overlay_layer();
        let flip_colors = matches!(editor.drag_started, crate::DragMode::Secondary);
        let attr = editor.get_caret_attribute();
        if flip_colors {
            let mut flipped = attr;
            let tmp = flipped.get_foreground();
            flipped.set_foreground(flipped.get_background());
            flipped.set_background(tmp);
            editor.set_caret_attribute(flipped);
        }
        plot_point(
            &mut editor.buffer_view.lock(),
            editor.half_block_click_pos,
            self.draw_mode.clone(),
            self.color_mode,
            PointRole::Line,
        );
        if flip_colors {
            editor.set_caret_attribute(attr);
        }
        editor.buffer_view.lock().get_edit_state_mut().set_is_buffer_dirty();
        Event::None
    }

    fn handle_drag_end(&mut self, editor: &mut AnsiEditor) -> Option<Message> {
        editor.join_overlay(fl!(crate::LANGUAGE_LOADER, "undo-pencil"));
        self.undo_op = None;
        None
    }

    fn get_toolbar_location_text(&self, _editor: &AnsiEditor) -> String {
        let pos = self.cur_pos;
        fl!(crate::LANGUAGE_LOADER, "toolbar-position", line = (pos.y + 1), column = (pos.x + 1))
    }
}
