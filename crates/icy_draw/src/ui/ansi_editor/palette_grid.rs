//! Palette grid component
//!
//! Shows all palette colors in a grid with FG/BG markers.
//! Supports 16, 64, and 256 color palettes.
//! Ported from egui palette_editor_16.

use iced::{
    Color, Element, Length, Point, Rectangle, Size, Theme,
    mouse, widget::canvas::{self, Canvas, Frame, Geometry, Path, Program, Stroke},
};
use icy_engine::{IceMode, FontMode, Palette};

/// Messages from the palette grid
#[derive(Clone, Debug)]
pub enum PaletteGridMessage {
    /// Set foreground color (left click)
    SetForeground(u32),
    /// Set background color (right click)
    SetBackground(u32),
}

/// Palette grid state
pub struct PaletteGrid {
    /// Current foreground color index
    foreground: u32,
    /// Current background color index
    background: u32,
    /// ICE mode for high background colors
    ice_mode: IceMode,
    /// Font mode for high foreground colors
    font_mode: FontMode,
    /// Cached palette for rendering
    cached_palette: Palette,
}

impl Default for PaletteGrid {
    fn default() -> Self {
        Self::new()
    }
}

impl PaletteGrid {
    pub fn new() -> Self {
        Self {
            foreground: 7,
            background: 0,
            ice_mode: IceMode::Unlimited,
            font_mode: FontMode::Unlimited,
            cached_palette: Palette::dos_default(),
        }
    }

    /// Set foreground color
    pub fn set_foreground(&mut self, color: u32) {
        self.foreground = color;
    }

    /// Set background color
    pub fn set_background(&mut self, color: u32) {
        self.background = color;
    }

    /// Set ICE mode
    pub fn set_ice_mode(&mut self, mode: IceMode) {
        self.ice_mode = mode;
    }

    /// Set font mode
    pub fn set_font_mode(&mut self, mode: FontMode) {
        self.font_mode = mode;
    }

    /// Sync palette from edit state
    pub fn sync_palette(&mut self, palette: &Palette) {
        // Only clone if different
        if self.cached_palette.len() != palette.len() {
            self.cached_palette = palette.clone();
        }
    }

    /// Get the width for the cached palette
    pub fn cached_palette_width(&self) -> f32 {
        Self::required_width(&self.cached_palette)
    }

    /// Calculate the optimal layout for the palette
    /// Returns (items_per_row, cell_size, total_width, total_height)
    fn calculate_layout(palette_len: usize, _max_width: f32) -> (usize, f32, f32, f32) {
        // For small palettes (≤16), use vertical layout (1 or 2 columns)
        // For larger palettes, use more columns
        let (items_per_row, cell_size) = if palette_len <= 8 {
            // 1 column, larger cells
            (1, 24.0)
        } else if palette_len <= 16 {
            // 2 columns  
            (2, 20.0)
        } else if palette_len <= 64 {
            // 4 columns
            (4, 16.0)
        } else {
            // 8 columns for 256 colors
            (8, 12.0)
        };
        
        let rows = (palette_len as f32 / items_per_row as f32).ceil() as usize;
        let width = cell_size * items_per_row as f32;
        let height = cell_size * rows as f32;
        
        (items_per_row, cell_size, width, height)
    }

    /// Get the required width for this palette
    pub fn required_width(palette: &Palette) -> f32 {
        let (_, _, width, _) = Self::calculate_layout(palette.len(), 200.0);
        width + 8.0 // Add padding
    }

    /// Render the palette grid (vertical layout for small palettes)
    /// Call sync_palette before this to ensure the cached palette is up to date
    pub fn view(&self) -> Element<'_, PaletteGridMessage> {
        let (items_per_row, cell_size, width, height) = Self::calculate_layout(self.cached_palette.len(), 200.0);

        Canvas::new(PaletteGridProgram {
            foreground: self.foreground,
            background: self.background,
            ice_mode: self.ice_mode,
            font_mode: self.font_mode,
            cached_palette: self.cached_palette.clone(),
            items_per_row,
            cell_size,
        })
        .width(Length::Fixed(width))
        .height(Length::Fixed(height))
        .into()
    }
}

/// Canvas program for drawing the palette grid
struct PaletteGridProgram {
    foreground: u32,
    background: u32,
    ice_mode: IceMode,
    font_mode: FontMode,
    cached_palette: Palette,
    items_per_row: usize,
    cell_size: f32,
}

impl Program<PaletteGridMessage> for PaletteGridProgram {
    type State = Option<u32>; // Hovered color index

