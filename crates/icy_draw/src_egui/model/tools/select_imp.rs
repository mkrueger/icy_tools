use eframe::egui;
use i18n_embed_fl::fl;
use icy_engine::{AddType, Rectangle, editor::AtomicUndoGuard};
use icy_engine_gui::TerminalCalc;

use crate::{AnsiEditor, Message, to_message};

use super::{Event, Position, Tool};

#[derive(Default)]
enum SelectionDrag {
    #[default]
    None,
    Move,
    Left,
    Right,
    Top,
    Bottom,

    TopLeft,
    TopRight,
    BottomLeft,
    BottomRight,
}

#[derive(Default, PartialEq, Copy, Clone)]
enum SelectionMode {
    #[default]
    Normal,
    Character,
    Attribute,
    Foreground,
    Background,
}
enum SelectionModifier {
    Replace,
    Add,
    Remove,
}
impl SelectionModifier {
    fn get_response(&self, ch: bool) -> Option<bool> {
        match self {
            SelectionModifier::Replace => Some(ch),
            SelectionModifier::Add => {
                if ch {
                    Some(true)
                } else {
                    None
                }
            }
            SelectionModifier::Remove => {
                if ch {
                    Some(false)
                } else {
                    None
                }
            }
        }
    }
}

#[derive(Default)]
pub struct SelectTool {
    start_selection: Rectangle,
    selection_drag: SelectionDrag,
    mode: SelectionMode,
    undo_op: Option<AtomicUndoGuard>,
}

