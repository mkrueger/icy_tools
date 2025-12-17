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
use iced::widget::{Space, checkbox, column, container, row, text};
use iced::{Length, Theme};
use icy_engine::{AttributedChar, MouseButton, Position, TextPane};
use icy_engine_edit::tools::Tool;
use icy_engine_gui::TerminalMessage;

use super::{ToolContext, ToolHandler, ToolMessage, ToolResult};

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
    fn handle_message(&mut self, _ctx: &mut ToolContext<'_>, msg: &ToolMessage) -> ToolResult {
        match *msg {
            ToolMessage::PipetteTakeForeground(v) => {
                self.take_fg = v;
                ToolResult::None
            }
            ToolMessage::PipetteTakeBackground(v) => {
                self.take_bg = v;
                ToolResult::None
            }
            ToolMessage::PipetteTakeChar(v) => {
                self.take_char = v;
                ToolResult::None
            }
            _ => ToolResult::None,
        }
    }

    fn handle_terminal_message(&mut self, ctx: &mut ToolContext, msg: &TerminalMessage) -> ToolResult {
        match msg {
            TerminalMessage::Move(evt) | TerminalMessage::Drag(evt) => {
                // Update hover preview
                if let Some(pos) = evt.text_position {
                    self.hovered_pos = Some(pos);
                    self.hovered_char = Some(ctx.state.get_buffer().char_at(pos));
                }
                ToolResult::Redraw
            }

            TerminalMessage::Press(evt) => {
                let Some(pos) = evt.text_position else {
                    return ToolResult::None;
                };

                // Determine what to take based on button and modifiers
                let (take_fg, take_bg) = if evt.modifiers.shift {
                    (true, false) // Shift: FG only
                } else if evt.modifiers.ctrl {
                    (false, true) // Ctrl: BG only
                } else if evt.button == MouseButton::Right {
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
                if self.take_char && evt.modifiers.alt {
                    // Could set brush char in resources here
                }

                // Switch back to Click tool after picking
                ToolResult::SwitchTool(Tool::Click)
            }

            _ => ToolResult::None,
        }
    }

    fn view_toolbar<'a>(&'a self, ctx: &super::ToolViewContext<'_>) -> Element<'a, ToolMessage> {
        let mut content = row![].spacing(16).align_y(iced::Alignment::Center);

        // Center content
        content = content.push(Space::new().width(Length::Fill));

        // Options
        let options = row![
            text("FG"),
            checkbox(self.take_fg).on_toggle(ToolMessage::PipetteTakeForeground),
            text("BG"),
            checkbox(self.take_bg).on_toggle(ToolMessage::PipetteTakeBackground),
            text("Char"),
            checkbox(self.take_char).on_toggle(ToolMessage::PipetteTakeChar),
        ]
        .spacing(8)
        .align_y(iced::Alignment::Center);

        content = content.push(options);

        if let Some(ch) = self.hovered_char {
            let char_display = if !ch.ch.is_control() && ch.ch as u32 >= 32 {
                format!("'{}'", ch.ch)
            } else {
                String::new()
            };
            content = content.push(text(format!("Code {} {}", ch.ch as u32, char_display)).size(12));

            // Foreground box
            if self.take_fg {
                let fg_idx = ch.attribute.foreground();
                let (r, g, b) = ctx.palette.color(fg_idx).rgb();
                let text_color = if (r as f32 * 0.299 + g as f32 * 0.587 + b as f32 * 0.114) > 186.0 {
                    iced::Color::BLACK
                } else {
                    iced::Color::WHITE
                };
                let hex_text = format!("#{:02x}{:02x}{:02x}", r, g, b);
                let fg_label = text(format!("Vordergrund {}", fg_idx)).size(12);
                let fg_box = container(text(hex_text).size(12).style(move |_| iced::widget::text::Style { color: Some(text_color) }))
                    .padding([4, 8])
                    .style(move |_theme: &Theme| container::Style {
                        background: Some(iced::Background::Color(iced::Color::from_rgb8(r, g, b))),
                        border: iced::Border {
                            color: iced::Color::WHITE,
                            width: 1.0,
                            radius: 4.0.into(),
                        },
                        ..Default::default()
                    });
                content = content.push(column![fg_label, fg_box].spacing(2).align_x(iced::Alignment::Center));
            }

            // Background box
            if self.take_bg {
                let bg_idx = ch.attribute.background();
                let (r, g, b) = ctx.palette.color(bg_idx).rgb();
                let text_color = if (r as f32 * 0.299 + g as f32 * 0.587 + b as f32 * 0.114) > 186.0 {
                    iced::Color::BLACK
                } else {
                    iced::Color::WHITE
                };
                let hex_text = format!("#{:02x}{:02x}{:02x}", r, g, b);
                let bg_label = text(format!("Hintergrund {}", bg_idx)).size(12);
                let bg_box = container(text(hex_text).size(12).style(move |_| iced::widget::text::Style { color: Some(text_color) }))
                    .padding([4, 8])
                    .style(move |_theme: &Theme| container::Style {
                        background: Some(iced::Background::Color(iced::Color::from_rgb8(r, g, b))),
                        border: iced::Border {
                            color: iced::Color::WHITE,
                            width: 1.0,
                            radius: 4.0.into(),
                        },
                        ..Default::default()
                    });
                content = content.push(column![bg_label, bg_box].spacing(2).align_x(iced::Alignment::Center));
            }
        } else {
            content = content.push(text("Hover over canvas to pick colors").size(12));
        }

        // Help text
        content = content.push(Space::new().width(Length::Fixed(24.0)));
        content = content.push(text("⇧: FG only   ⌃: BG only").size(12));

        content = content.push(Space::new().width(Length::Fill));

        content.into()
    }

    fn view_status<'a>(&'a self, _ctx: &super::ToolViewContext<'_>) -> Element<'a, ToolMessage> {
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
