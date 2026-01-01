//! Outline Style Selector Popup
//!
//! A visual grid of outline styles that can be selected with the mouse.
//! Similar to the CharSelector popup, used for selecting TheDraw font outline styles.

use codepages::tables::UNICODE_TO_CP437;
use icy_ui::{
    mouse::{self, Cursor},
    widget::{
        canvas::{self, Canvas, Frame, Geometry, Path, Stroke},
        Action,
    },
    Color, Element, Length, Point, Rectangle, Size, Theme,
};
use icy_engine::BitFont;

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
const PER_ROW: usize = 5;

/// Padding around each preview cell
const CELL_PADDING: f32 = 6.0;
const CELL_PADDING_LEFT: f32 = 18.0;

/// Spacing between cells
const CELL_SPACING: f32 = 6.0;

/// Total number of outline styles available (from TheDraw/retrofont)
pub const OUTLINE_STYLES: usize = 19;

/// Uniform padding around the entire popup
const POPUP_PADDING: f32 = 12.0;

/// TheDraw keyboard shortcuts for outline styles (A-S)
const THEDRAW_SHORTCUTS: [&str; 19] = ["A", "B", "C", "D", "E", "F", "G", "H", "I", "J", "K", "L", "M", "N", "O", "P", "Q", "R", "S"];

/// Messages from the outline selector
#[derive(Clone, Debug)]
pub enum OutlineSelectorMessage {
    /// User selected an outline style
    SelectOutline(usize),
    /// User cancelled/closed the selector
    Cancel,
}

/// Calculate the size of a single preview cell using the default font
fn cell_size() -> (f32, f32) {
    let font = BitFont::default();
    let font_size = font.size();
    let w = font_size.width as f32 * OUTLINE_WIDTH as f32 + CELL_PADDING_LEFT + CELL_PADDING;
    let h = font_size.height as f32 * OUTLINE_HEIGHT as f32 + 2.0 * CELL_PADDING;
    (w, h)
}

/// Total width of the selector popup
pub fn outline_selector_width() -> f32 {
    let (cell_w, _) = cell_size();
    PER_ROW as f32 * (cell_w + CELL_SPACING) - CELL_SPACING + 2.0 * POPUP_PADDING
}

/// Total height of the selector popup
pub fn outline_selector_height() -> f32 {
    let (_, cell_h) = cell_size();
    let rows = (OUTLINE_STYLES + PER_ROW - 1) / PER_ROW;
    rows as f32 * (cell_h + CELL_SPACING) - CELL_SPACING + 2.0 * POPUP_PADDING
}

/// Outline selector state
pub struct OutlineSelector {
    /// Currently selected outline style
    pub current_style: usize,
}

impl OutlineSelector {
    pub fn new(current_style: usize) -> Self {
        Self { current_style }
    }

    /// Render the outline selector popup
    pub fn view(self) -> Element<'static, OutlineSelectorMessage> {
        Canvas::new(OutlineSelectorProgram {
            current_style: self.current_style,
            font: BitFont::default(),
        })
        .width(Length::Fixed(outline_selector_width()))
        .height(Length::Fixed(outline_selector_height()))
        .into()
    }
}

/// Canvas program for drawing the outline selector
struct OutlineSelectorProgram {
    current_style: usize,
    font: BitFont,
}

