//! Outline style picker widget for TDF (TheDraw Font) outline fonts.
//!
//! This module provides a reusable `OutlinePicker` widget that displays
//! all available outline styles (19 styles from TheDraw) and allows
//! the user to select one.

use codepages::tables::UNICODE_TO_CP437;
use iced::{
    Element, Length, Point, Rectangle, Size, Theme, mouse,
    mouse::Cursor,
    widget::canvas::{self, Action, Cache, Frame, Geometry, Program},
};
use icy_engine::BitFont;

use super::SettingsDialogMessage;

/// Preview pattern dimensions (characters)
const OUTLINE_WIDTH: usize = 8;
const OUTLINE_HEIGHT: usize = 6;

/// Preview pattern using TheDraw outline placeholders (A-Q = 65-81)
/// This pattern shows all the different outline elements:
/// - Corners (E,F,G,H,I,J,K,L = various corner types)
/// - Horizontal lines (A,B)
/// - Vertical lines (C,D)
/// - T-junctions (M,N)
/// - Fill marker (@=64)
/// - Hole marker (O=79)
const OUTLINE_FONT_CHAR: [u8; 48] = [
    69, 65, 65, 65, 65, 65, 65, 70, // E A A A A A A F  (top row with corners)
    67, 79, 71, 66, 66, 72, 79, 68, // C O G B B H O D  (with holes and junctions)
    67, 79, 73, 65, 65, 74, 79, 68, // C O I A A J O D
    67, 79, 71, 66, 66, 72, 79, 68, // C O G B B H O D
    67, 79, 68, 64, 64, 67, 79, 68, // C O D @ @ C O D  (with fill markers)
    75, 66, 76, 64, 64, 75, 66, 76, // K B L @ @ K B L  (bottom corners)
];

/// Number of outline styles per row in the picker grid
const PER_ROW: usize = 7;

/// Padding around each preview cell (left/right has extra space for hotkey label)
const CELL_PADDING: f32 = 4.0;
const CELL_PADDING_LEFT: f32 = 16.0;

/// Spacing between cells
const CELL_SPACING: f32 = 4.0;

/// Total number of outline styles available (from TheDraw/retrofont)
pub const OUTLINE_STYLES: usize = 19;

/// TheDraw keyboard shortcuts for outline styles (A-S)
const THEDRAW_SHORTCUTS: [&str; 19] = ["A", "B", "C", "D", "E", "F", "G", "H", "I", "J", "K", "L", "M", "N", "O", "P", "Q", "R", "S"];

/// Create an outline picker element.
///
/// # Arguments
/// * `selected_outline` - Currently selected outline style index (0-18)
/// * `cursor` - Current cursor position for keyboard navigation
///
/// # Returns
/// An iced Element that can be used in any view
pub fn outline_picker(selected_outline: usize, cursor: usize) -> Element<'static, SettingsDialogMessage> {
    let program = OutlinePickerProgram {
        selected_outline,
        cursor,
        cache: Cache::default(),
        font: BitFont::default(),
    };

    let (cell_w, cell_h) = program.cell_size();
    let rows = (OUTLINE_STYLES + PER_ROW - 1) / PER_ROW;
    let total_width = PER_ROW as f32 * (cell_w + CELL_SPACING) - CELL_SPACING;
    let total_height = rows as f32 * (cell_h + CELL_SPACING) - CELL_SPACING;

    canvas::Canvas::new(program)
        .width(Length::Fixed(total_width))
        .height(Length::Fixed(total_height))
        .into()
}

struct OutlinePickerProgram {
    selected_outline: usize,
    cursor: usize,
    cache: Cache,
    font: BitFont,
}

impl OutlinePickerProgram {
    /// Calculate the size of a single preview cell
    fn cell_size(&self) -> (f32, f32) {
        let font_size = self.font.size();
        let w = font_size.width as f32 * OUTLINE_WIDTH as f32 + CELL_PADDING_LEFT + CELL_PADDING;
        let h = font_size.height as f32 * OUTLINE_HEIGHT as f32 + 2.0 * CELL_PADDING;
        (w, h)
    }

    /// Get the bounding rectangle for a specific style cell
    fn cell_rect(&self, style: usize) -> Rectangle {
        let (cw, ch) = self.cell_size();
        let row = style / PER_ROW;
        let col = style % PER_ROW;

        Rectangle {
            x: col as f32 * (cw + CELL_SPACING),
            y: row as f32 * (ch + CELL_SPACING),
            width: cw,
            height: ch,
        }
    }

    /// Find which style cell contains the given point
    fn hit_test(&self, p: Point) -> Option<usize> {
        for style in 0..OUTLINE_STYLES {
            if self.cell_rect(style).contains(p) {
                return Some(style);
            }
        }
        None
    }

