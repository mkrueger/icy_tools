//! Outline Click Tool
//!
//! A specialized text editing tool for Outline fonts.
//! Filters keyboard input to only accept valid outline codes:
//! - A-Q: 17 Outline placeholder characters (mapped to box-drawing)
//! - @: Fill marker
//! - &: End marker  
//! - Space: Regular space character
//! - 0xFF: Hard blank (Shift+Control+Space)
//!
//! F-key mapping:
//! - F1-F10 → 'A'-'J' (first 10 outline codes)

use iced::widget::{column, container, row, text};
use iced::{Element, Font};
use icy_engine::{Position, Selection, TextPane};
use icy_engine_gui::terminal::crt_state::{is_command_pressed, is_ctrl_pressed};
use icy_engine_gui::TerminalMessage;

use super::{ToolContext, ToolHandler, ToolId, ToolMessage, ToolResult, ToolViewContext};
use crate::ui::editor::ansi::selection_drag::{compute_dragged_selection, hit_test_selection, DragParameters, SelectionDrag};

/// Valid outline font input codes (17 placeholders A-Q, plus @, &, space)
pub const OUTLINE_VALID_CHARS: &[char] = &[
    'A', 'B', 'C', 'D', 'E', 'F', 'G', 'H', 'I', 'J', // Outline placeholders (F1-F10)
    'K', 'L', 'M', 'N', 'O', 'P', 'Q',        // Additional outline placeholders
    '@',        // Fill marker
    '&',        // End marker
    ' ',        // Space
    '\u{00FF}', // Hard blank (0xFF)
];

/// Outline Click Tool state
#[derive(Default)]
pub struct OutlineClickTool {
    // Selection drag state (shared behavior with other tools)
    selection_drag: SelectionDrag,
    hover_drag: SelectionDrag,
    selection_start_pos: Option<Position>,
    selection_cur_pos: Option<Position>,
    selection_start_rect: Option<icy_engine::Rectangle>,
}

impl OutlineClickTool {
    pub fn new() -> Self {
        Self::default()
    }

    /// Check if a character is valid for outline fonts
    fn is_valid_outline_char(ch: char) -> bool {
        OUTLINE_VALID_CHARS.contains(&ch.to_ascii_uppercase())
    }

    /// Map F-key slot (0-9) to outline character code
    fn fkey_to_outline_char(slot: usize) -> Option<char> {
        match slot {
            0 => Some('A'),
            1 => Some('B'),
            2 => Some('C'),
            3 => Some('D'),
            4 => Some('E'),
            5 => Some('F'),
            6 => Some('G'),
            7 => Some('H'),
            8 => Some('I'),
            9 => Some('J'),
            _ => None,
        }
    }

    fn type_outline_char(&self, ctx: &mut ToolContext, ch: char) -> ToolResult {
        // Outline fonts store raw ASCII codes directly
        if let Err(e) = ctx.state.type_key(ch) {
            log::warn!("Failed to type outline char '{}': {}", ch, e);
            return ToolResult::None;
        }
        ToolResult::Commit(format!("Type outline '{}'", ch))
    }
}

impl ToolHandler for OutlineClickTool {
    fn id(&self) -> ToolId {
        ToolId::Tool(icy_engine_edit::tools::Tool::Click)
    }

    fn as_any(&self) -> &dyn std::any::Any {
        self
    }

    fn as_any_mut(&mut self) -> &mut dyn std::any::Any {
        self
    }

    fn cancel_capture(&mut self) {
        self.selection_drag = SelectionDrag::None;
        self.hover_drag = SelectionDrag::None;
        self.selection_start_pos = None;
        self.selection_cur_pos = None;
        self.selection_start_rect = None;
    }

    fn view_toolbar(&self, _ctx: &ToolViewContext) -> Element<'_, ToolMessage> {
        // Outline cheat sheet (shown as the tool toolbar)
        // Keys: F1-F10 map to A-J; additional placeholders K-Q; @ fill, & end, Space; Ctrl+Shift+Space => 0xFF.

        const OUTLINE_RESULTS: &[&str] = &["═", "─", "│", "║", "╒", "╗", "╓", "┐", "╚", "╜", "└", "╜", "╡", "╟", "SP", "@", "&", "÷"];
        const OUTLINE_KEYS: &[&str] = &[
            "F1", "F2", "F3", "F4", "F5", "F6", "F7", "F8", "F9", "F10", "1", "2", "3", "4", "5", "6", "7", "8",
        ];
        const OUTLINE_CODES: &[&str] = &["A", "B", "C", "D", "E", "F", "G", "H", "I", "J", "K", "L", "M", "N", "O", "@", "&", "÷"];

        let mono = Font::MONOSPACE;

        let key_label = crate::fl!("tdf-editor-cheat_sheet_key");
        let code_label = crate::fl!("tdf-editor-cheat_sheet_code");
        let res_label = crate::fl!("tdf-editor-cheat_sheet_res");