    fn draw(
        &self,
        _state: &Self::State,
        renderer: &iced::Renderer,
        _theme: &Theme,
        bounds: Rectangle,
        _cursor: mouse::Cursor,
    ) -> Vec<Geometry> {
        let mut frame = Frame::new(renderer, bounds.size());
        
        let cell_size = self.cell_size;
        let upper_limit = ((self.cached_palette.len() as f32 / self.items_per_row as f32).ceil() as usize) 
            * self.items_per_row;

        // Draw color cells
        for i in 0..upper_limit.min(self.cached_palette.len()) {
            let col = i % self.items_per_row;
            let row = i / self.items_per_row;
            let x = col as f32 * cell_size;
            let y = row as f32 * cell_size;

            let (r, g, b) = self.cached_palette.get_rgb(i as u32);
            frame.fill_rectangle(
                Point::new(x, y),
                Size::new(cell_size, cell_size),
                Color::from_rgb8(r, g, b),
            );
        }

        // Draw foreground marker (triangle top-left)
        let marker_len = cell_size / 3.0;
        let fg_col = self.foreground as usize % self.items_per_row;
        let fg_row = self.foreground as usize / self.items_per_row;
        let fg_origin = Point::new(fg_col as f32 * cell_size, fg_row as f32 * cell_size);

        let fg_marker = Path::new(|builder| {
            builder.move_to(fg_origin);
            builder.line_to(Point::new(fg_origin.x + marker_len, fg_origin.y));
            builder.line_to(Point::new(fg_origin.x, fg_origin.y + marker_len));
            builder.close();
        });
        
        frame.fill(&fg_marker, Color::WHITE);
        frame.stroke(
            &fg_marker,
            Stroke::default().with_color(Color::from_rgb8(128, 128, 128)).with_width(1.0),
        );

        // Draw background marker (triangle bottom-right)
        let bg_col = self.background as usize % self.items_per_row;
        let bg_row = self.background as usize / self.items_per_row;
        let bg_origin = Point::new(
            (bg_col + 1) as f32 * cell_size,
            (bg_row + 1) as f32 * cell_size,
        );

        let bg_marker = Path::new(|builder| {
            builder.move_to(bg_origin);
            builder.line_to(Point::new(bg_origin.x - marker_len, bg_origin.y));
            builder.line_to(Point::new(bg_origin.x, bg_origin.y - marker_len));
            builder.close();
        });
        
        frame.fill(&bg_marker, Color::WHITE);
        frame.stroke(
            &bg_marker,
            Stroke::default().with_color(Color::from_rgb8(128, 128, 128)).with_width(1.0),
        );

        vec![frame.into_geometry()]
    }

    fn update(
        &self,
        state: &mut Self::State,
        event: &iced::Event,
        bounds: Rectangle,
        cursor: mouse::Cursor,
    ) -> Option<canvas::Action<PaletteGridMessage>> {
        let cell_size = self.cell_size;

        match event {
            iced::Event::Mouse(mouse::Event::CursorMoved { .. }) => {
                if let Some(pos) = cursor.position_in(bounds) {
                    let col = (pos.x / cell_size) as u32;
                    let row = (pos.y / cell_size) as u32;
                    let color = col + row * self.items_per_row as u32;
                    if color < self.cached_palette.len() as u32 {
                        *state = Some(color);
                    } else {
                        *state = None;
                    }
                }
                None
            }
            iced::Event::Mouse(mouse::Event::ButtonPressed(button)) => {
                if let Some(pos) = cursor.position_in(bounds) {
                    let col = (pos.x / cell_size) as u32;
                    let row = (pos.y / cell_size) as u32;
                    let color = col + row * self.items_per_row as u32;
                    
                    if color < self.cached_palette.len() as u32 {
                        match button {
                            mouse::Button::Left => {
                                // Check if high foreground color is allowed
                                if color < 8 || self.font_mode.has_high_fg_colors() || self.cached_palette.len() > 16 {
                                    return Some(canvas::Action::publish(
                                        PaletteGridMessage::SetForeground(color),
                                    ));
                                }
                            }
                            mouse::Button::Right => {
                                // Check if high background color is allowed
                                if color < 8 || self.ice_mode.has_high_bg_colors() || self.cached_palette.len() > 16 {
                                    return Some(canvas::Action::publish(
                                        PaletteGridMessage::SetBackground(color),
                                    ));
                                }
                            }
                            _ => {}
                        }
                    }
                }
                None
            }
            _ => None,
        }
    }
}
