use std::sync::Arc;

use eframe::egui;
use egui::mutex::Mutex;
use i18n_embed_fl::fl;
use icy_engine::{editor::AtomicUndoGuard, AddType, Rectangle, TextPane};
use icy_engine_gui::TerminalCalc;

use crate::{model::MKey, AnsiEditor, CharTableToolWindow, Document, Message};

use super::{Event, MModifiers, Position, Tool};

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

#[derive(Default)]
pub struct ClickTool {
    start_selection: Rectangle,
    selection_drag: SelectionDrag,
    undo_op: Option<AtomicUndoGuard>,
    char_table: Option<CharTableToolWindow>,
}

pub const VALID_OUTLINE_CHARS: &str = "ABCDEFGHIJKLMNO@&\u{F7} ";

impl Tool for ClickTool {
    fn get_icon(&self) -> &'static egui::Image<'static> {
        &super::icons::TEXT_SVG
    }

    fn tool_name(&self) -> String {
        fl!(crate::LANGUAGE_LOADER, "tool-click_name")
    }

    fn tooltip(&self) -> String {
        fl!(crate::LANGUAGE_LOADER, "tool-click_tooltip")
    }

    fn use_caret(&self, editor: &AnsiEditor) -> bool {
        let is_selected = editor.buffer_view.lock().get_edit_state().is_something_selected();
        !is_selected
    }

    fn has_context_menu(&self) -> bool {
        true
    }

    fn show_ui(&mut self, ctx: &egui::Context, ui: &mut egui::Ui, editor_opt: Option<&mut AnsiEditor>) -> Option<Message> {
        if self.char_table.is_none() {
            self.char_table = Some(CharTableToolWindow::new(ctx, 16));
        }
        let mut msg = None;
        if let Some(editor) = editor_opt {
            editor.color_mode.show_ui(ui);

            ui.vertical(|ui| {
                ui.set_height(16.0 * 256.0 * 2.0);
                msg = self.char_table.as_mut().unwrap().show_char_table(ui, editor);
            });
        }
        msg
    }

    fn show_doc_ui(&mut self, ctx: &egui::Context, ui: &mut egui::Ui, doc: Arc<Mutex<Box<dyn Document>>>) -> Option<Message> {
        if !doc.lock().can_paste_char() {
            return None;
        }
        if self.char_table.is_none() {
            self.char_table = Some(CharTableToolWindow::new(ctx, 16));
        }
        ui.vertical(|ui| {
            ui.set_height(16.0 * 256.0 * 2.0);
            let ch: Option<char> = self.char_table.as_mut().unwrap().show_plain_char_table(ui);

            if let Some(ch) = ch {
                doc.lock().paste_char(ui, ch);
            }
        });

        None
    }

    fn handle_click(&mut self, editor: &mut AnsiEditor, button: i32, pos: Position, cur_abs: Position, _response: &egui::Response) -> Option<Message> {
        if button == 1 && !is_inside_selection(editor, cur_abs) {
            editor.set_caret_position(pos);
            editor.buffer_view.lock().clear_selection();
        }
        None
    }

    fn handle_drag_begin(&mut self, editor: &mut AnsiEditor, _response: &egui::Response) -> Event {
        self.selection_drag = get_selection_drag(editor, editor.drag_pos.start_abs);

        if !matches!(self.selection_drag, SelectionDrag::None) {
            if let Some(selection) = editor.buffer_view.lock().get_selection() {
                self.start_selection = selection.as_rectangle();
            }
        }
        self.undo_op = Some(editor.begin_atomic_undo(fl!(crate::LANGUAGE_LOADER, "undo-select")));

        Event::None
    }
    fn handle_drag(&mut self, _ui: &egui::Ui, response: egui::Response, editor: &mut AnsiEditor, _calc: &TerminalCalc) -> egui::Response {
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
                    editor.buffer_view.lock().clear_selection();
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
        match get_selection_drag(editor, cur_abs) {
            SelectionDrag::None => ui.output_mut(|o| o.cursor_icon = egui::CursorIcon::Text),
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
            editor.buffer_view.lock().clear_selection();
        }
        self.undo_op = None;

        None
    }

