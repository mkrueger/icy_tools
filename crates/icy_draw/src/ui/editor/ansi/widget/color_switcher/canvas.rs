//! Color switcher component
//!
//! Shows foreground/background color rectangles with swap functionality.
//! Ported from egui palette_switcher.
//!
//! Layout (like Photoshop color picker):
//! - Large foreground rectangle (top-left, overlapping)
//! - Large background rectangle (bottom-right, behind foreground)
//! - Small default color rectangles (bottom-left corner)
//! - Swap icon (top-right corner)
//!
//! NOTE: Some helper functions are defined for future use.

#![allow(dead_code)]

use iced::{
    mouse,
    widget::canvas::{Action, Canvas, Frame, Geometry, Path, Program, Stroke},
    Color, Element, Length, Point, Rectangle, Size, Theme,
};
use icy_engine::{Palette, TextAttribute};

/// Size of the color switcher widget (square like in egui version)
pub const SWITCHER_SIZE: f32 = 62.0;
/// Messages from the color switcher
#[derive(Clone, Debug)]
pub enum ColorSwitcherMessage {
    /// Swap foreground and background colors
    SwapColors,
    /// Reset to default colors (white on black)
    ResetToDefault,
}

/// Color switcher state
pub struct ColorSwitcher {
    /// Current foreground color index
    foreground: u32,
    /// Current background color index
    background: u32,
    /// Cached palette for rendering
    cached_palette: Palette,
}

impl Default for ColorSwitcher {
    fn default() -> Self {
        Self::new()
    }
}

impl ColorSwitcher {
    pub fn new() -> Self {
        Self {
            foreground: 7, // Light gray
            background: 0, // Black
            cached_palette: Palette::dos_default(),
        }
    }

    /// Update colors from attribute
    pub fn set_from_attribute(&mut self, attr: &TextAttribute) {
        self.foreground = attr.foreground();
        self.background = attr.background();
    }

    /// Get foreground color
    pub fn foreground(&self) -> u32 {
        self.foreground
    }

    /// Get background color
    pub fn background(&self) -> u32 {
        self.background
    }

    /// Sync palette from edit state
    pub fn sync_palette(&mut self, palette: &Palette) {
        self.cached_palette = palette.clone();
    }

    /// Render the color switcher with the given foreground and background colors
    pub fn view(&self, foreground: u32, background: u32) -> Element<'_, ColorSwitcherMessage> {
        Canvas::new(ColorSwitcherProgram {
            foreground,
            background,
            cached_palette: self.cached_palette.clone(),
        })
        .width(Length::Fixed(SWITCHER_SIZE))
        .height(Length::Fixed(SWITCHER_SIZE))
        .into()
    }
}

/// Canvas program for drawing the color switcher
struct ColorSwitcherProgram {
    foreground: u32,
    background: u32,
    cached_palette: Palette,
}

impl ColorSwitcherProgram {
    /// Draw a color rectangle with black and white borders
    fn draw_color_rect(frame: &mut Frame, x: f32, y: f32, size: f32, color: Color) {
        // Black outer border
        frame.fill_rectangle(Point::new(x, y), Size::new(size, size), Color::BLACK);
        // White inner border
        frame.fill_rectangle(Point::new(x + 1.0, y + 1.0), Size::new(size - 2.0, size - 2.0), Color::WHITE);
        // Color fill
        frame.fill_rectangle(Point::new(x + 2.0, y + 2.0), Size::new(size - 4.0, size - 4.0), color);
    }

    /// Draw a small default color rectangle
    fn draw_small_rect(frame: &mut Frame, x: f32, y: f32, size: f32, color: Color) {
        let [r, g, b, _] = color.into_rgba8();
        // Inverted color border
        frame.fill_rectangle(Point::new(x, y), Size::new(size, size), Color::from_rgb8(r ^ 0xFF, g ^ 0xFF, b ^ 0xFF));
        // Color fill
        frame.fill_rectangle(Point::new(x + 1.0, y + 1.0), Size::new(size - 2.0, size - 2.0), color);
    }
}

impl Program<ColorSwitcherMessage> for ColorSwitcherProgram {
    type State = ();

