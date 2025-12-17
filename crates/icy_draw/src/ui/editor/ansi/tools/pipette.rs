//! Pipette Tool
//!
//! Pick color and/or character from the canvas.
//!
//! - Left click: Take foreground color (or both colors if no modifier)
//! - Right click: Take background color
//! - Shift+click: Take foreground only
//! - Ctrl+click: Take background only
//! - Alt+click: Take character as well

use iced::Element;
use iced::widget::{checkbox, row, text};
use icy_engine::{AttributedChar, MouseButton, Position, TextPane};
use icy_engine_edit::tools::Tool;

use super::{ToolContext, ToolHandler, ToolInput, ToolMessage, ToolResult};

/// Pipette tool state
#[derive(Clone, Debug, Default)]
pub struct PipetteTool {
    /// Currently hovered character (for preview in status bar)
    hovered_char: Option<AttributedChar>,
    /// Current hover position
    hovered_pos: Option<Position>,
    /// Take foreground color
    take_fg: bool,
    /// Take background color
    take_bg: bool,
    /// Take character
    take_char: bool,
}

impl PipetteTool {
    pub fn new() -> Self {
        Self {
            take_fg: true,
            take_bg: true,
            take_char: false,
            ..Default::default()
        }
    }
}

impl ToolHandler for PipetteTool {
    fn handle_event(&mut self, ctx: &mut ToolContext, event: ToolInput) -> ToolResult {
        match event {
            ToolInput::MouseMove { pos, .. } => {
                // Update hover preview
                self.hovered_pos = Some(pos);
                // Get character from buffer using TextPane trait
                self.hovered_char = Some(ctx.state.get_buffer().char_at(pos));
                ToolResult::Redraw
            }

            ToolInput::MouseDown { pos, button, modifiers, .. } => {
                // Determine what to take based on button and modifiers
                let (take_fg, take_bg) = if modifiers.shift {
                    (true, false) // Shift: FG only
                } else if modifiers.ctrl {
                    (false, true) // Ctrl: BG only
                } else if button == MouseButton::Right {
                    (false, true) // Right click: BG only
                } else {
                    (self.take_fg, self.take_bg) // Use current settings
                };

                // Get character at position using TextPane trait
                let ch = ctx.state.get_buffer().char_at(pos);

                if take_fg {
                    ctx.state.set_caret_foreground(ch.attribute.foreground());
                }
                if take_bg {
                    ctx.state.set_caret_background(ch.attribute.background());
                }
                if self.take_char && modifiers.alt {
                    // Could set brush char in resources here
                }

                // Switch back to Click tool after picking
                ToolResult::SwitchTool(Tool::Click)
            }

            ToolInput::Message(msg) => {
                match msg {
                    ToolMessage::PipetteTakeForeground(v) => self.take_fg = v,
                    ToolMessage::PipetteTakeBackground(v) => self.take_bg = v,
                    ToolMessage::PipetteTakeChar(v) => self.take_char = v,
                    _ => return ToolResult::None,
                }
                ToolResult::Redraw
            }

            ToolInput::Deactivate => {
                // Clear hover state when leaving
                self.hovered_char = None;
                self.hovered_pos = None;
                ToolResult::None
            }

            _ => ToolResult::None,
        }
    }

    fn view_toolbar<'a>(&'a self, _ctx: &'a ToolContext) -> Element<'a, ToolMessage> {
        row![
            text("FG"),
            checkbox(self.take_fg).on_toggle(ToolMessage::PipetteTakeForeground),
            text("BG"),
            checkbox(self.take_bg).on_toggle(ToolMessage::PipetteTakeBackground),
            text("Char"),
            checkbox(self.take_char).on_toggle(ToolMessage::PipetteTakeChar),
        ]
        .spacing(10)
        .into()
    }

    fn view_status<'a>(&'a self, _ctx: &'a ToolContext) -> Element<'a, ToolMessage> {
        let status = if let (Some(pos), Some(ch)) = (&self.hovered_pos, &self.hovered_char) {
            format!(
                "Pipette | Pos: ({}, {}) | Char: '{}' | FG: {} | BG: {}",
                pos.x,
                pos.y,
                if ch.ch.is_control() { '?' } else { ch.ch },
                ch.attribute.foreground(),
                ch.attribute.background()
            )
        } else {
            "Pipette | Click to pick color".to_string()
        };
        text(status).into()
    }

    fn cursor(&self) -> iced::mouse::Interaction {
        iced::mouse::Interaction::Crosshair
    }

    fn show_caret(&self) -> bool {
        false // Hide caret while using pipette
    }
}