impl OutlineSelectorProgram {
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
            x: POPUP_PADDING + col as f32 * (cw + CELL_SPACING),
            y: POPUP_PADDING + row as f32 * (ch + CELL_SPACING),
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
    fn draw_outline_cell(&self, frame: &mut Frame, style: usize, rect: Rectangle, fg: Color, bg: Color) {
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
                let glyph = self.font.glyph(cp437_ch);
                // Draw each pixel of the glyph
                let bitmap_pixels = glyph.to_bitmap_pixels();
                for (y, glyph_row) in bitmap_pixels.iter().enumerate() {
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

/// State tracks hovered style and keyboard cursor
#[derive(Debug, Clone, Default)]
pub struct OutlineSelectorState {
    /// Currently hovered style (from mouse)
    pub hovered: Option<usize>,
    /// Keyboard cursor position
    pub cursor: usize,
}

impl canvas::Program<OutlineSelectorMessage> for OutlineSelectorProgram {
    type State = OutlineSelectorState;

    fn draw(&self, state: &Self::State, renderer: &icy_ui::Renderer, theme: &Theme, bounds: Rectangle, _cursor: Cursor) -> Vec<Geometry> {
        let mut frame = Frame::new(renderer, bounds.size());
        let hovered = state.hovered;
        let keyboard_cursor = state.cursor;

        // Background with dotted pattern (like CharSelector)
        let bg_panel = theme.background.base;
        let dot_color = theme.primary.divider.scale_alpha(0.12);
        frame.fill_rectangle(Point::ORIGIN, bounds.size(), bg_panel);

        // Draw subtle dot pattern in padding area
        let dot_spacing = 8.0;
        let dot_size = 2.0;
        let mut y = dot_spacing / 2.0;
        while y < bounds.height {
            let mut x = dot_spacing / 2.0;
            while x < bounds.width {
                // Only draw dots in the padding area
                let in_grid = x >= POPUP_PADDING && x < bounds.width - POPUP_PADDING && y >= POPUP_PADDING && y < bounds.height - POPUP_PADDING;
                if !in_grid {
                    frame.fill_rectangle(Point::new(x - dot_size / 2.0, y - dot_size / 2.0), Size::new(dot_size, dot_size), dot_color);
                }
                x += dot_spacing;
            }
            y += dot_spacing;
        }

        // Colors derived from theme
        let fg_normal = theme.background.on;
        let bg_normal = theme.secondary.base;
        let bg_hovered = theme.accent.selected.scale_alpha(0.35);
        let bg_selected = theme.accent.base.scale_alpha(0.45);
        let bg_cursor = theme.accent.hover.scale_alpha(0.30);
        let fg_selected = theme.background.on;
        let border_selected = theme.background.on;
        let border_cursor = theme.accent.hover;
        let label_color = theme.secondary.on.scale_alpha(0.75);
        let label_selected = theme.background.on;

        for style in 0..OUTLINE_STYLES {
            let rect = self.cell_rect(style);
            let is_selected = style == self.current_style;
            let is_hovered = hovered == Some(style);
            let is_cursor = style == keyboard_cursor;

            // Determine colors based on state
            let (bg, fg) = if is_selected {
                (bg_selected, fg_selected)
            } else if is_cursor {
                (bg_cursor, fg_normal)
            } else if is_hovered {
                (bg_hovered, fg_normal)
            } else {
                (bg_normal, fg_normal)
            };

            // Draw the cell
            self.draw_outline_cell(&mut frame, style, rect, fg, bg);

            // Draw TheDraw shortcut label in top-left corner
            let shortcut = THEDRAW_SHORTCUTS[style];
            let label_text = canvas::Text {
                content: shortcut.to_string(),
                position: Point::new(rect.x + 3.0, rect.y + 2.0),
                color: if is_selected { label_selected } else { label_color },
                size: icy_ui::Pixels(12.0),
                ..Default::default()
            };
            frame.fill_text(label_text);

            // Draw selection or cursor border
            if is_selected {
                let border = Path::rectangle(Point::new(rect.x, rect.y), rect.size());
                frame.stroke(&border, Stroke::default().with_width(2.0).with_color(border_selected));
            } else if is_cursor {
                let border = Path::rectangle(Point::new(rect.x, rect.y), rect.size());
                frame.stroke(&border, Stroke::default().with_width(1.5).with_color(border_cursor));
            }
        }

        vec![frame.into_geometry()]
    }

    fn update(&self, state: &mut Self::State, event: &icy_ui::Event, bounds: Rectangle, cursor: Cursor) -> Option<Action<OutlineSelectorMessage>> {
        match event {
            icy_ui::Event::Mouse(mouse::Event::CursorMoved { .. }) => {
                let new_hover = cursor.position_in(bounds).and_then(|p| self.hit_test(p));
                if state.hovered != new_hover {
                    state.hovered = new_hover;
                    // Update cursor to hovered position for keyboard continuity
                    if let Some(h) = new_hover {
                        state.cursor = h;
                    }
                    return Some(Action::request_redraw());
                }
                None
            }
            icy_ui::Event::Mouse(mouse::Event::ButtonPressed {
                button: mouse::Button::Left, ..
            }) => {
                let Some(cursor_pos) = cursor.position_in(bounds) else {
                    return None;
                };

                if let Some(style) = self.hit_test(cursor_pos) {
                    return Some(Action::publish(OutlineSelectorMessage::SelectOutline(style)));
                }

                None
            }
            icy_ui::Event::Keyboard(icy_ui::keyboard::Event::KeyPressed { key, modifiers, .. }) => {
                use icy_ui::keyboard::key::Named;
                use icy_ui::keyboard::Key;

                match key {
                    Key::Named(Named::Escape) => {
                        return Some(Action::publish(OutlineSelectorMessage::Cancel));
                    }

                    // Arrow key navigation
                    Key::Named(Named::ArrowLeft) => {
                        if state.cursor > 0 {
                            state.cursor -= 1;
                        } else {
                            state.cursor = OUTLINE_STYLES - 1;
                        }
                        return Some(Action::request_redraw());
                    }
                    Key::Named(Named::ArrowRight) => {
                        if state.cursor < OUTLINE_STYLES - 1 {
                            state.cursor += 1;
                        } else {
                            state.cursor = 0;
                        }
                        return Some(Action::request_redraw());
                    }
                    Key::Named(Named::ArrowUp) => {
                        if state.cursor >= PER_ROW {
                            state.cursor -= PER_ROW;
                        } else {
                            // Wrap to last row
                            let col = state.cursor;
                            let last_row_idx = (OUTLINE_STYLES - 1) / PER_ROW * PER_ROW + col;
                            state.cursor = last_row_idx.min(OUTLINE_STYLES - 1);
                        }
                        return Some(Action::request_redraw());
                    }
                    Key::Named(Named::ArrowDown) => {
                        if state.cursor + PER_ROW < OUTLINE_STYLES {
                            state.cursor += PER_ROW;
                        } else {
                            // Wrap to first row
                            let col = state.cursor % PER_ROW;
                            state.cursor = col;
                        }
                        return Some(Action::request_redraw());
                    }

                    // Home/End navigation
                    Key::Named(Named::Home) => {
                        if modifiers.control() {
                            state.cursor = 0;
                        } else {
                            // Start of row
                            let row = state.cursor / PER_ROW;
                            state.cursor = row * PER_ROW;
                        }
                        return Some(Action::request_redraw());
                    }
                    Key::Named(Named::End) => {
                        if modifiers.control() {
                            state.cursor = OUTLINE_STYLES - 1;
                        } else {
                            // End of row
                            let row = state.cursor / PER_ROW;
                            state.cursor = ((row + 1) * PER_ROW - 1).min(OUTLINE_STYLES - 1);
                        }
                        return Some(Action::request_redraw());
                    }

                    // Select with Enter or Space
                    Key::Named(Named::Enter) | Key::Named(Named::Space) => {
                        return Some(Action::publish(OutlineSelectorMessage::SelectOutline(state.cursor)));
                    }

                    // Handle A-S keys for quick selection (TheDraw shortcuts)
                    Key::Character(c) => {
                        let ch = c.to_uppercase().to_string();
                        if let Some(idx) = THEDRAW_SHORTCUTS.iter().position(|&s| s == ch) {
                            return Some(Action::publish(OutlineSelectorMessage::SelectOutline(idx)));
                        }
                    }

                    _ => {}
                }
                None
            }
            icy_ui::Event::Mouse(mouse::Event::CursorLeft) => {
                if state.hovered.is_some() {
                    state.hovered = None;
                    return Some(Action::request_redraw());
                }
                None
            }
            _ => None,
        }
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
