//! Pencil (Freehand Drawing) Tool
//!
//! Allows freehand drawing with various brush modes:
//! - Character mode: Stamps characters
//! - Half-block mode: 2x vertical resolution drawing
//! - Colorize mode: Changes only colors
//! - Shade mode: Lightens/darkens existing content

use super::{ToolContext, ToolHandler, ToolInput, ToolMessage, ToolOverlay, ToolResult};
use iced::Element;
use iced::widget::column;
use icy_engine::{MouseButton, Position};

/// State for freehand pencil drawing
#[derive(Debug, Clone, Default)]
pub struct PencilTool {
    /// Whether a stroke is in progress
    is_drawing: bool,
    /// Last position for interpolation
    last_pos: Option<Position>,
    /// Last half-block position for half-block mode interpolation
    last_half_block_pos: Position,
    /// Mouse button used for current stroke
    stroke_button: MouseButton,
}

impl PencilTool {
    pub fn new() -> Self {
        Self::default()
    }
}

impl ToolHandler for PencilTool {
    fn handle_event(&mut self, ctx: &mut ToolContext<'_>, input: ToolInput) -> ToolResult {
        match input {
            ToolInput::MouseDown {
                pos, pos_half_block, button, ..
            } => {
                self.is_drawing = true;
                self.last_pos = Some(pos);
                self.last_half_block_pos = pos_half_block;
                self.stroke_button = button;

                // Begin atomic undo for the stroke
                if ctx.undo_guard.is_none() {
                    *ctx.undo_guard = Some(ctx.state.begin_atomic_undo("Pencil".to_string()));
                }

                // TODO: Initial stamp at click position
                // The actual painting is still handled by AnsiEditor for now

                ToolResult::Multi(vec![ToolResult::StartCapture, ToolResult::Redraw])
            }

            ToolInput::MouseMove {
                pos,
                pos_half_block,
                is_dragging,
                ..
            } => {
                if !is_dragging || !self.is_drawing {
                    return ToolResult::None;
                }

                // Track positions for interpolation
                self.last_pos = Some(pos);
                self.last_half_block_pos = pos_half_block;

                // TODO: Interpolated painting between last_pos and current pos
                // The actual painting is still handled by AnsiEditor for now

                ToolResult::Redraw
            }

            ToolInput::MouseUp { .. } => {
                if !self.is_drawing {
                    return ToolResult::None;
                }

                self.is_drawing = false;
                self.last_pos = None;

                // End the atomic undo (drop the guard)
                *ctx.undo_guard = None;

                ToolResult::Multi(vec![ToolResult::EndCapture, ToolResult::Commit("Pencil".to_string())])
            }

            ToolInput::Activate => {
                self.is_drawing = false;
                self.last_pos = None;
                ToolResult::None
            }

            ToolInput::Deactivate => {
                // If a stroke is in progress, cancel it
                if self.is_drawing {
                    self.is_drawing = false;
                    self.last_pos = None;
                    *ctx.undo_guard = None;
                }
                ToolResult::None
            }

            ToolInput::Message(msg) => self.handle_message(msg),

            _ => ToolResult::None,
        }
    }

    fn view_toolbar<'a>(&'a self, _ctx: &'a ToolContext) -> Element<'a, ToolMessage> {
        // Brush toolbar is rendered by AnsiEditor for now
        // (uses shared brush_options from top_toolbar)
        column![].into()
    }

    fn view_options<'a>(&'a self, _ctx: &'a ToolContext) -> Element<'a, ToolMessage> {
        // Brush options are rendered by AnsiEditor for now
        column![].into()
    }

    fn view_status<'a>(&'a self, _ctx: &'a ToolContext) -> Element<'a, ToolMessage> {
        // Status rendering by AnsiEditor for now
        column![].into()
    }

    fn get_overlay(&self) -> Option<ToolOverlay> {
        None
    }

    fn cursor(&self) -> iced::mouse::Interaction {
        iced::mouse::Interaction::Crosshair
    }

    fn show_caret(&self) -> bool {
        false
    }

    fn show_selection(&self) -> bool {
        false
    }
}

impl PencilTool {
    fn handle_message(&mut self, msg: ToolMessage) -> ToolResult {
        match msg {
            ToolMessage::SetBrushSize(_) => {
                // Handled by ToolResources/Options
                ToolResult::None
            }
            ToolMessage::SetBrushChar(_) => {
                // Handled by ToolResources/Options
                ToolResult::None
            }
            _ => ToolResult::None,
        }
    }
}