        let mut key_row: Vec<Element<'_, ToolMessage>> = vec![text(format!("{:>6}:", key_label)).size(11).font(mono).into()];
        for k in OUTLINE_KEYS {
            key_row.push(text(format!(" {:>3}", k)).size(11).font(mono).into());
        }

        let mut code_row: Vec<Element<'_, ToolMessage>> = vec![text(format!("{:>6}:", code_label)).size(11).font(mono).into()];
        for c in OUTLINE_CODES {
            code_row.push(text(format!(" {:>3}", c)).size(11).font(mono).into());
        }

        let mut res_row: Vec<Element<'_, ToolMessage>> = vec![text(format!("{:>6}:", res_label)).size(11).font(mono).into()];
        for r in OUTLINE_RESULTS {
            res_row.push(text(format!(" {:>3}", r)).size(11).font(mono).into());
        }

        container(column![row(key_row).spacing(0), row(code_row).spacing(0), row(res_row).spacing(0),].spacing(2))
            .padding(4)
            .into()
    }

    fn handle_terminal_message(&mut self, ctx: &mut ToolContext, msg: &TerminalMessage) -> ToolResult {
        match msg {
            TerminalMessage::Move(evt) => {
                let Some(pos) = evt.text_position else {
                    return ToolResult::None;
                };
                self.hover_drag = hit_test_selection(ctx.state.selection(), pos);
                ToolResult::None
            }

            TerminalMessage::Press(evt) => {
                let Some(pos) = evt.text_position else {
                    return ToolResult::None;
                };

                if evt.button == icy_engine::MouseButton::Left {
                    let current_selection = ctx.state.selection();
                    let hit = hit_test_selection(current_selection, pos);

                    if hit != SelectionDrag::None {
                        self.selection_drag = hit;
                        self.selection_start_rect = current_selection.map(|s| s.as_rectangle());
                    } else {
                        let _ = ctx.state.clear_selection();
                        ctx.state.set_caret_from_document_position(pos);
                        self.selection_drag = SelectionDrag::Create;
                        self.selection_start_rect = None;
                    }

                    self.selection_start_pos = Some(pos);
                    self.selection_cur_pos = Some(pos);
                    self.hover_drag = SelectionDrag::None;

                    ToolResult::StartCapture.and(ToolResult::Redraw)
                } else {
                    ToolResult::None
                }
            }

            TerminalMessage::Drag(evt) => {
                if self.selection_drag != SelectionDrag::None {
                    let Some(pos) = evt.text_position else {
                        return ToolResult::None;
                    };

                    self.selection_cur_pos = Some(pos);

                    let Some(start_pos) = self.selection_start_pos else {
                        return ToolResult::None;
                    };

                    if self.selection_drag == SelectionDrag::Create {
                        let selection = Selection {
                            anchor: start_pos,
                            lead: pos,
                            locked: false,
                            shape: icy_engine::Shape::Rectangle,
                            add_type: icy_engine::AddType::Default,
                        };
                        let _ = ctx.state.set_selection(selection);
                        return ToolResult::Redraw;
                    }

                    let Some(start_rect) = self.selection_start_rect else {
                        return ToolResult::None;
                    };

                    let params = DragParameters {
                        start_rect,
                        start_pos,
                        cur_pos: pos,
                    };

                    if let Some(new_rect) = compute_dragged_selection(self.selection_drag, params) {
                        let mut selection = Selection::from(new_rect);
                        selection.add_type = icy_engine::AddType::Default;
                        let _ = ctx.state.set_selection(selection);
                        return ToolResult::Redraw;
                    }
                }

                ToolResult::None
            }

            TerminalMessage::Release(evt) => {
                if self.selection_drag != SelectionDrag::None {
                    let end_pos = evt.text_position;

                    if self.selection_drag == SelectionDrag::Create {
                        if let (Some(start), Some(end)) = (self.selection_start_pos, end_pos) {
                            if start == end {
                                let _ = ctx.state.clear_selection();
                            }
                        }
                    }

                    self.selection_drag = SelectionDrag::None;
                    self.hover_drag = SelectionDrag::None;
                    self.selection_start_pos = None;
                    self.selection_cur_pos = None;
                    self.selection_start_rect = None;

                    ToolResult::EndCapture.and(ToolResult::Redraw)
                } else {
                    ToolResult::None
                }
            }

            _ => ToolResult::None,
        }
    }

    fn handle_event(&mut self, ctx: &mut ToolContext, event: &iced::Event) -> ToolResult {
        match event {
            iced::Event::Keyboard(iced::keyboard::Event::KeyPressed { key, modifiers, text, .. }) => {
                use iced::keyboard::key::Named;

                // Shift+Control+Space inserts 0xFF (hard blank) - works for all font types
                if modifiers.shift() && modifiers.control() {
                    if let iced::keyboard::Key::Named(Named::Space) = key {
                        return self.type_outline_char(ctx, '\u{00FF}');
                    }
                }

                // Handle character input using translated text - filter for valid outline chars only
                if !modifiers.control() && !modifiers.alt() {
                    if let Some(input_text) = text {
                        if let Some(ch) = input_text.chars().next() {
                            // Skip control characters (0x00-0x1F) and DEL (0x7F) - these should be handled
                            // by Named key handlers (Backspace, Tab, Enter, Delete, etc.)
                            if ch < ' ' || ch == '\x7F' {
                                // Fall through to Named key handling below
                            } else {
                                let upper = ch.to_ascii_uppercase();

                                // Only accept valid outline characters (A-Q, @, &)
                                if Self::is_valid_outline_char(upper) {
                                    return self.type_outline_char(ctx, upper);
                                }

                                // Invalid character - ignore silently
                                return ToolResult::None;
                            }
                        }
                    }
                }

                // Handle Space key (text field may not contain it)
                if let iced::keyboard::Key::Named(Named::Space) = key {
                    if !modifiers.shift() && !modifiers.control() {
                        return self.type_outline_char(ctx, ' ');
                    }
                }

                if let iced::keyboard::Key::Named(named) = key {
                    match named {
                        // F1-F10 mapped to A-J
                        Named::F1 | Named::F2 | Named::F3 | Named::F4 | Named::F5 | Named::F6 | Named::F7 | Named::F8 | Named::F9 | Named::F10 => {
                            let slot = match named {
                                Named::F1 => 0,
                                Named::F2 => 1,
                                Named::F3 => 2,
                                Named::F4 => 3,
                                Named::F5 => 4,
                                Named::F6 => 5,
                                Named::F7 => 6,
                                Named::F8 => 7,
                                Named::F9 => 8,
                                Named::F10 => 9,
                                _ => 0,
                            };

                            if let Some(outline_ch) = Self::fkey_to_outline_char(slot) {
                                return self.type_outline_char(ctx, outline_ch);
                            }
                        }

                        // Navigation keys
                        Named::ArrowUp => {
                            ctx.state.move_caret_up(1);
                            return ToolResult::Redraw;
                        }
                        Named::ArrowDown => {
                            ctx.state.move_caret_down(1);
                            return ToolResult::Redraw;
                        }
                        Named::ArrowLeft => {
                            ctx.state.move_caret_left(1);
                            return ToolResult::Redraw;
                        }
                        Named::ArrowRight => {
                            ctx.state.move_caret_right(1);
                            return ToolResult::Redraw;
                        }
                        Named::Home => {
                            ctx.state.set_caret_x(0);
                            return ToolResult::Redraw;
                        }
                        Named::End => {
                            let width = ctx.state.get_buffer().width();
                            ctx.state.set_caret_x(width.saturating_sub(1));
                            return ToolResult::Redraw;
                        }
                        Named::PageUp => {
                            ctx.state.move_caret_up(24);
                            return ToolResult::Redraw;
                        }
                        Named::PageDown => {
                            ctx.state.move_caret_down(24);
                            return ToolResult::Redraw;
                        }
                        Named::Delete => {
                            let _ = if ctx.state.is_something_selected() {
                                ctx.state.erase_selection()
                            } else {
                                ctx.state.delete_key()
                            };
                            return ToolResult::Commit("Delete".to_string());
                        }
                        Named::Backspace => {
                            let _ = if ctx.state.is_something_selected() {
                                ctx.state.erase_selection()
                            } else {
                                ctx.state.backspace()
                            };
                            return ToolResult::Commit("Backspace".to_string());
                        }
                        Named::Enter => {
                            let _ = ctx.state.new_line();
                            return ToolResult::Commit("New line".to_string());
                        }
                        Named::Tab => {
                            if modifiers.shift() {
                                ctx.state.handle_reverse_tab();
                            } else {
                                ctx.state.handle_tab();
                            }
                            return ToolResult::Redraw;
                        }
                        Named::Insert => {
                            ctx.state.toggle_insert_mode();
                            return ToolResult::Redraw;
                        }
                        Named::Escape => {
                            let _ = ctx.state.clear_selection();
                            return ToolResult::Redraw;
                        }
                        _ => {}
                    }
                }

                ToolResult::None
            }
            _ => ToolResult::None,
        }
    }

    fn cursor(&self) -> iced::mouse::Interaction {
        if self.selection_drag != SelectionDrag::None {
            self.selection_drag.to_cursor_interaction().unwrap_or(iced::mouse::Interaction::Text)
        } else if self.hover_drag != SelectionDrag::None {
            self.hover_drag.to_cursor_interaction().unwrap_or(iced::mouse::Interaction::Text)
        } else {
            iced::mouse::Interaction::Text
        }
    }

    fn show_caret(&self) -> bool {
        true
    }

    fn show_selection(&self) -> bool {
        true
    }
}