    fn draw(&self, _state: &Self::State, renderer: &iced::Renderer, theme: &Theme, bounds: Rectangle, _cursor: mouse::Cursor) -> Vec<Geometry> {
        let mut frame = Frame::new(renderer, bounds.size());
        let height = SWITCHER_SIZE;

        // Main color rectangle size (golden ratio based)
        let rect_height = height * 0.618;

        // Background color rectangle (bottom-right, drawn first so FG overlaps)
        let (r, g, b) = self.cached_palette.rgb(self.background);
        Self::draw_color_rect(&mut frame, height - rect_height, height - rect_height, rect_height, Color::from_rgb8(r, g, b));

        // Foreground color rectangle (top-left, overlaps background)
        let (r, g, b) = self.cached_palette.rgb(self.foreground);
        Self::draw_color_rect(&mut frame, 0.0, 0.0, rect_height, Color::from_rgb8(r, g, b));

        // Small default color rectangles (bottom-left corner)
        let s_rect_height = height * 0.382;
        let rh = s_rect_height / 1.8;
        let overlap = 2.0;

        // Default foreground (white/color 7) - bottom small rect
        let (r, g, b) = self.cached_palette.rgb(7);
        Self::draw_small_rect(&mut frame, rh - overlap, height - rh - overlap, rh, Color::from_rgb8(r, g, b));

        // Default background (black/color 0) - top small rect
        let (r, g, b) = self.cached_palette.rgb(0);
        Self::draw_small_rect(&mut frame, overlap, height - 2.0 * rh + 2.0 + overlap, rh, Color::from_rgb8(r, g, b));

        // Swap icon (top-right corner) - draw arrows
        let swap_x = rect_height + 4.0;
        let swap_y = 2.0;
        let arrow_size = s_rect_height * 0.6;

        // Draw curved double arrow for swap
        let arrow_path = Path::new(|builder| {
            let cx = swap_x + arrow_size / 2.0;
            let cy = swap_y + arrow_size / 2.0;
            let r = arrow_size / 3.0;

            // Upper-right arrow (curved)
            builder.move_to(Point::new(cx + r, cy - r * 0.5));
            builder.line_to(Point::new(cx + r + 3.0, cy - r * 0.5 - 3.0));
            builder.move_to(Point::new(cx + r, cy - r * 0.5));
            builder.line_to(Point::new(cx + r - 3.0, cy - r * 0.5 - 3.0));

            // Arc line
            builder.move_to(Point::new(cx - r, cy + r * 0.5));
            builder.line_to(Point::new(cx, cy));
            builder.line_to(Point::new(cx + r, cy - r * 0.5));

            // Lower-left arrow
            builder.move_to(Point::new(cx - r, cy + r * 0.5));
            builder.line_to(Point::new(cx - r - 3.0, cy + r * 0.5 + 3.0));
            builder.move_to(Point::new(cx - r, cy + r * 0.5));
            builder.line_to(Point::new(cx - r + 3.0, cy + r * 0.5 + 3.0));
        });

        let arrow_color = theme.extended_palette().background.base.text;
        frame.stroke(&arrow_path, Stroke::default().with_color(arrow_color).with_width(1.5));

        vec![frame.into_geometry()]
    }

    fn update(&self, _state: &mut Self::State, event: &iced::Event, bounds: Rectangle, cursor: mouse::Cursor) -> Option<Action<ColorSwitcherMessage>> {
        let height = SWITCHER_SIZE;
        let rect_height = height * 0.618;

        if let iced::Event::Mouse(mouse::Event::ButtonPressed { button: mouse::Button::Left, .. }) = event {
            if let Some(pos) = cursor.position_in(bounds) {
                // Check if clicked on swap area (top-right)
                if pos.x > rect_height && pos.y < rect_height {
                    return Some(Action::publish(ColorSwitcherMessage::SwapColors));
                }

                // Check if clicked on default colors area (bottom-left)
                if pos.x < rect_height && pos.y > rect_height {
                    return Some(Action::publish(ColorSwitcherMessage::ResetToDefault));
                }
            }
        }

        None
    }
}