impl Tool for SelectTool {
    fn get_icon(&self) -> &egui::Image<'static> {
        &super::icons::SELECT_SVG
    }

    fn tool_name(&self) -> String {
        fl!(crate::LANGUAGE_LOADER, "tool-select_name")
    }

    fn tooltip(&self) -> String {
        fl!(crate::LANGUAGE_LOADER, "tool-select_tooltip")
    }

    fn use_caret(&self, _editor: &AnsiEditor) -> bool {
        false
    }

    fn show_ui(&mut self, _ctx: &egui::Context, ui: &mut egui::Ui, _editor_opt: Option<&mut AnsiEditor>) -> Option<Message> {
        ui.label(fl!(crate::LANGUAGE_LOADER, "tool-select-label"));
        ui.radio_value(&mut self.mode, SelectionMode::Normal, fl!(crate::LANGUAGE_LOADER, "tool-select-normal"));
        ui.radio_value(&mut self.mode, SelectionMode::Character, fl!(crate::LANGUAGE_LOADER, "tool-select-character"));
        ui.radio_value(&mut self.mode, SelectionMode::Attribute, fl!(crate::LANGUAGE_LOADER, "tool-select-attribute"));
        ui.radio_value(&mut self.mode, SelectionMode::Foreground, fl!(crate::LANGUAGE_LOADER, "tool-select-foreground"));

        ui.radio_value(&mut self.mode, SelectionMode::Background, fl!(crate::LANGUAGE_LOADER, "tool-select-background"));
        ui.add_space(8.0);
        ui.vertical_centered(|ui| {
            ui.small(fl!(crate::LANGUAGE_LOADER, "tool-select-description"));
        });

        None
    }
    fn has_context_menu(&self) -> bool {
        true
    }

    fn handle_click(&mut self, editor: &mut AnsiEditor, button: i32, pos: Position, cur_abs: Position, response: &egui::Response) -> Option<Message> {
        let cur_ch = editor.get_char_from_cur_layer(pos);

        let selection_mode = if response.ctx.input(|i| i.modifiers.shift_only()) {
            SelectionModifier::Add
        } else if response.ctx.input(|i| i.modifiers.command_only()) {
            SelectionModifier::Remove
        } else {
            SelectionModifier::Replace
        };
        match self.mode {
            SelectionMode::Normal => {
                if button == 1 && !is_inside_selection(editor, cur_abs) {
                    let lock = &mut editor.buffer_view.lock();
                    let _ = lock.get_edit_state_mut().add_selection_to_mask();
                    let _ = lock.get_edit_state_mut().deselect();
                }
            }
            SelectionMode::Character => editor
                .buffer_view
                .lock()
                .get_edit_state_mut()
                .enumerate_selections(|_, ch, _| selection_mode.get_response(ch.ch == cur_ch.ch)),
            SelectionMode::Attribute => editor
                .buffer_view
                .lock()
                .get_edit_state_mut()
                .enumerate_selections(|_, ch, _| selection_mode.get_response(ch.attribute == cur_ch.attribute)),
            SelectionMode::Foreground => editor
                .buffer_view
                .lock()
                .get_edit_state_mut()
                .enumerate_selections(|_, ch, _| selection_mode.get_response(ch.attribute.get_foreground() == cur_ch.attribute.get_foreground())),
            SelectionMode::Background => editor
                .buffer_view
                .lock()
                .get_edit_state_mut()
                .enumerate_selections(|_, ch, _| selection_mode.get_response(ch.attribute.get_background() == cur_ch.attribute.get_background())),
        }
        None
    }

    fn handle_drag_begin(&mut self, editor: &mut AnsiEditor, response: &egui::Response) -> Event {
        self.undo_op = Some(editor.begin_atomic_undo(fl!(crate::LANGUAGE_LOADER, "undo-select")));
        if self.mode != SelectionMode::Normal {
            return Event::None;
        }

        self.selection_drag = get_selection_drag(editor, editor.drag_pos.start_abs);
        if !matches!(self.selection_drag, SelectionDrag::None) {
            if let Some(selection) = editor.buffer_view.lock().get_selection() {
                self.start_selection = selection.as_rectangle();
            }
        } else if !response.ctx.input(|i| i.modifiers.shift_only() || i.modifiers.command_only()) {
            let _ = editor.buffer_view.lock().get_edit_state_mut().clear_selection();
        }
        Event::None
    }

    fn handle_drag(&mut self, _ui: &egui::Ui, response: egui::Response, editor: &mut AnsiEditor, _calc: &TerminalCalc) -> egui::Response {
        if self.mode != SelectionMode::Normal {
            return response;
        }
        let mut rect = if let Some(selection) = editor.buffer_view.lock().get_selection() {
            selection.as_rectangle()
        } else {
            Rectangle::from_coords(0, 0, 0, 0)
        };

        match self.selection_drag {
            SelectionDrag::Move => {
                rect.start = self.start_selection.top_left() - editor.drag_pos.start_abs + editor.drag_pos.cur_abs;
                editor.buffer_view.lock().set_selection(rect);
            }
            SelectionDrag::Left => {
                self.move_left(editor, &mut rect);
                editor.buffer_view.lock().set_selection(rect);
            }
            SelectionDrag::Right => {
                self.move_right(editor, &mut rect);
                editor.buffer_view.lock().set_selection(rect);
            }
            SelectionDrag::Top => {
                self.move_top(editor, &mut rect);
                editor.buffer_view.lock().set_selection(rect);
            }
            SelectionDrag::Bottom => {
                self.move_bottom(editor, &mut rect);
                editor.buffer_view.lock().set_selection(rect);
            }
            SelectionDrag::TopLeft => {
                self.move_left(editor, &mut rect);
                self.move_top(editor, &mut rect);
                editor.buffer_view.lock().set_selection(rect);
            }
            SelectionDrag::TopRight => {
                self.move_right(editor, &mut rect);
                self.move_top(editor, &mut rect);
                editor.buffer_view.lock().set_selection(rect);
            }
            SelectionDrag::BottomLeft => {
                self.move_left(editor, &mut rect);
                self.move_bottom(editor, &mut rect);
                editor.buffer_view.lock().set_selection(rect);
            }
            SelectionDrag::BottomRight => {
                self.move_right(editor, &mut rect);
                self.move_bottom(editor, &mut rect);
                editor.buffer_view.lock().set_selection(rect);
            }

            SelectionDrag::None => {
                if editor.drag_pos.start == editor.drag_pos.cur {
                    let _ = editor.buffer_view.lock().get_edit_state_mut().deselect();
                } else {
                    editor.buffer_view.lock().set_selection(Rectangle::from(
                        editor.drag_pos.start_abs.x.min(editor.drag_pos.cur_abs.x),
                        editor.drag_pos.start_abs.y.min(editor.drag_pos.cur_abs.y),
                        (editor.drag_pos.cur_abs.x - editor.drag_pos.start_abs.x).abs(),
                        (editor.drag_pos.cur_abs.y - editor.drag_pos.start_abs.y).abs(),
                    ));
                }
            }
        }

        let lock = &mut editor.buffer_view.lock();
        if let Some(mut selection) = lock.get_selection() {
            if response.ctx.input(|i| i.modifiers.command_only()) {
                selection.add_type = AddType::Subtract;
            }
            if response.ctx.input(|i| i.modifiers.shift_only()) {
                selection.add_type = AddType::Add;
            }
            lock.set_selection(selection);
        }
        response
    }

    fn handle_hover(&mut self, ui: &egui::Ui, response: egui::Response, editor: &mut AnsiEditor, _cur: Position, cur_abs: Position) -> egui::Response {
        if self.mode != SelectionMode::Normal {
            return response.on_hover_cursor(egui::CursorIcon::Crosshair);
        }

        match get_selection_drag(editor, cur_abs) {
            SelectionDrag::None => ui.output_mut(|o| o.cursor_icon = egui::CursorIcon::Crosshair),
            SelectionDrag::Move => ui.output_mut(|o| o.cursor_icon = egui::CursorIcon::Move),
            SelectionDrag::Left => ui.output_mut(|o| o.cursor_icon = egui::CursorIcon::ResizeWest),
            SelectionDrag::Right => ui.output_mut(|o| o.cursor_icon = egui::CursorIcon::ResizeEast),
            SelectionDrag::Top => ui.output_mut(|o| o.cursor_icon = egui::CursorIcon::ResizeNorth),
            SelectionDrag::Bottom => {
                ui.output_mut(|o| o.cursor_icon = egui::CursorIcon::ResizeSouth);
            }
            SelectionDrag::TopLeft => {
                ui.output_mut(|o| o.cursor_icon = egui::CursorIcon::ResizeNorthWest);
            }
            SelectionDrag::TopRight => {
                ui.output_mut(|o| o.cursor_icon = egui::CursorIcon::ResizeNorthEast);
            }
            SelectionDrag::BottomLeft => {
                ui.output_mut(|o| o.cursor_icon = egui::CursorIcon::ResizeSouthWest);
            }
            SelectionDrag::BottomRight => {
                ui.output_mut(|o| o.cursor_icon = egui::CursorIcon::ResizeSouthEast);
            }
        }
        response
    }

    fn handle_drag_end(&mut self, editor: &mut AnsiEditor) -> Option<Message> {
        if self.mode != SelectionMode::Normal {
            self.undo_op = None;
            return None;
        }

        if !matches!(self.selection_drag, SelectionDrag::None) {
            self.selection_drag = SelectionDrag::None;
            self.undo_op = None;
            return None;
        }

        let mut cur = editor.drag_pos.cur;
        if editor.drag_pos.start < cur {
            cur += Position::new(1, 1);
        }

        if editor.drag_pos.start == cur {
            let _ = editor.buffer_view.lock().get_edit_state_mut().deselect();
        }

        let lock = &mut editor.buffer_view.lock();
        self.undo_op = None;

        to_message(lock.get_edit_state_mut().add_selection_to_mask())
    }
}

