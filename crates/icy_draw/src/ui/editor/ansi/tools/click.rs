//! Click Tool (Text Cursor / Keyboard Input)
//!
//! The primary text editing tool. Handles:
//! - Caret positioning and movement
//! - Character input and insertion
//! - Line operations (insert, delete)
//! - Cursor navigation (arrows, home, end, etc.)
//! - Layer dragging (Ctrl+Click+Drag)

use iced::Element;
use iced::widget::{column, text};
use icy_engine::{Position, TextPane};

use super::{ToolContext, ToolHandler, ToolInput, ToolMessage, ToolResult};

/// Click tool state
#[derive(Clone, Debug, Default)]
pub struct ClickTool {
    /// Whether layer drag is active (Ctrl+Click+Drag)
    layer_drag_active: bool,
    /// Layer offset at start of drag
    layer_drag_start_offset: Position,
    /// Start position of drag
    drag_start: Option<Position>,
    /// Current position during drag
    drag_current: Option<Position>,
}

impl ClickTool {
    pub fn new() -> Self {
        Self::default()
    }

    /// Check if layer drag is active
    #[allow(dead_code)]
    pub fn is_layer_dragging(&self) -> bool {
        self.layer_drag_active
    }
}

impl ToolHandler for ClickTool {
    fn handle_event(&mut self, ctx: &mut ToolContext, event: ToolInput) -> ToolResult {
        match event {
            ToolInput::MouseDown { pos, modifiers, .. } => {
                if modifiers.ctrl || modifiers.meta {
                    // Start layer drag
                    self.layer_drag_active = true;
                    self.drag_start = Some(pos);
                    self.drag_current = Some(pos);

                    // Get current layer offset
                    if let Some(layer) = ctx.state.get_cur_layer() {
                        self.layer_drag_start_offset = layer.offset();
                    }

                    ToolResult::StartCapture.and(ToolResult::Redraw)
                } else {
                    // Normal click - position caret
                    ctx.state.set_caret_position(pos);
                    ToolResult::Redraw
                }
            }

            ToolInput::MouseMove { pos, is_dragging, .. } => {
                if is_dragging && self.layer_drag_active {
                    self.drag_current = Some(pos);

                    // Calculate delta and update layer preview
                    if let Some(start) = self.drag_start {
                        let delta = pos - start;
                        let new_offset = self.layer_drag_start_offset + delta;

                        if let Some(layer) = ctx.state.get_cur_layer_mut() {
                            layer.set_preview_offset(Some(new_offset));
                        }
                    }

                    ToolResult::Redraw
                } else {
                    ToolResult::None
                }
            }

            ToolInput::MouseUp { pos, .. } => {
                if self.layer_drag_active {
                    self.layer_drag_active = false;

                    // Apply final layer offset
                    if let Some(start) = self.drag_start {
                        let delta = pos - start;
                        let new_offset = self.layer_drag_start_offset + delta;

                        if let Some(layer) = ctx.state.get_cur_layer_mut() {
                            layer.set_preview_offset(None);
                            layer.set_offset(new_offset);
                        }
                    }

                    self.drag_start = None;
                    self.drag_current = None;

                    ToolResult::EndCapture.and(ToolResult::Commit("Move layer".to_string()))
                } else {
                    ToolResult::None
                }
            }

            ToolInput::KeyDown { key, modifiers } => {
                use iced::keyboard::key::Named;

                if let iced::keyboard::Key::Named(named) = key {
                    match named {
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
                            if modifiers.ctrl {
                                ctx.state.set_caret_position(Position::new(0, 0));
                            } else {
                                let pos = ctx.state.get_caret().position();
                                ctx.state.set_caret_position(Position::new(0, pos.y));
                            }
                            return ToolResult::Redraw;
                        }
                        Named::End => {
                            let pos = ctx.state.get_caret().position();
                            let width = ctx.state.get_buffer().width();
                            ctx.state.set_caret_position(Position::new(width - 1, pos.y));
                            return ToolResult::Redraw;
                        }
                        Named::Delete => {
                            let _ = ctx.state.delete_key();
                            return ToolResult::Commit("Delete".to_string());
                        }
                        Named::Backspace => {
                            let _ = ctx.state.backspace();
                            return ToolResult::Commit("Backspace".to_string());
                        }
                        Named::Enter => {
                            // Move to next line
                            let pos = ctx.state.get_caret().position();
                            ctx.state.set_caret_position(Position::new(0, pos.y + 1));
                            return ToolResult::Redraw;
                        }
                        Named::Tab => {
                            // Move to next tab stop (every 8 columns)
                            let pos = ctx.state.get_caret().position();
                            let next_tab = ((pos.x / 8) + 1) * 8;
                            ctx.state.set_caret_position(Position::new(next_tab, pos.y));
                            return ToolResult::Redraw;
                        }
                        _ => {}
                    }
                }

                // Character input is handled by the editor's character input handling
                ToolResult::None
            }

            ToolInput::Deactivate => {
                self.layer_drag_active = false;
                self.drag_start = None;
                self.drag_current = None;
                ToolResult::None
            }

            _ => ToolResult::None,
        }
    }

    fn view_toolbar<'a>(&'a self, _ctx: &'a ToolContext) -> Element<'a, ToolMessage> {
        column![].into()
    }

    fn view_status<'a>(&'a self, ctx: &'a ToolContext) -> Element<'a, ToolMessage> {
        let caret = ctx.state.get_caret();
        let pos = caret.position();

        let status = if self.layer_drag_active {
            "Click | Dragging layer...".to_string()
        } else {
            format!("Click | Pos: ({},{}) | Ctrl+Drag=Move layer", pos.x, pos.y)
        };
        text(status).into()
    }

    fn cursor(&self) -> iced::mouse::Interaction {
        if self.layer_drag_active {
            iced::mouse::Interaction::Grabbing
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
