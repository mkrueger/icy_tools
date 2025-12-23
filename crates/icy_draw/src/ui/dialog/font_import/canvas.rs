//! Font preview canvas for the import dialog
//!
//! Similar to the CharSetCanvas in the bitfont editor

use iced::{
    mouse::Cursor,
    widget::canvas::{self, Frame},
    Point, Rectangle, Size,
};

use crate::ui::editor::bitfont::style::{draw_rulers, RulerState};

/// Canvas for previewing a font in the import dialog (16x16 grid)
pub struct FontPreviewCanvas<'a> {
    pub font: &'a icy_engine::BitFont,
    pub cell_width: f32,
    pub cell_height: f32,
    pub label_size: f32,
}

impl<'a, Message> canvas::Program<Message> for FontPreviewCanvas<'a> {
    type State = ();

    fn draw(&self, _state: &Self::State, renderer: &iced::Renderer, theme: &iced::Theme, bounds: Rectangle, _cursor: Cursor) -> Vec<canvas::Geometry> {
        let mut frame = Frame::new(renderer, bounds.size());

        // Colors
        let palette = theme.extended_palette();
        let fg_color = palette.background.base.text;
        let bg_color = palette.background.weak.color;

        let (font_width, font_height) = (self.font.size().width, self.font.size().height);

        // Fill background
        frame.fill_rectangle(Point::ORIGIN, bounds.size(), bg_color);

        // Draw rulers (simplified, not focused)
        let ruler_state = RulerState::new(false, -1, -1, 16, 16, self.label_size, self.cell_width, self.cell_height, bounds.size());
        draw_rulers(&mut frame, &ruler_state, theme);

        // Draw all 256 characters
        for ch_code in 0..256u32 {
            let row = (ch_code / 16) as i32;
            let col = (ch_code % 16) as i32;
            let x = self.label_size + col as f32 * self.cell_width;
            let y = self.label_size + row as f32 * self.cell_height;

            // Get glyph data from font
            let ch = char::from_u32(ch_code).unwrap_or(' ');
            let glyph = self.font.glyph(ch);

            // Calculate pixel scale factor
            let pixel_scale_x = self.cell_width / font_width as f32;
            let pixel_scale_y = self.cell_height / font_height as f32;

            // Draw pixels
            for py in 0..font_height {
                for px in 0..font_width {
                    if glyph.get_pixel(px as usize, py as usize) {
                        frame.fill_rectangle(
                            Point::new(x + px as f32 * pixel_scale_x, y + py as f32 * pixel_scale_y),
                            Size::new(pixel_scale_x, pixel_scale_y),
                            fg_color,
                        );
                    }
                }
            }
        }

        vec![frame.into_geometry()]
    }
}