    /// Draw a single outline preview cell
    fn draw_outline_cell(&self, frame: &mut Frame, style: usize, rect: Rectangle, fg: iced::Color, bg: iced::Color) {
        let font_size = self.font.size();
        let font_w = font_size.width as usize;
        let font_h = font_size.height as usize;

        // Fill background
        frame.fill_rectangle(Point::new(rect.x, rect.y), rect.size(), bg);

        // Draw each character in the preview pattern
        for row in 0..OUTLINE_HEIGHT {
            for col in 0..OUTLINE_WIDTH {
                let src_char = OUTLINE_FONT_CHAR[col + row * OUTLINE_WIDTH];

                // Transform the outline placeholder to the actual Unicode character for this style
                let unicode_ch = retrofont::transform_outline(style, src_char);

                // Convert Unicode back to CP437 for the bitmap font lookup
                let cp437_ch = if let Some(&cp437) = UNICODE_TO_CP437.get(&unicode_ch) {
                    char::from(cp437)
                } else {
                    unicode_ch
                };

                // Get the glyph from the CP437 font
                if let Some(glyph) = self.font.glyph(cp437_ch) {
                    // Draw each pixel of the glyph
                    for (y, glyph_row) in glyph.bitmap.pixels.iter().enumerate() {
                        if y >= font_h {
                            break;
                        }
                        for (x, &pixel) in glyph_row.iter().enumerate() {
                            if x >= font_w {
                                break;
                            }
                            if pixel {
                                let px = rect.x + CELL_PADDING_LEFT + col as f32 * font_w as f32 + x as f32;
                                let py = rect.y + CELL_PADDING + row as f32 * font_h as f32 + y as f32;
                                frame.fill_rectangle(Point::new(px, py), Size::new(1.0, 1.0), fg);
                            }
                        }
                    }
                }
            }
        }
    }
}

impl Program<SettingsDialogMessage> for OutlinePickerProgram {
    type State = Option<usize>; // Hovered style

    fn update(&self, state: &mut Self::State, event: &iced::Event, bounds: Rectangle, cursor: Cursor) -> Option<Action<SettingsDialogMessage>> {
        match event {
            iced::Event::Mouse(mouse::Event::CursorMoved { .. }) => {
                let new_hover = cursor.position_in(bounds).and_then(|p| self.hit_test(p));
                if *state != new_hover {
                    *state = new_hover;
                    return Some(Action::request_redraw());
                }
                None
            }
            iced::Event::Mouse(mouse::Event::CursorLeft) => {
                if state.is_some() {
                    *state = None;
                    return Some(Action::request_redraw());
                }
                None
            }
            iced::Event::Mouse(mouse::Event::ButtonPressed(mouse::Button::Left)) => {
                let Some(p) = cursor.position_in(bounds) else {
                    return None;
                };
                let style = self.hit_test(p)?;
                Some(Action::publish(SettingsDialogMessage::SelectOutlineStyle(style)))
            }
            _ => None,
        }
    }

    fn draw(&self, state: &Self::State, renderer: &iced::Renderer, theme: &Theme, bounds: Rectangle, _cursor: Cursor) -> Vec<Geometry> {
        let palette = theme.extended_palette();
        let hovered = *state;

        let geometry = self.cache.draw(renderer, bounds.size(), |frame: &mut Frame| {
            for style in 0..OUTLINE_STYLES {
                let rect = self.cell_rect(style);
                let is_selected = style == self.selected_outline;
                let is_cursor = style == self.cursor;
                let is_hovered = hovered == Some(style);

                // Determine colors based on state
                let (bg, fg) = if is_selected {
                    (palette.primary.strong.color, palette.primary.strong.text)
                } else if is_cursor {
                    (palette.secondary.base.color, palette.secondary.base.text)
                } else if is_hovered {
                    (palette.primary.weak.color, palette.primary.weak.text)
                } else {
                    (palette.background.weak.color, palette.background.base.text)
                };

                // Draw the cell
                self.draw_outline_cell(frame, style, rect, fg, bg);

                // Draw TheDraw shortcut label in top-left corner
                let shortcut = THEDRAW_SHORTCUTS[style];
                let label_text = canvas::Text {
                    content: shortcut.to_string(),
                    position: Point::new(rect.x + 3.0, rect.y + 2.0),
                    color: if is_selected {
                        palette.primary.strong.text
                    } else if is_cursor {
                        palette.secondary.base.text
                    } else {
                        palette.secondary.weak.color
                    },
                    size: iced::Pixels(13.0),
                    ..Default::default()
                };
                frame.fill_text(label_text);

                // Draw border
                let border_color = if is_selected {
                    palette.primary.base.color
                } else if is_cursor {
                    palette.secondary.strong.color
                } else {
                    palette.background.strong.color
                };
                let border = canvas::Path::rectangle(Point::new(rect.x, rect.y), rect.size());
                frame.stroke(
                    &border,
                    canvas::Stroke::default()
                        .with_width(if is_selected || is_cursor { 2.0 } else { 1.0 })
                        .with_color(border_color),
                );
            }
        });

        vec![geometry]
    }

    fn mouse_interaction(&self, _state: &Self::State, bounds: Rectangle, cursor: Cursor) -> mouse::Interaction {
        if let Some(p) = cursor.position_in(bounds) {
            if self.hit_test(p).is_some() {
                return mouse::Interaction::Pointer;
            }
        }
        mouse::Interaction::default()
    }
}
