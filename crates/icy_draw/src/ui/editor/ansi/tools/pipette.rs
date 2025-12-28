//! Pipette Tool
//!
//! Pick color and/or character from the canvas.
//!
//! - Left click: Take both foreground and background colors
//! - Right click: Take background color only
//! - Shift+click: Take foreground only
//! - Ctrl+click: Take background only

use i18n_embed_fl::fl;
use iced::widget::canvas::{self, Frame, Geometry};
use iced::widget::{column, container, row, text, Canvas, Space};
use iced::{Alignment, Color, Element, Font, Length, Point, Rectangle, Size, Theme};
use icy_engine::{AttributedChar, BitFont, MouseButton, Position, TextPane};
use icy_engine_edit::tools::Tool;
use icy_engine_gui::terminal::crt_state::{is_command_pressed, is_ctrl_pressed, is_shift_pressed};
use icy_engine_gui::TerminalMessage;

use crate::LANGUAGE_LOADER;

use super::{ToolContext, ToolHandler, ToolId, ToolMessage, ToolResult, ToolViewContext};

/// Pipette tool state
#[derive(Clone, Debug, Default)]
pub struct PipetteTool {
    /// Currently hovered character (for preview in toolbar)
    hovered_char: Option<AttributedChar>,
    /// Font for the hovered character
    hovered_font: Option<BitFont>,
    /// Current hover position
    hovered_pos: Option<Position>,
}

impl PipetteTool {
    pub fn new() -> Self {
        Self::default()
    }
}

/// Canvas program to draw a single glyph preview
struct GlyphPreview {
    ch: char,
    font: BitFont,
    fg: Color,
    bg: Color,
}

impl canvas::Program<ToolMessage> for GlyphPreview {
    type State = ();

    fn draw(&self, _state: &Self::State, renderer: &iced::Renderer, _theme: &Theme, bounds: Rectangle, _cursor: iced::mouse::Cursor) -> Vec<Geometry> {
        let mut frame = Frame::new(renderer, bounds.size());

        let font_width = self.font.size().width as f32;
        let font_height = self.font.size().height as f32;

        // Calculate scale to fit in bounds while maintaining aspect ratio
        let scale_x = bounds.width / font_width;
        let scale_y = bounds.height / font_height;
        let scale = scale_x.min(scale_y);

        let char_width = font_width * scale;
        let char_height = font_height * scale;

        // Center the glyph
        let offset_x = (bounds.width - char_width) / 2.0;
        let offset_y = (bounds.height - char_height) / 2.0;

        // Fill background
        frame.fill_rectangle(Point::ORIGIN, bounds.size(), self.bg);

        // Draw glyph pixels
        let glyph = self.font.glyph(self.ch);
        let pixel_w = scale;
        let pixel_h = scale;

        let bitmap_pixels = glyph.to_bitmap_pixels();
        for (row_idx, row) in bitmap_pixels.iter().enumerate() {
            for (col_idx, &pixel) in row.iter().enumerate() {
                if pixel {
                    frame.fill_rectangle(
                        Point::new(offset_x + col_idx as f32 * pixel_w, offset_y + row_idx as f32 * pixel_h),
                        Size::new(pixel_w, pixel_h),
                        self.fg,
                    );
                }
            }
        }

        vec![frame.into_geometry()]
    }
}

impl ToolHandler for PipetteTool {
    fn id(&self) -> ToolId {
        ToolId::Tool(Tool::Pipette)
    }

    fn as_any(&self) -> &dyn std::any::Any {
        self
    }

    fn as_any_mut(&mut self) -> &mut dyn std::any::Any {
        self
    }

    fn handle_message(&mut self, _ctx: &mut ToolContext<'_>, _msg: &ToolMessage) -> ToolResult {
        ToolResult::None
    }

    fn handle_terminal_message(&mut self, ctx: &mut ToolContext, msg: &TerminalMessage) -> ToolResult {
        match msg {
            TerminalMessage::Move(evt) | TerminalMessage::Drag(evt) => {
                // Update hover preview
                if let Some(pos) = evt.text_position {
                    self.hovered_pos = Some(pos);
                    let ch = ctx.state.get_buffer().char_at(pos);
                    self.hovered_char = Some(ch);
                    // Get the font for this character's font page
                    let font_page = ch.attribute.font_page();
                    self.hovered_font = ctx.state.get_buffer().font(font_page).or_else(|| ctx.state.get_buffer().font(0)).cloned();
                }
                ToolResult::Redraw
            }

            TerminalMessage::Press(evt) => {
                let Some(pos) = evt.text_position else {
                    return ToolResult::None;
                };

                // Determine what to take based on button and modifiers
                let (take_fg, take_bg) = if evt.button == MouseButton::Right {
                    (false, true) // Right click: BG only
                } else {
                    (!evt.modifiers.ctrl, !evt.modifiers.shift) // Default: both
                };

                // Get character at position using TextPane trait
                let ch = ctx.state.get_buffer().char_at(pos);

                if take_fg {
                    ctx.state.set_caret_foreground(ch.attribute.foreground());
                }
                if take_bg {
                    ctx.state.set_caret_background(ch.attribute.background());
                }

                // Switch back to Click tool after picking
                ToolResult::SwitchTool(super::ToolId::Tool(Tool::Click))
            }

            _ => ToolResult::None,
        }
    }

