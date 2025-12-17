//! Line Tool
//!
//! Draw straight lines between two points.
//! Supports half-block mode for higher resolution.

use iced::Element;
use iced::widget::{column, text};
use icy_engine::{MouseButton, Position};

use super::{ToolContext, ToolHandler, ToolInput, ToolMessage, ToolResult};

/// Line tool state
#[derive(Clone, Debug, Default)]
pub struct LineTool {
    /// Start position of the line
    start_pos: Option<Position>,
    /// Current end position (during drag)
    current_pos: Option<Position>,
    /// Start position in half-block coordinates
    start_half_block: Option<Position>,
    /// Current position in half-block coordinates
    current_half_block: Option<Position>,
    /// Whether currently dragging
    is_dragging: bool,
    /// Mouse button used for drawing
    draw_button: MouseButton,
    /// Whether to clear/erase instead of draw (Shift modifier)
    clear_mode: bool,
}

impl LineTool {
    pub fn new() -> Self {
        Self::default()
    }
}

impl ToolHandler for LineTool {
    fn handle_event(&mut self, ctx: &mut ToolContext, event: ToolInput) -> ToolResult {
        match event {
            ToolInput::MouseDown {
                pos,
                pos_half_block,
                button,
                modifiers,
                ..
            } => {
                self.start_pos = Some(pos);
                self.current_pos = Some(pos);
                self.start_half_block = Some(pos_half_block);
                self.current_half_block = Some(pos_half_block);
                self.is_dragging = true;
                self.draw_button = button;
                self.clear_mode = modifiers.shift;

                ToolResult::StartCapture.and(ToolResult::Redraw)
            }

            ToolInput::MouseMove {
                pos,
                pos_half_block,
                is_dragging,
                ..
            } => {
                if is_dragging && self.is_dragging {
                    self.current_pos = Some(pos);
                    self.current_half_block = Some(pos_half_block);
                    // TODO: Update overlay preview
                    ToolResult::Redraw
                } else {
                    ToolResult::None
                }
            }

            ToolInput::MouseUp { pos, .. } => {
                if self.is_dragging {
                    self.current_pos = Some(pos);
                    self.is_dragging = false;

                    // TODO: Actually draw the line using ctx.state
                    // For now, just commit
                    let start = self.start_pos.unwrap_or_default();
                    let end = pos;

                    // Reset state
                    self.start_pos = None;
                    self.current_pos = None;
                    self.start_half_block = None;
                    self.current_half_block = None;

                    let _ = (ctx, start, end); // Will use these when implementing actual drawing

                    ToolResult::EndCapture.and(ToolResult::Commit(format!("Line from ({},{}) to ({},{})", start.x, start.y, end.x, end.y)))
                } else {
                    ToolResult::None
                }
            }

            ToolInput::Deactivate => {
                // Cancel any in-progress line
                self.start_pos = None;
                self.current_pos = None;
                self.start_half_block = None;
                self.current_half_block = None;
                self.is_dragging = false;
                ToolResult::Redraw
            }

            _ => ToolResult::None,
        }
    }

    fn view_toolbar<'a>(&'a self, _ctx: &'a ToolContext) -> Element<'a, ToolMessage> {
        // TODO: Add brush options (size, mode, etc.)
        column![].into()
    }

    fn view_status<'a>(&'a self, _ctx: &'a ToolContext) -> Element<'a, ToolMessage> {
        let status = if let (Some(start), Some(end)) = (self.start_pos, self.current_pos) {
            format!("Line | ({},{}) → ({},{}) | Shift=Erase", start.x, start.y, end.x, end.y)
        } else {
            "Line | Click and drag to draw".to_string()
        };
        text(status).into()
    }

    fn cursor(&self) -> iced::mouse::Interaction {
        iced::mouse::Interaction::Crosshair
    }

    fn show_caret(&self) -> bool {
        false
    }
}
