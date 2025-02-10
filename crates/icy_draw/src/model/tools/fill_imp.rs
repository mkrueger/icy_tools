use std::{cell::RefCell, collections::HashSet, rc::Rc};

use eframe::egui;
use i18n_embed_fl::fl;
use icy_engine::{paint::HalfBlockInfo, AttributedChar, Size, TextAttribute, TextPane};

use crate::{
    paint::{BrushMode, ColorMode},
    AnsiEditor, Message,
};

use super::{Position, Tool};

pub struct FillTool {
    color_mode: ColorMode,

    char_code: std::rc::Rc<std::cell::RefCell<char>>,
    fill_type: BrushMode,
    use_exact_matching: bool,
}

impl FillTool {
    pub fn new() -> Self {
        let c = Rc::new(RefCell::new('\u{00B0}'));
        Self {
            color_mode: ColorMode::Both,
            char_code: c.clone(),
            fill_type: BrushMode::HalfBlock,
            use_exact_matching: false,
        }
    }
}

#[allow(clippy::struct_excessive_bools)]
struct FillOperation {
    fill_type: BrushMode,
    color_mode: ColorMode,
    use_exact_matching: bool,

    size: Size,
    pub offset: Position,
    use_selection: bool,
    base_char: AttributedChar,
    new_char: AttributedChar,
    visited: HashSet<Position>,
}

impl FillOperation {
    pub fn new(fill_tool: &FillTool, editor: &AnsiEditor, base_char: AttributedChar, new_ch: AttributedChar) -> Self {
        let lock = &editor.buffer_view.lock();
        let state = lock.get_edit_state();
        let size = state.get_cur_layer().unwrap().get_size();
        let use_selection = state.is_something_selected();
        let offset = if let Some(layer) = state.get_cur_layer() {
            layer.get_offset()
        } else {
            Position::default()
        };

        Self {
            size,
            color_mode: fill_tool.color_mode,
            fill_type: fill_tool.fill_type.clone(),
            use_selection,
            base_char,
            offset,
            new_char: new_ch,
            use_exact_matching: fill_tool.use_exact_matching,
            visited: HashSet::new(),
        }
    }

    pub fn fill(&mut self, editor: &mut AnsiEditor, pos: Position) {
        let mut pos_stack = vec![pos];

        while let Some(pos) = pos_stack.pop() {
            if pos.x < 0 || pos.y < 0 || pos.x >= self.size.width || pos.y >= self.size.height || !self.visited.insert(pos) {
                continue;
            }

            if !self.use_selection || editor.buffer_view.lock().get_edit_state().get_is_selected(pos + self.offset) {
                let cur_char = editor.buffer_view.lock().get_edit_state().get_cur_layer().unwrap().get_char(pos);

                let mut repl_ch = cur_char;

                match &self.fill_type {
                    BrushMode::Char(_) => {
                        if self.use_exact_matching && cur_char != self.base_char || !self.use_exact_matching && cur_char.ch != self.base_char.ch {
                            continue;
                        }
                        repl_ch.ch = self.new_char.ch;
                        repl_ch.set_font_page(self.new_char.get_font_page());
                    }
                    BrushMode::Colorize => {
                        if self.use_exact_matching && cur_char != self.base_char || !self.use_exact_matching && cur_char.attribute != self.base_char.attribute {
                            continue;
                        }
                    }
                    _ => {}
                }
                if self.color_mode.use_fore() {
                    repl_ch.attribute.set_foreground(self.new_char.attribute.get_foreground());
                    repl_ch.attribute.set_is_bold(self.new_char.attribute.is_bold());
                }

                if self.color_mode.use_back() {
                    repl_ch.attribute.set_background(self.new_char.attribute.get_background());
                }

                repl_ch.set_font_page(editor.buffer_view.lock().get_caret().get_attribute().get_font_page());
                repl_ch.attribute.attr &= !icy_engine::attribute::INVISIBLE;
                editor.set_char(pos, repl_ch);
            }

            pos_stack.push(pos + Position::new(-1, 0));
            pos_stack.push(pos + Position::new(1, 0));
            pos_stack.push(pos + Position::new(0, -1));
            pos_stack.push(pos + Position::new(0, 1));
        }
    }
}

