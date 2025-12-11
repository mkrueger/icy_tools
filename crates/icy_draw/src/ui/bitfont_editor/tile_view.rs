//! Tile view canvas for BitFont editor

use iced::{
    Color, Point, Rectangle, Size,
    mouse::Cursor,
    widget::canvas::{self, Frame},
};

use super::{BitFontEditor, BitFontEditorMessage};

/// Canvas for the tile view (8x8 grid of the current character)
pub struct TileViewCanvas<'a> {
    pub editor: &'a BitFontEditor,
    pub fg_color: u32,
    pub bg_color: u32,
    pub cell_width: f32,
    pub cell_height: f32,
    pub grid_size: i32,
}

impl<'a> canvas::Program<BitFontEditorMessage> for TileViewCanvas<'a> {
    type State = ();

    fn draw(&self, _state: &Self::State, renderer: &iced::Renderer, _theme: &iced::Theme, bounds: Rectangle, _cursor: Cursor) -> Vec<canvas::Geometry> {
        let mut frame = Frame::new(renderer, bounds.size());

        // Get colors from palette
        let palette = icy_engine::Palette::dos_default();
        let (fg_r, fg_g, fg_b) = palette.rgb(self.fg_color);
        let (bg_r, bg_g, bg_b) = palette.rgb(self.bg_color);
        let fg_iced_color = Color::from_rgb8(fg_r, fg_g, fg_b);
        let bg_iced_color = Color::from_rgb8(bg_r, bg_g, bg_b);

        let (width, height) = self.editor.font_size();
        let selected_char = self.editor.selected_char();
        let pixels = self.editor.get_glyph_pixels(selected_char);

        // Check if we're in 9-dot mode
        let use_9dot = self.editor.use_letter_spacing() && width == 8;
        let display_width = if use_9dot { width + 1 } else { width };

        // Check if this is a box-drawing character (for 9-dot mode)
        let char_code = selected_char as u32;
        let is_box_drawing = (0xC0..=0xDF).contains(&char_code);

        // Fill background
        frame.fill_rectangle(Point::ORIGIN, bounds.size(), bg_iced_color);

        // Calculate pixel scale factor
        let pixel_scale_x = self.cell_width / display_width as f32;
        let pixel_scale_y = self.cell_height / height as f32;

        // Draw the current character in an 8x8 grid
        for tile_row in 0..self.grid_size {
            for tile_col in 0..self.grid_size {
                let x = tile_col as f32 * self.cell_width;
                let y = tile_row as f32 * self.cell_height;

                // Draw pixels (scaled)
                for py in 0..height as usize {
                    for px in 0..width as usize {
                        let is_set = pixels.get(py).and_then(|r| r.get(px)).copied().unwrap_or(false);
                        if is_set {
                            frame.fill_rectangle(
                                Point::new(x + px as f32 * pixel_scale_x, y + py as f32 * pixel_scale_y),
                                Size::new(pixel_scale_x, pixel_scale_y),
                                fg_iced_color,
                            );
                        }
                    }

                    // Draw 9th column if in 9-dot mode
                    if use_9dot {
                        let pixel_8 = pixels.get(py).and_then(|r| r.get(7)).copied().unwrap_or(false);
                        let is_set = is_box_drawing && pixel_8;
                        if is_set {
                            frame.fill_rectangle(
                                Point::new(x + 8.0 * pixel_scale_x, y + py as f32 * pixel_scale_y),
                                Size::new(pixel_scale_x, pixel_scale_y),
                                fg_iced_color,
                            );
                        }
                    }
                }
            }
        }

        vec![frame.into_geometry()]
    }
}
