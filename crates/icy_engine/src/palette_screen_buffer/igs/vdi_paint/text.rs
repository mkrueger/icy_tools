use icy_parser_core::{TextEffects, TextRotation};

use super::VdiPaint;
use crate::igs::load_atari_font;
use crate::{EditableScreen, Position, Size};

impl VdiPaint {
    #[inline]
    fn apply_rotation(&self, x: i32, y: i32, font_size: Size, skew_offset: i32, y_offset: i32) -> (i32, i32) {
        match self.text_rotation {
            TextRotation::Degrees0 => (x + skew_offset, y - y_offset),
            TextRotation::Degrees90 => (y - y_offset, -1 + font_size.width - (x + skew_offset)),
            TextRotation::Degrees180 => (font_size.width - (x + skew_offset) - 1, -y + y_offset),
            TextRotation::Degrees270 => (-y + y_offset, x + skew_offset),
        }
    }

    #[inline]
    fn apply_underline_rotation(&self, x: i32, y: i32, font_size: Size, skew_offset: i32, y_offset: i32) -> (i32, i32) {
        match self.text_rotation {
            TextRotation::Degrees0 => (x + skew_offset, y - y_offset),
            TextRotation::Degrees90 => (y - y_offset, font_size.width - (x + skew_offset)),
            TextRotation::Degrees180 => (font_size.width - (x + skew_offset) - 1, -y + y_offset),
            TextRotation::Degrees270 => (-y + y_offset - 1, (x + skew_offset) - 1),
        }
    }

    pub fn write_text(&mut self, screen: &mut dyn EditableScreen, pos: Position, text: &[u8]) {
        let (metrics, font) = load_atari_font(self.text_size);
        let is_outlined = self.text_effects.contains(TextEffects::OUTLINED);
        let outline_thickness = if is_outlined { 1 } else { 0 };

        let mut pos = pos;

        let color = self.text_color;
        let bg_color = 0;
        let font_size = font.size();

        match self.text_rotation {
            TextRotation::Degrees90 => {
                pos.y -= font_size.height - 1;
            }
            TextRotation::Degrees180 => {
                pos.x -= font_size.width - 1;
            }
            _ => {}
        }

        let mut draw_mask: u16 = if self.text_effects.contains(TextEffects::GHOSTED) { 0x5555 } else { 0xFFFF };

        for ch in text {
            let glyph = font.glyph(*ch as char);

            if is_outlined {
                for y in 0..font_size.height {
                    for x in 0..font_size.width {
                        let pixel_set = glyph.get_pixel(x as usize, y as usize);
                        draw_mask = draw_mask.rotate_left(1);
                        if pixel_set && (1 & draw_mask) != 0 {
                            let (rx, ry) = self.apply_rotation(x, y, font_size, 0, metrics.y_off);
                            for dy in -1..=1 {
                                for dx in -1..=1 {
                                    let p = pos + Position::new(rx + dx, ry + dy);
                                    self.set_pixel(screen, p.x, p.y, color);
                                }
                            }
                        }
                    }
                }
                let mut draw_mask: u16 = if self.text_effects.contains(TextEffects::GHOSTED) { 0x5555 } else { 0xFFFF };
                for y in 0..font_size.height {
                    for x in 0..font_size.width {
                        let pixel_set = glyph.get_pixel(x as usize, y as usize);
                        draw_mask = draw_mask.rotate_left(1);

                        if pixel_set && (1 & draw_mask) != 0 {
                            let (rx, ry) = self.apply_rotation(x, y, font_size, 0, metrics.y_off);
                            let p = pos + Position::new(rx, ry);
                            self.set_pixel(screen, p.x, p.y, bg_color);
                        }
                    }
                }
            } else {
                for y in 0..font_size.height {
                    draw_mask = draw_mask.rotate_left(1);

                    for x in 0..font_size.width {
                        let pixel_set = glyph.get_pixel(x as usize, y as usize);
                        if pixel_set {
                            if 1 & draw_mask != 0 {
                                let skew_offset = if self.text_effects.contains(TextEffects::SKEWED) {
                                    (font_size.height - 1 - y) / 2 - (y % 2)
                                } else {
                                    0
                                };

                                let (rx, ry) = self.apply_rotation(x, y, font_size, skew_offset, metrics.y_off);
                                let p = pos + Position::new(rx, ry);
                                self.set_pixel(screen, p.x, p.y, color);

                                if self.text_effects.contains(TextEffects::THICKENED) {
                                    match self.text_rotation {
                                        TextRotation::Degrees0 => {
                                            for t in 1..=metrics.thicken {
                                                self.set_pixel(screen, p.x + t, p.y, color);
                                            }
                                        }
                                        TextRotation::Degrees90 => {
                                            for t in 1..=metrics.thicken {
                                                self.set_pixel(screen, p.x, p.y - t, color);
                                            }
                                        }
                                        TextRotation::Degrees180 => {
                                            for t in 1..=metrics.thicken {
                                                self.set_pixel(screen, p.x - t, p.y, color);
                                            }
                                        }
                                        TextRotation::Degrees270 => {
                                            for t in 1..=metrics.thicken {
                                                self.set_pixel(screen, p.x, p.y + t, color);
                                            }
                                        }
                                    }
                                }
                            }
                        }
                        draw_mask = draw_mask.rotate_left(1);
                    }
                }
            }

            if self.text_effects.contains(TextEffects::UNDERLINED) {
                let mut underline_mask: u16 = if self.text_effects.contains(TextEffects::GHOSTED) { 0x5555 } else { 0xFFFF };
                let underline_width = if is_outlined {
                    metrics.underline_width + 2 * outline_thickness
                } else {
                    metrics.underline_width
                };
                for y2 in 0..metrics.underline_height {
                    for x in 0..underline_width {
                        underline_mask = underline_mask.rotate_left(1);

                        if 1 & underline_mask != 0 {
                            let underline_y = metrics.underline_pos + y2;
                            let skew_offset = if self.text_effects.contains(TextEffects::SKEWED) {
                                (font_size.height - 1 - underline_y) / 2 - (underline_y % 2)
                            } else {
                                0
                            };

                            let (rx, ry) = self.apply_underline_rotation(x, underline_y, font_size, skew_offset, metrics.y_off);
                            let p = pos + Position::new(rx, ry);
                            self.set_pixel(screen, p.x, p.y, color);
                        }
                    }
                }
            }

            let base_width = if is_outlined {
                font_size.width + 2 * outline_thickness
            } else {
                font_size.width
            };

            let char_width = base_width;

            match self.text_rotation {
                TextRotation::Degrees0 => pos.x += char_width,
                TextRotation::Degrees90 => pos.y -= char_width,
                TextRotation::Degrees270 => pos.y += char_width,
                TextRotation::Degrees180 => pos.x -= char_width,
            }
        }
    }
}