// Fill with
// Attribute, Fore/Back
// Character
// Both
impl Tool for FillTool {
    fn get_icon(&self) -> &egui::Image<'static> {
        &super::icons::FILL_SVG
    }

    fn tool_name(&self) -> String {
        fl!(crate::LANGUAGE_LOADER, "tool-fill_name")
    }

    fn tooltip(&self) -> String {
        fl!(crate::LANGUAGE_LOADER, "tool-fill_tooltip")
    }

    fn use_caret(&self, _editor: &AnsiEditor) -> bool {
        false
    }
    fn show_ui(&mut self, _ctx: &egui::Context, ui: &mut egui::Ui, editor_opt: Option<&mut AnsiEditor>) -> Option<Message> {
        self.color_mode.show_ui(ui);

        ui.checkbox(&mut self.use_exact_matching, fl!(crate::LANGUAGE_LOADER, "tool-fill-exact_match_label"));

        self.fill_type.show_ui(ui, editor_opt, self.char_code.clone(), crate::paint::BrushUi::Fill)
    }

    fn handle_hover(&mut self, _ui: &egui::Ui, response: egui::Response, _editor: &mut AnsiEditor, _cur: Position, _cur_abs: Position) -> egui::Response {
        response.on_hover_cursor(egui::CursorIcon::Crosshair)
    }

    fn handle_click(&mut self, editor: &mut AnsiEditor, button: i32, pos: Position, _pos_abs: Position, _response: &egui::Response) -> Option<Message> {
        let Ok(layer) = editor.get_cur_layer_index() else { return None };
        if layer >= editor.buffer_view.lock().get_buffer().layers.len() {
            return None;
        }
        let ch = if let Some(layer) = editor.buffer_view.lock().get_edit_state().get_cur_layer() {
            layer.get_char(pos)
        } else {
            return None;
        };

        let mut attr = editor.get_caret_attribute();
        let flip_colors = button == 2;
        if flip_colors {
            let tmp = attr.get_foreground();
            attr.set_foreground(attr.get_background());
            attr.set_background(tmp);
        }

        match self.fill_type {
            BrushMode::HalfBlock => {
                let _undo = editor.begin_atomic_undo(fl!(crate::LANGUAGE_LOADER, "undo-bucket-fill"));
                let mut op = FillHalfBlockOperation::new(self, editor, attr.get_foreground());
                op.fill(editor, pos);
            }
            BrushMode::Char(_) => {
                let _undo = editor.begin_atomic_undo(fl!(crate::LANGUAGE_LOADER, "undo-bucket-fill"));
                let mut op = FillOperation::new(self, editor, ch, AttributedChar::new(*self.char_code.borrow(), attr));
                op.fill(editor, pos);
            }
            _ => {
                if self.color_mode.use_fore() || self.color_mode.use_back() {
                    let _undo = editor.begin_atomic_undo(fl!(crate::LANGUAGE_LOADER, "undo-bucket-fill"));
                    let mut op = FillOperation::new(self, editor, ch, AttributedChar::new(*self.char_code.borrow(), attr));
                    op.fill(editor, pos);
                }
            }
        }

        None
    }
}

#[allow(clippy::struct_excessive_bools)]
struct FillHalfBlockOperation {
    color: u32,
    size: Size,
    pub offset: Position,
    use_selection: bool,
    visited: HashSet<Position>,
}

impl FillHalfBlockOperation {
    pub fn new(_fill_tool: &FillTool, editor: &AnsiEditor, color: u32) -> Self {
        let lock = &editor.buffer_view.lock();
        let state = lock.get_edit_state();
        let size = state.get_cur_layer().unwrap().get_size();
        let use_selection = state.is_something_selected();
        let offset = if let Some(layer) = state.get_cur_layer() {
            layer.get_offset()
        } else {
            Position::default()
        };
        Self {
            color,
            size,
            use_selection,
            offset,
            visited: HashSet::new(),
        }
    }

    pub fn fill(&mut self, editor: &mut AnsiEditor, mut pos: Position) {
        pos.y *= 2;

        let mut pos_stack = vec![(pos, pos)];
        let cur_layer = editor.buffer_view.lock().get_edit_state().get_current_layer().unwrap();
        let block = HalfBlockInfo::from(&editor.buffer_view.lock().get_edit_state_mut().get_buffer_mut().layers[cur_layer], pos);
        if !block.is_blocky() {
            return;
        }
        let target_color = if block.is_top { block.upper_block_color } else { block.lower_block_color };
        if target_color == self.color {
            return;
        }

        while let Some((from, to)) = pos_stack.pop() {
            let text_pos = Position::new(to.x, to.y / 2);
            if to.x < 0 || to.y < 0 || to.x >= self.size.width || text_pos.y >= self.size.height || !self.visited.insert(to) {
                continue;
            }
            if self.use_selection && !editor.buffer_view.lock().get_edit_state().get_is_selected(text_pos + self.offset) {
                continue;
            }
            let block = HalfBlockInfo::from(&editor.buffer_view.lock().get_edit_state_mut().get_buffer_mut().layers[cur_layer], to);
            if block.is_blocky() && (block.is_top && block.upper_block_color == target_color) || (!block.is_top && block.lower_block_color == target_color) {
                let ch = block.get_half_block_char(self.color);
                let _ = editor.buffer_view.lock().get_edit_state_mut().set_char(text_pos, ch);

                pos_stack.push((to, to + Position::new(-1, 0)));
                pos_stack.push((to, to + Position::new(1, 0)));
                pos_stack.push((to, to + Position::new(0, -1)));
                pos_stack.push((to, to + Position::new(0, 1)));
            } else if block.is_vertically_blocky() {
                let ch = if from.y == to.y - 1 && block.left_block_color == target_color {
                    Some(AttributedChar::new(221 as char, TextAttribute::new(self.color, block.right_block_color)))
                } else if from.y == to.y - 1 && block.right_block_color == target_color {
                    Some(AttributedChar::new(222 as char, TextAttribute::new(self.color, block.left_block_color)))
                } else if from.y == to.y + 1 && block.right_block_color == target_color {
                    Some(AttributedChar::new(222 as char, TextAttribute::new(self.color, block.left_block_color)))
                } else if from.y == to.y + 1 && block.left_block_color == target_color {
                    Some(AttributedChar::new(221 as char, TextAttribute::new(self.color, block.right_block_color)))
                } else if from.x == to.x - 1 && block.left_block_color == target_color {
                    Some(AttributedChar::new(221 as char, TextAttribute::new(self.color, block.right_block_color)))
                } else if from.x == to.x + 1 && block.right_block_color == target_color {
                    Some(AttributedChar::new(222 as char, TextAttribute::new(self.color, block.left_block_color)))
                } else {
                    None
                };
                if let Some(ch) = ch {
                    let _ = editor.buffer_view.lock().get_edit_state_mut().set_char(text_pos, ch);
                }
            }
        }
    }
}
