//! Line Numbers Overlay Widget
//!
//! Renders row numbers on the left/right and column numbers on top/bottom of the canvas.
//! Uses display_scale from RenderInfo for correct zoom (works with Auto and Manual modes).
//! Draws as an overlay on top of the terminal widget.

use crate::ui::editor::ansi::AnsiEditorCoreMessage;
use icy_engine_gui::RenderInfo;
use icy_ui::{widget::canvas, Element, Length, Renderer, Theme};
use parking_lot::RwLock;
use std::sync::Arc;

/// Create line numbers overlay that draws on top of the terminal
/// Uses RenderInfo.display_scale for the actual zoom factor (works with Auto/Manual modes)
///
/// If `selection_range` is Some, it highlights the selection range instead of the caret position.
/// The selection_range is (min_col, min_row, max_col, max_row).
pub fn line_numbers_overlay(
    render_info: Arc<RwLock<RenderInfo>>,
    buffer_width: usize,
    buffer_height: usize,
    font_width: f32,
    font_height: f32,
    caret_row: usize,
    caret_col: usize,
    scroll_x: f32,
    scroll_y: f32,
    selection_range: Option<(usize, usize, usize, usize)>,
) -> Element<'static, AnsiEditorCoreMessage> {
    let state = LineNumbersOverlayState {
        render_info,
        buffer_width,
        buffer_height,
        font_width,
        font_height,
        caret_row,
        caret_col,
        scroll_x,
        scroll_y,
        selection_range,
    };

    canvas(state).width(Length::Fill).height(Length::Fill).into()
}

struct LineNumbersOverlayState {
    render_info: Arc<RwLock<RenderInfo>>,
    buffer_width: usize,
    buffer_height: usize,
    font_width: f32,
    font_height: f32,
    caret_row: usize,
    caret_col: usize,
    scroll_x: f32,
    scroll_y: f32,
    selection_range: Option<(usize, usize, usize, usize)>,
}

impl<Message> canvas::Program<Message> for LineNumbersOverlayState {
    type State = ();

    fn draw(
        &self,
        _state: &Self::State,
        renderer: &Renderer,
        theme: &Theme,
        bounds: icy_ui::Rectangle,
        _cursor: icy_ui::mouse::Cursor,
    ) -> Vec<canvas::Geometry> {
        let mut frame = canvas::Frame::new(renderer, bounds.size());

        // Get the effective zoom from RenderInfo (works for both Auto and Manual modes)
        let zoom = {
            let info = self.render_info.read();
            if info.display_scale > 0.0 {
                info.display_scale
            } else {
                1.0 // Fallback if not yet initialized
            }
        };

        // Calculate scaled font dimensions
        let scaled_font_width = self.font_width * zoom;
        let scaled_font_height = self.font_height * zoom;

        if scaled_font_width <= 0.0 || scaled_font_height <= 0.0 {
            return vec![frame.into_geometry()];
        }

        // Calculate total content size
        let content_width = self.buffer_width as f32 * scaled_font_width;
        let content_height = self.buffer_height as f32 * scaled_font_height;

        // Calculate centering offset (like the terminal shader does)
        let offset_x = ((bounds.width - content_width) / 2.0).max(0.0);
        let offset_y = ((bounds.height - content_height) / 2.0).max(0.0);

        // Text colors - derived from theme
        let text_color = theme.secondary.on.scale_alpha(0.8);
        let highlight_color = theme.background.on;

        // Font size for line numbers (scales with zoom)
        let line_number_font_size = (12.0 * scaled_font_height / 16.0).max(8.0).min(16.0);

        // Calculate scroll offset in pixels (scaled)
        let scroll_x_px = self.scroll_x * zoom;
        let scroll_y_px = self.scroll_y * zoom;

        // === Draw row numbers for ALL rows (up to buffer height) ===
        for row in 0..self.buffer_height {
            // Y position: row * font_height - scroll, plus centering offset
            let y = offset_y + (row as f32 * scaled_font_height) - scroll_y_px;

            // Skip if outside visible area (with some margin for the text)
            if y + scaled_font_height < -20.0 || y > bounds.height + 20.0 {
                continue;
            }

            let label = format!("{}", row + 1);
            // Highlight if row is in selection range, otherwise check caret position
            let is_highlighted = if let Some((_, min_row, _, max_row)) = self.selection_range {
                row >= min_row && row <= max_row
            } else {
                row == self.caret_row
            };
            let color = if is_highlighted { highlight_color } else { text_color };

            // Left side: right-aligned, just before content
            let left_x = offset_x - 4.0;
            if left_x > 0.0 {
                frame.fill_text(canvas::Text {
                    content: label.clone(),
                    position: icy_ui::Point::new(left_x, y + scaled_font_height * 0.15),
                    color,
                    size: icy_ui::Pixels(line_number_font_size),
                    align_x: icy_ui::alignment::Horizontal::Right.into(),
                    ..Default::default()
                });
            }

            // Right side: left-aligned, just after content
            let right_x = offset_x + content_width + 4.0;
            if right_x < bounds.width {
                frame.fill_text(canvas::Text {
                    content: label,
                    position: icy_ui::Point::new(right_x, y + scaled_font_height * 0.15),
                    color,
                    size: icy_ui::Pixels(line_number_font_size),
                    align_x: icy_ui::alignment::Horizontal::Left.into(),
                    ..Default::default()
                });
            }
        }

        // === Draw column numbers for ALL columns (up to buffer width) ===
        for col in 0..self.buffer_width {
            // X position: col * font_width - scroll, plus centering offset
            let x = offset_x + (col as f32 * scaled_font_width) - scroll_x_px + scaled_font_width * 0.5;

            // Skip if outside visible area
            if x + scaled_font_width < -10.0 || x > bounds.width + 10.0 {
                continue;
            }

            // Show only last digit (1-based, mod 10)
            let label = format!("{}", (col + 1) % 10);
            // Highlight if column is in selection range, otherwise check caret position
            let is_highlighted = if let Some((min_col, _, max_col, _)) = self.selection_range {
                col >= min_col && col <= max_col
            } else {
                col == self.caret_col
            };
            let color = if is_highlighted { highlight_color } else { text_color };

            // Top: above content
            let top_y = offset_y - 2.0;
            if top_y > 0.0 {
                frame.fill_text(canvas::Text {
                    content: label.clone(),
                    position: icy_ui::Point::new(x, top_y),
                    color,
                    size: icy_ui::Pixels(line_number_font_size),
                    align_x: icy_ui::alignment::Horizontal::Center.into(),
                    align_y: icy_ui::alignment::Vertical::Bottom.into(),
                    ..Default::default()
                });
            }

            // Bottom: below content
            let bottom_y = offset_y + content_height + 2.0;
            if bottom_y < bounds.height {
                frame.fill_text(canvas::Text {
                    content: label,
                    position: icy_ui::Point::new(x, bottom_y),
                    color,
                    size: icy_ui::Pixels(line_number_font_size),
                    align_x: icy_ui::alignment::Horizontal::Center.into(),
                    align_y: icy_ui::alignment::Vertical::Top.into(),
                    ..Default::default()
                });
            }
        }

        vec![frame.into_geometry()]
    }
}