    fn handle_key(&mut self, editor: &mut AnsiEditor, key: MKey, modifier: MModifiers) -> Event {
        // TODO Keys:
        // ctrl+pgup  - upper left corner
        // ctrl+pgdn  - lower left corner
        let pos = editor.buffer_view.lock().get_caret().get_position();
        match key {
            MKey::Down => {
                if matches!(modifier, MModifiers::None) {
                    editor.set_caret(pos.x, pos.y + 1);
                }
            }
            MKey::Up => {
                if matches!(modifier, MModifiers::None) {
                    editor.set_caret(pos.x, pos.y - 1);
                }
            }
            MKey::Left => {
                if matches!(modifier, MModifiers::None) {
                    editor.set_caret(pos.x - 1, pos.y);
                }
            }
            MKey::Right => {
                if matches!(modifier, MModifiers::None) {
                    editor.set_caret(pos.x + 1, pos.y);
                }
            }
            MKey::PageDown => {
                let height = editor.buffer_view.lock().calc.terminal_rect.height();
                let char_height = editor.buffer_view.lock().calc.char_size.y;
                let pg_size = (height / char_height) as i32;
                editor.set_caret(pos.x, pos.y + pg_size);
            }
            MKey::PageUp => {
                let height = editor.buffer_view.lock().calc.terminal_rect.height();
                let char_height = editor.buffer_view.lock().calc.char_size.y;
                let pg_size = (height / char_height) as i32;
                editor.set_caret(pos.x, pos.y - pg_size);
            }

            MKey::Escape => {
                editor.buffer_view.lock().clear_selection();
            }

            MKey::Tab => {
                let tab_size = 8;
                if let MModifiers::Shift = modifier {
                    let tabs = ((pos.x / tab_size) - 1).max(0);
                    let next_tab = tabs * tab_size;
                    editor.set_caret(next_tab, pos.y);
                } else {
                    let tabs = 1 + pos.x / tab_size;
                    let next_tab = (editor.buffer_view.lock().get_buffer().get_width() - 1).min(tabs * tab_size);
                    editor.set_caret(next_tab, pos.y);
                }
            }

            MKey::Return => {
                editor.set_caret(0, pos.y + 1);
            }
            MKey::Insert => {
                let insert_mode = editor.buffer_view.lock().get_caret().insert_mode;
                editor.buffer_view.lock().get_caret_mut().insert_mode = !insert_mode;
            }
            MKey::Backspace => {
                editor.backspace();
            }

            MKey::Delete => {
                if editor.buffer_view.lock().get_selection().is_none() {
                    editor.delete();
                }
            }

            MKey::Home => {
                let mut pos = editor.get_caret_position();
                pos.x = 0;

                if let MModifiers::Control = modifier {
                    pos.y = 0;
                }
                editor.set_caret(pos.x, pos.y);
            }
            MKey::End => {
                let mut pos = editor.get_caret_position();
                pos.x = i32::MAX;
                if let MModifiers::Control = modifier {
                    pos.y = i32::MAX;
                }
                editor.set_caret(pos.x, pos.y);
            }

            MKey::Character(ch) => {
                let typed_char = unsafe { char::from_u32_unchecked(ch as u32) };
                if editor.outline_font_mode {
                    let typed_char = typed_char.to_ascii_uppercase();
                    if VALID_OUTLINE_CHARS.contains(typed_char) {
                        editor.type_key(typed_char);
                    } else if let '1'..='8' = typed_char {
                        editor.type_key(VALID_OUTLINE_CHARS.chars().nth(10 + typed_char as usize - b'1' as usize).unwrap());
                    }
                } else {
                    editor.type_key(typed_char);
                }
            }

            MKey::F1 => {
                if matches!(modifier, MModifiers::None) {
                    if editor.outline_font_mode {
                        editor.type_key(VALID_OUTLINE_CHARS.chars().next().unwrap());
                    } else {
                        editor.type_char_set_key(0);
                    }
                }
            }
            MKey::F2 => {
                if matches!(modifier, MModifiers::None) {
                    if editor.outline_font_mode {
                        editor.type_key(VALID_OUTLINE_CHARS.chars().nth(1).unwrap());
                    } else {
                        editor.type_char_set_key(1);
                    }
                }
            }
            MKey::F3 => {
                if matches!(modifier, MModifiers::None) {
                    if editor.outline_font_mode {
                        editor.type_key(VALID_OUTLINE_CHARS.chars().nth(2).unwrap());
                    } else {
                        editor.type_char_set_key(2);
                    }
                }
            }
            MKey::F4 => {
                if matches!(modifier, MModifiers::None) {
                    if editor.outline_font_mode {
                        editor.type_key(VALID_OUTLINE_CHARS.chars().nth(3).unwrap());
                    } else {
                        editor.type_char_set_key(3);
                    }
                }
            }
            MKey::F5 => {
                if matches!(modifier, MModifiers::None) {
                    if editor.outline_font_mode {
                        editor.type_key(VALID_OUTLINE_CHARS.chars().nth(4).unwrap());
                    } else {
                        editor.type_char_set_key(4);
                    }
                }
            }
            MKey::F6 => {
                if matches!(modifier, MModifiers::None) {
                    if editor.outline_font_mode {
                        editor.type_key(VALID_OUTLINE_CHARS.chars().nth(5).unwrap());
                    } else {
                        editor.type_char_set_key(5);
                    }
                }
            }
            MKey::F7 => {
                if matches!(modifier, MModifiers::None) {
                    if editor.outline_font_mode {
                        editor.type_key(VALID_OUTLINE_CHARS.chars().nth(6).unwrap());
                    } else {
                        editor.type_char_set_key(6);
                    }
                }
            }
            MKey::F8 => {
                if matches!(modifier, MModifiers::None) {
                    if editor.outline_font_mode {
                        editor.type_key(VALID_OUTLINE_CHARS.chars().nth(7).unwrap());
                    } else {
                        editor.type_char_set_key(7);
                    }
                }
            }
            MKey::F9 => {
                if matches!(modifier, MModifiers::None) {
                    if editor.outline_font_mode {
                        editor.type_key(VALID_OUTLINE_CHARS.chars().nth(8).unwrap());
                    } else {
                        editor.type_char_set_key(8);
                    }
                }
            }
            MKey::F10 => {
                if matches!(modifier, MModifiers::None) {
                    if editor.outline_font_mode {
                        editor.type_key(VALID_OUTLINE_CHARS.chars().nth(9).unwrap());
                    } else {
                        editor.type_char_set_key(9);
                    }
                }
            }
            _ => {}
        }
        Event::None
    }
}

impl ClickTool {
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
