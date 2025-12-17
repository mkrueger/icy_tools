//! Fill Tool (Bucket Fill / Flood Fill)
//!
//! Fills connected regions with color/character.
//! Supports multiple modes:
//! - Character mode: Fill with character
//! - Half-block mode: Fill with half-block characters
//! - Colorize mode: Change only colors

use iced::Element;
use iced::widget::{column, text};
use icy_engine::{MouseButton, Position};

use super::{ToolContext, ToolHandler, ToolInput, ToolMessage, ToolResult};

/// Fill mode
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
#[allow(dead_code)]
pub enum FillMode {
    #[default]
    Character,
    HalfBlock,
    Colorize,
}

/// Fill tool state
#[derive(Clone, Debug, Default)]
pub struct FillTool {
    /// Current fill mode
    mode: FillMode,
    /// Last fill position (for status display)
    last_fill_pos: Option<Position>,
    /// Whether exact matching is enabled
    exact_matching: bool,
}

impl FillTool {
    pub fn new() -> Self {
        Self::default()
    }

    /// Get current fill mode
    #[allow(dead_code)]
    pub fn mode(&self) -> FillMode {
        self.mode
    }

    /// Set fill mode
    #[allow(dead_code)]
    pub fn set_mode(&mut self, mode: FillMode) {
        self.mode = mode;
    }
}

impl ToolHandler for FillTool {
    fn handle_event(&mut self, ctx: &mut ToolContext, event: ToolInput) -> ToolResult {
        match event {
            ToolInput::MouseDown { pos, button, .. } => {
                self.last_fill_pos = Some(pos);

                // Begin atomic undo
                let _undo = ctx.state.begin_atomic_undo("Bucket fill".to_string());

                // The actual fill logic is complex and still handled by AnsiEditor
                // This handler just tracks state and provides the interface

                let action = if button == MouseButton::Right { "Fill (swap colors)" } else { "Fill" };

                ToolResult::Commit(format!("{} at ({},{})", action, pos.x, pos.y))
            }

            ToolInput::MouseMove { pos, .. } => {
                // Update hover position for status display
                self.last_fill_pos = Some(pos);
                ToolResult::None
            }

            ToolInput::Message(msg) => match msg {
                ToolMessage::ToggleFilled(exact) => {
                    self.exact_matching = exact;
                    ToolResult::None
                }
                _ => ToolResult::None,
            },

            _ => ToolResult::None,
        }
    }

    fn view_toolbar<'a>(&'a self, _ctx: &'a ToolContext) -> Element<'a, ToolMessage> {
        // Fill options are rendered by AnsiEditor for now
        column![].into()
    }

    fn view_status<'a>(&'a self, _ctx: &'a ToolContext) -> Element<'a, ToolMessage> {
        let mode_str = match self.mode {
            FillMode::Character => "Char",
            FillMode::HalfBlock => "Half-Block",
            FillMode::Colorize => "Colorize",
        };

        let status = if let Some(pos) = self.last_fill_pos {
            format!("Fill ({}) | Position: ({},{}) | Right-click=Swap colors", mode_str, pos.x, pos.y)
        } else {
            format!("Fill ({}) | Click to fill region", mode_str)
        };
        text(status).into()
    }

    fn cursor(&self) -> iced::mouse::Interaction {
        iced::mouse::Interaction::Crosshair
    }

    fn show_caret(&self) -> bool {
        false
    }

    fn show_selection(&self) -> bool {
        true
    }
}
