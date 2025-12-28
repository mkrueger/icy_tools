//! Character Selector Popup
//!
//! A 16x16 grid of characters that can be selected with the mouse.
//! Used for customizing F-key character assignments.

use iced::{
    mouse::{self, Cursor},
    widget::{
        canvas::{self, Canvas, Frame, Geometry},
        Action,
    },
    Color, Element, Length, Point, Rectangle, Size, Theme,
};
use icy_engine::{BitFont, Palette};

/// Size of each character cell in the grid
const CELL_SIZE: f32 = 28.0;

/// Grid dimensions
const GRID_WIDTH: usize = 16;
const GRID_HEIGHT: usize = 16;

/// Uniform padding around the grid
const PADDING: f32 = 12.0;

/// Total width of the selector
pub const CHAR_SELECTOR_WIDTH: f32 = CELL_SIZE * GRID_WIDTH as f32 + PADDING * 2.0;

/// Total height of the selector
pub const CHAR_SELECTOR_HEIGHT: f32 = CELL_SIZE * GRID_HEIGHT as f32 + PADDING * 2.0;

/// Messages from the character selector
#[derive(Clone, Debug)]
pub enum CharSelectorMessage {
    /// User selected a character code
    SelectChar(u16),
    /// User cancelled/closed the selector
    Cancel,
}

/// Character selector state
pub struct CharSelector {
    /// Currently selected character code
    pub current_code: u16,
}

impl CharSelector {
    pub fn new(current_code: u16) -> Self {
        Self { current_code }
    }

    /// Render the character selector popup
    pub fn view(self, font: Option<BitFont>, palette: Palette, fg_color: u32, bg_color: u32) -> Element<'static, CharSelectorMessage> {
        Canvas::new(CharSelectorProgram {
            font,
            palette,
            fg_color,
            bg_color,
            current_code: self.current_code,
        })
        .width(Length::Fixed(CHAR_SELECTOR_WIDTH))
        .height(Length::Fixed(CHAR_SELECTOR_HEIGHT))
        .into()
    }
}

/// Canvas program for drawing the character selector
struct CharSelectorProgram {
    font: Option<BitFont>,
    palette: Palette,
    fg_color: u32,
    bg_color: u32,
    current_code: u16,
}

impl CharSelectorProgram {
    fn blend(a: Color, b: Color, t: f32) -> Color {
        let t = t.clamp(0.0, 1.0);
        Color {
            r: a.r + (b.r - a.r) * t,
            g: a.g + (b.g - a.g) * t,
            b: a.b + (b.b - a.b) * t,
            a: a.a + (b.a - a.a) * t,
        }
    }

    /// Get the character code at a grid position
    fn code_at_pos(&self, cursor_pos: Point) -> Option<u16> {
        let x = cursor_pos.x - PADDING;
        let y = cursor_pos.y - PADDING;

        if x < 0.0 || y < 0.0 {
            return None;
        }

        let col = (x / CELL_SIZE) as usize;
        let row = (y / CELL_SIZE) as usize;

        if col < GRID_WIDTH && row < GRID_HEIGHT {
            Some((row * GRID_WIDTH + col) as u16)
        } else {
            None
        }
    }

    /// Draw a single glyph from the font
    fn draw_glyph(&self, frame: &mut Frame, x: f32, y: f32, ch: char, fg: Color, bg: Color, scale: f32) {
        let Some(font) = &self.font else {
            // Fallback: draw simple rectangle
            frame.fill_rectangle(Point::new(x, y), Size::new(CELL_SIZE, CELL_SIZE), bg);
            return;
        };

        let font_width = font.size().width as f32;
        let font_height = font.size().height as f32;
        let char_width = font_width * scale;
        let char_height = font_height * scale;

        // Center the glyph in the cell
        let offset_x = (CELL_SIZE - char_width) / 2.0;
        let offset_y = (CELL_SIZE - char_height) / 2.0;

        // Fill background for the entire cell
        frame.fill_rectangle(Point::new(x, y), Size::new(CELL_SIZE, CELL_SIZE), bg);

        // Get glyph and draw pixels (centered)
        let glyph = font.glyph(ch);
        let pixel_w = scale;
        let pixel_h = scale;

        let bitmap_pixels = glyph.to_bitmap_pixels();
        for (row_idx, row) in bitmap_pixels.iter().enumerate() {
            for (col_idx, &pixel) in row.iter().enumerate() {
                if pixel {
                    frame.fill_rectangle(
                        Point::new(x + offset_x + col_idx as f32 * pixel_w, y + offset_y + row_idx as f32 * pixel_h),
                        Size::new(pixel_w, pixel_h),
                        fg,
                    );
                }
            }
        }
    }
}