    fn view_toolbar(&self, ctx: &ToolViewContext) -> Element<'_, ToolMessage> {
        let mut content = row![].spacing(16).align_y(iced::Alignment::Center);

        // Center content
        content = content.push(Space::new().width(Length::Fill));

        if let Some(ch) = self.hovered_char {
            // Character code as hex
            let code_text: String = format!("#{:02X}", ch.ch as u8);
            content = content.push(text(code_text).size(12).font(Font::MONOSPACE));

            // Glyph preview using canvas
            // Show what the caret colors will look like after picking:
            // - If taking FG: use hovered FG, else use caret FG
            // - If taking BG: use hovered BG, else use caret BG
            if let Some(font) = &self.hovered_font {
                let (take_fg_preview, take_bg_preview) = if is_shift_pressed() {
                    (true, false)
                } else if is_ctrl_pressed() || is_command_pressed() {
                    (false, true)
                } else {
                    (true, true)
                };

                let preview_fg_idx = if take_fg_preview { ch.attribute.foreground() } else { ctx.caret_fg };
                let preview_bg_idx = if take_bg_preview { ch.attribute.background() } else { ctx.caret_bg };
                let (fg_r, fg_g, fg_b) = ctx.palette.color(preview_fg_idx).rgb();
                let (bg_r, bg_g, bg_b) = ctx.palette.color(preview_bg_idx).rgb();

                let preview = GlyphPreview {
                    ch: ch.ch,
                    font: font.clone(),
                    fg: Color::from_rgb8(fg_r, fg_g, fg_b),
                    bg: Color::from_rgb8(bg_r, bg_g, bg_b),
                };

                let glyph_canvas = Canvas::new(preview).width(Length::Fixed(32.0)).height(Length::Fixed(32.0));

                let glyph_container = container(glyph_canvas).style(|_theme: &Theme| container::Style {
                    border: iced::Border {
                        color: Color::from_rgb8(80, 80, 80),
                        width: 1.0,
                        radius: 2.0.into(),
                    },
                    ..Default::default()
                });

                content = content.push(glyph_container);
            }

            // Foreground color: label + box in a row
            let fg_idx = ch.attribute.foreground();
            let (r, g, b) = ctx.palette.color(fg_idx).rgb();
            let text_color = if (r as f32 * 0.299 + g as f32 * 0.587 + b as f32 * 0.114) > 186.0 {
                iced::Color::BLACK
            } else {
                iced::Color::WHITE
            };
            let hex_text = format!("#{:02x}{:02x}{:02x}", r, g, b);

            // Show in the labels what would be taken on click, based on current modifier state.
            // (Right click is still BG-only, but the main confusion was Shift/Ctrl behavior.)
            let (take_fg_hint, take_bg_hint) = if is_shift_pressed() {
                (true, false)
            } else if is_ctrl_pressed() || is_command_pressed() {
                (false, true)
            } else {
                (true, true)
            };
            let fg_label_text = format!(
                "{} {}",
                fl!(LANGUAGE_LOADER, "pipette-foreground", index = fg_idx.to_string()),
                if take_fg_hint { "✓" } else { "×" }
            );
            let fg_label = text(fg_label_text).size(12).font(Font::MONOSPACE);
            let fg_box = container(
                text(hex_text)
                    .size(12)
                    .font(Font::MONOSPACE)
                    .style(move |_| iced::widget::text::Style { color: Some(text_color) }),
            )
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
            content = content.push(column![fg_label, fg_box, Space::new().width(2.0)].spacing(2).align_x(Alignment::Center));

            // Background color: label + box in a row
            let bg_idx = ch.attribute.background();
            let (r, g, b) = ctx.palette.color(bg_idx).rgb();
            let text_color = if (r as f32 * 0.299 + g as f32 * 0.587 + b as f32 * 0.114) > 186.0 {
                iced::Color::BLACK
            } else {
                iced::Color::WHITE
            };
            let hex_text = format!("#{:02x}{:02x}{:02x}", r, g, b);
            let bg_label_text = format!(
                "{} {}",
                fl!(LANGUAGE_LOADER, "pipette-background", index = bg_idx.to_string()),
                if take_bg_hint { "✓" } else { "×" }
            );
            let bg_label = text(bg_label_text).size(12).font(Font::MONOSPACE);
            let bg_box = container(
                text(hex_text)
                    .size(12)
                    .font(Font::MONOSPACE)
                    .style(move |_| iced::widget::text::Style { color: Some(text_color) }),
            )
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
            content = content.push(column![bg_label, bg_box].spacing(2).align_x(Alignment::Center));
        } else {
            content = content.push(text(fl!(LANGUAGE_LOADER, "pipette-hover_hint")).size(12));
        }

        // Help text
        content = content.push(Space::new().width(Length::Fixed(24.0)));
        content = content.push(text(fl!(LANGUAGE_LOADER, "pipette-help")).size(12));

        content = content.push(Space::new().width(Length::Fill));

        content.into()
    }

    fn cursor(&self) -> iced::mouse::Interaction {
        iced::mouse::Interaction::Crosshair
    }

    fn show_caret(&self) -> bool {
        false // Hide caret while using pipette
    }
}
