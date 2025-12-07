//! Color switcher component
//!
//! Shows foreground/background color rectangles with swap functionality.
//! Ported from egui palette_switcher.

use iced::{
    Color, Element, Length, Point, Rectangle, Size, Theme,
    mouse, widget::canvas::{Canvas, Frame, Geometry, Path, Program, Stroke, Action},
};
use icy_engine::{Palette, TextAttribute};

/// Size of the color switcher widget (horizontal version for toolbar)
pub const SWITCHER_WIDTH: f32 = 70.0;
pub const SWITCHER_HEIGHT: f32 = 32.0;

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
            foreground: 7,  // Light gray
            background: 0,  // Black
            cached_palette: Palette::dos_default(),
        }
    }

    /// Update colors from attribute
    pub fn set_from_attribute(&mut self, attr: &TextAttribute) {
        self.foreground = attr.get_foreground();
        self.background = attr.get_background();
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
        // Only clone if different
        if self.cached_palette.len() != palette.len() {
            self.cached_palette = palette.clone();
        }
    }

    /// Render the color switcher (horizontal layout for toolbar)
    pub fn view(&self) -> Element<'_, ColorSwitcherMessage> {
        Canvas::new(ColorSwitcherProgram {
            foreground: self.foreground,
            background: self.background,
            cached_palette: self.cached_palette.clone(),
        })
        .width(Length::Fixed(SWITCHER_WIDTH))
        .height(Length::Fixed(SWITCHER_HEIGHT))
        .into()
    }
}

/// Canvas program for drawing the color switcher
struct ColorSwitcherProgram {
    foreground: u32,
    background: u32,
    cached_palette: Palette,
}

impl Program<ColorSwitcherMessage> for ColorSwitcherProgram {
    type State = ();

    fn draw(
        &self,
        _state: &Self::State,
        renderer: &iced::Renderer,
        _theme: &Theme,
        bounds: Rectangle,
        _cursor: mouse::Cursor,
    ) -> Vec<Geometry> {
        let mut frame = Frame::new(renderer, bounds.size());
        
        // Horizontal layout: [FG rect][BG rect][swap icon]
        let rect_size = SWITCHER_HEIGHT - 4.0;
        let spacing = 2.0;
        
        // Foreground color rectangle (left)
        let fg_x = 2.0;
        let fg_y = 2.0;
        
        // Black border
        frame.fill_rectangle(
            Point::new(fg_x, fg_y),
            Size::new(rect_size, rect_size),
            Color::BLACK,
        );
        
        // White inner border
        frame.fill_rectangle(
            Point::new(fg_x + 1.0, fg_y + 1.0),
            Size::new(rect_size - 2.0, rect_size - 2.0),
            Color::WHITE,
        );
        
        // Foreground color fill
        let (r, g, b) = self.cached_palette.get_rgb(self.foreground);
        frame.fill_rectangle(
            Point::new(fg_x + 2.0, fg_y + 2.0),
            Size::new(rect_size - 4.0, rect_size - 4.0),
            Color::from_rgb8(r, g, b),
        );

        // Background color rectangle (right of foreground)
        let bg_x = fg_x + rect_size + spacing;
        let bg_y = 2.0;
        
        // Black border
        frame.fill_rectangle(
            Point::new(bg_x, bg_y),
            Size::new(rect_size, rect_size),
            Color::BLACK,
        );
        
        // White inner border
        frame.fill_rectangle(
            Point::new(bg_x + 1.0, bg_y + 1.0),
            Size::new(rect_size - 2.0, rect_size - 2.0),
            Color::WHITE,
        );
        
        // Background color fill
        let (r, g, b) = self.cached_palette.get_rgb(self.background);
        frame.fill_rectangle(
            Point::new(bg_x + 2.0, bg_y + 2.0),
            Size::new(rect_size - 4.0, rect_size - 4.0),
            Color::from_rgb8(r, g, b),
        );

        // Swap icon (double arrow between the boxes)
        let swap_x = bg_x + rect_size + spacing + 2.0;
        let swap_y = SWITCHER_HEIGHT / 2.0;
        let arrow_len = 8.0;
        
        // Draw swap arrows (horizontal double arrow)
        let arrow_path = Path::new(|builder| {
            // Left arrow
            builder.move_to(Point::new(swap_x + arrow_len, swap_y - 3.0));
            builder.line_to(Point::new(swap_x, swap_y));
            builder.line_to(Point::new(swap_x + arrow_len, swap_y + 3.0));
            
            // Right arrow
            builder.move_to(Point::new(swap_x + arrow_len + 4.0, swap_y - 3.0));
            builder.line_to(Point::new(swap_x + arrow_len * 2.0 + 4.0, swap_y));
            builder.line_to(Point::new(swap_x + arrow_len + 4.0, swap_y + 3.0));
        });
        
        frame.stroke(
            &arrow_path,
            Stroke::default()
                .with_color(Color::from_rgb8(180, 180, 180))
                .with_width(1.5),
        );

        vec![frame.into_geometry()]
    }

    fn update(
        &self,
        _state: &mut Self::State,
        event: &iced::Event,
        bounds: Rectangle,
        cursor: mouse::Cursor,
    ) -> Option<Action<ColorSwitcherMessage>> {
        let rect_size = SWITCHER_HEIGHT - 4.0;
        let spacing = 2.0;
        let swap_x = 2.0 + rect_size + spacing + rect_size + spacing;

        if let iced::Event::Mouse(mouse::Event::ButtonPressed(mouse::Button::Left)) = event {
            if let Some(pos) = cursor.position_in(bounds) {
                // Check if clicked on swap area (right side)
                if pos.x > swap_x {
                    return Some(Action::publish(ColorSwitcherMessage::SwapColors));
                }
            }
        }

        None
    }
}
