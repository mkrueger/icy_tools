//! Font Tool (TDF/Figlet Font Rendering)
//!
//! Renders text using TDF (TheDraw Font) or Figlet fonts.
//! Each character typed is rendered as a multi-cell font glyph.

use iced::Element;
use iced::widget::{column, text};
use icy_engine::Position;

use super::{ToolContext, ToolHandler, ToolInput, ToolMessage, ToolResult};

/// Font tool state
#[derive(Clone, Debug, Default)]
pub struct FontTool {
    /// Currently selected font slot (0-9)
    font_slot: usize,
    /// Preview text (for status display)
    preview_text: String,
    /// Whether outline mode is enabled
    outline_mode: bool,
}

impl FontTool {
    pub fn new() -> Self {
        Self::default()
    }

    /// Get current font slot
    #[allow(dead_code)]
    pub fn font_slot(&self) -> usize {
        self.font_slot
    }

    /// Set font slot
    #[allow(dead_code)]
    pub fn set_font_slot(&mut self, slot: usize) {
        self.font_slot = slot.min(9);
    }

    /// Get outline mode
    #[allow(dead_code)]
    pub fn outline_mode(&self) -> bool {
        self.outline_mode
    }

    /// Set outline mode
    #[allow(dead_code)]
    pub fn set_outline_mode(&mut self, enabled: bool) {
        self.outline_mode = enabled;
    }
}

impl ToolHandler for FontTool {
    fn handle_event(&mut self, ctx: &mut ToolContext, event: ToolInput) -> ToolResult {
        match event {
            ToolInput::MouseDown { pos, .. } => {
                // Position caret at click location
                ctx.state.set_caret_position(pos);
                ToolResult::Redraw
            }

            ToolInput::KeyDown { key, modifiers } => {
                use iced::keyboard::key::Named;

                // Handle navigation keys
                if let iced::keyboard::Key::Named(named) = &key {
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
                        Named::Backspace => {
                            // TODO: Delete last font character (multi-cell)
                            return ToolResult::Commit("Delete font char".to_string());
                        }
                        Named::Enter => {
                            // Move to next line
                            let pos = ctx.state.get_caret().position();
                            ctx.state.set_caret_position(Position::new(0, pos.y + 1));
                            return ToolResult::Redraw;
                        }
                        _ => {}
                    }
                }

                // Handle font slot switching (0-9)
                if let iced::keyboard::Key::Character(ch) = &key {
                    if modifiers.ctrl {
                        if let Some(digit) = ch.chars().next().and_then(|c| c.to_digit(10)) {
                            self.font_slot = digit as usize;
                            return ToolResult::Status(format!("Font slot: {}", self.font_slot));
                        }
                    }
                }

                // Character input - render font glyph
                // The actual font rendering is handled by AnsiEditor's font system
                ToolResult::None
            }

            ToolInput::Message(msg) => match msg {
                ToolMessage::FontSelectSlot(slot) => {
                    self.font_slot = slot;
                    ToolResult::Redraw
                }
                ToolMessage::FontSetOutline(style) => {
                    self.outline_mode = style > 0;
                    ToolResult::Redraw
                }
                _ => ToolResult::None,
            },

            ToolInput::Deactivate => {
                self.preview_text.clear();
                ToolResult::None
            }

            _ => ToolResult::None,
        }
    }

    fn view_toolbar<'a>(&'a self, _ctx: &'a ToolContext) -> Element<'a, ToolMessage> {
        // Font selector is rendered by AnsiEditor
        column![].into()
    }

    fn view_status<'a>(&'a self, ctx: &'a ToolContext) -> Element<'a, ToolMessage> {
        let caret = ctx.state.get_caret();
        let pos = caret.position();

        let status = format!(
            "Font | Slot: {} | Pos: ({},{}) | Outline: {} | Ctrl+0-9=Switch font",
            self.font_slot,
            pos.x,
            pos.y,
            if self.outline_mode { "On" } else { "Off" }
        );
        text(status).into()
    }

    fn cursor(&self) -> iced::mouse::Interaction {
        iced::mouse::Interaction::Text
    }

    fn show_caret(&self) -> bool {
        true
    }

    fn show_selection(&self) -> bool {
        true
    }
}