impl SelectTool {
    fn move_left(&mut self, editor: &AnsiEditor, rect: &mut Rectangle) {
        let delta = editor.drag_pos.start_abs.x - editor.drag_pos.cur_abs.x;
        rect.start.x = self.start_selection.left() - delta;
        rect.size.width = self.start_selection.get_width() + delta;

        if rect.size.width < 0 {
            rect.size.width = rect.start.x - self.start_selection.right();
            rect.start.x = self.start_selection.right();
        }
    }

    fn move_right(&mut self, editor: &AnsiEditor, rect: &mut Rectangle) {
        rect.size.width = self.start_selection.get_width() - editor.drag_pos.start_abs.x + editor.drag_pos.cur_abs.x;
        if rect.size.width < 0 {
            rect.start.x = self.start_selection.left() + rect.size.width;
            rect.size.width = self.start_selection.left() - rect.start.x;
        }
    }

    fn move_top(&mut self, editor: &AnsiEditor, rect: &mut Rectangle) {
        let delta = editor.drag_pos.start_abs.y - editor.drag_pos.cur_abs.y;
        rect.start.y = self.start_selection.top() - delta;
        rect.size.height = self.start_selection.get_height() + delta;

        if rect.size.height < 0 {
            rect.size.height = rect.start.y - self.start_selection.bottom();
            rect.start.y = self.start_selection.bottom();
        }
    }

    fn move_bottom(&mut self, editor: &AnsiEditor, rect: &mut Rectangle) {
        rect.size.height = self.start_selection.get_height() - editor.drag_pos.start_abs.y + editor.drag_pos.cur_abs.y;
        if rect.size.height < 0 {
            rect.start.y = self.start_selection.top() + rect.size.height;
            rect.size.height = self.start_selection.top() - rect.start.y;
        }
    }
}

fn is_inside_selection(editor: &AnsiEditor, cur_abs: Position) -> bool {
    if let Some(selection) = editor.buffer_view.lock().get_selection() {
        return selection.is_inside(cur_abs);
    }
    false
}

fn get_selection_drag(editor: &AnsiEditor, cur_abs: Position) -> SelectionDrag {
    if let Some(selection) = editor.buffer_view.lock().get_selection() {
        let rect = selection.as_rectangle();

        if rect.is_inside(cur_abs) {
            let left = cur_abs.x - rect.left() < 2;
            let top = cur_abs.y - rect.top() < 2;
            let right = rect.right() - cur_abs.x < 2;
            let bottom = rect.bottom() - cur_abs.y < 2;

            if left && top {
                return SelectionDrag::TopLeft;
            }

            if right && top {
                return SelectionDrag::TopRight;
            }
            if left && bottom {
                return SelectionDrag::BottomLeft;
            }

            if right && bottom {
                return SelectionDrag::BottomRight;
            }

            if left {
                return SelectionDrag::Left;
            }
            if right {
                return SelectionDrag::Right;
            }

            if top {
                return SelectionDrag::Top;
            }
            if bottom {
                return SelectionDrag::Bottom;
            }

            return SelectionDrag::Move;
        }
    }
    SelectionDrag::None
}