/// State tracks hovered character code
type HoverState = Option<u16>;

impl canvas::Program<CharSelectorMessage> for CharSelectorProgram {
    type State = HoverState;

    fn draw(&self, state: &Self::State, renderer: &iced::Renderer, theme: &Theme, bounds: Rectangle, _cursor: Cursor) -> Vec<Geometry> {
        let mut frame = Frame::new(renderer, bounds.size());

        // Background with dotted pattern
        let palette = theme.extended_palette();
        let bg_panel = palette.background.base.color;
        let dot_color = palette.background.strong.color.scale_alpha(0.12);
        frame.fill_rectangle(Point::ORIGIN, bounds.size(), bg_panel);

        // Draw subtle dot pattern in padding area
        let dot_spacing = 8.0;
        let dot_size = 2.0;
        let mut y = dot_spacing / 2.0;
        while y < bounds.height {
            let mut x = dot_spacing / 2.0;
            while x < bounds.width {
                // Only draw dots in the padding area (not over the char grid)
                let in_grid_x = x >= PADDING && x < bounds.width - PADDING;
                let in_grid_y = y >= PADDING && y < bounds.height - PADDING;
                if !(in_grid_x && in_grid_y) {
                    frame.fill_rectangle(Point::new(x - dot_size / 2.0, y - dot_size / 2.0), Size::new(dot_size, dot_size), dot_color);
                }
                x += dot_spacing;
            }
            y += dot_spacing;
        }

        // Colors from palette
        let (fg_r, fg_g, fg_b) = self.palette.rgb(self.fg_color);
        let (bg_r, bg_g, bg_b) = self.palette.rgb(self.bg_color);
        let fg = Color::from_rgb8(fg_r, fg_g, fg_b);
        let bg = Color::from_rgb8(bg_r, bg_g, bg_b);
        let hover_tint = palette.primary.weak.color;
        let selected_border = palette.background.base.text;

        // Calculate scale to fit cells
        let font_height = self.font.as_ref().map(|f| f.size().height as f32).unwrap_or(16.0);
        let scale = CELL_SIZE / font_height;

        let hovered_code = *state;

        // Draw all 256 characters in a 16x16 grid
        for row in 0..GRID_HEIGHT {
            for col in 0..GRID_WIDTH {
                let code = (row * GRID_WIDTH + col) as u16;
                let ch = char::from_u32(code as u32).unwrap_or(' ');

                let x = PADDING + col as f32 * CELL_SIZE;
                let y = PADDING + row as f32 * CELL_SIZE;

                let is_hovered = hovered_code == Some(code);
                let is_current = code == self.current_code;

                // Draw character with appropriate background
                let cell_bg = if is_hovered { Self::blend(bg, hover_tint, 0.35) } else { bg };
                self.draw_glyph(&mut frame, x, y, ch, fg, cell_bg, scale);

                // Draw selection border for current character
                if is_current {
                    use iced::widget::canvas::{Path, Stroke};
                    let rect_path = Path::rectangle(Point::new(x, y), Size::new(CELL_SIZE, CELL_SIZE));
                    frame.stroke(&rect_path, Stroke::default().with_color(selected_border).with_width(2.0));
                }
            }
        }

        vec![frame.into_geometry()]
    }

    fn update(&self, state: &mut Self::State, event: &iced::Event, bounds: Rectangle, cursor: Cursor) -> Option<Action<CharSelectorMessage>> {
        match event {
            iced::Event::Mouse(mouse::Event::CursorMoved { .. }) => {
                let new_hover = cursor.position_in(bounds).and_then(|p| self.code_at_pos(p));
                if *state != new_hover {
                    *state = new_hover;
                    return Some(Action::request_redraw());
                }
                None
            }
            iced::Event::Mouse(mouse::Event::ButtonPressed { button: mouse::Button::Left, .. }) => {
                let Some(cursor_pos) = cursor.position_in(bounds) else {
                    return None;
                };

                if let Some(code) = self.code_at_pos(cursor_pos) {
                    return Some(Action::publish(CharSelectorMessage::SelectChar(code)));
                }

                None
            }
            iced::Event::Keyboard(iced::keyboard::Event::KeyPressed { key, .. }) => {
                use iced::keyboard::Key;
                if matches!(key, Key::Named(iced::keyboard::key::Named::Escape)) {
                    return Some(Action::publish(CharSelectorMessage::Cancel));
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
            _ => None,
        }
    }
}
