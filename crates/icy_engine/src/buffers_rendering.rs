use crate::{Buffer, Position, Rectangle, RenderOptions, TextPane};

use super::Size;

impl Buffer {
    pub fn render_to_rgba(&self, options: &RenderOptions) -> (Size, Vec<u8>) {
        let font_size = self.get_font(0).unwrap().size;
        let rect = options.rect.as_rectangle_with_width(self.get_width());
        let px_width = rect.get_width() * font_size.width;
        let px_height = rect.get_height() * font_size.height;
        let line_bytes = px_width * 4;
        let required_size = (px_width * px_height * 4) as usize;

        let mut pixels = vec![0u8; required_size];

        self.render_to_rgba_into(options, &mut pixels, font_size, rect, px_width, px_height, line_bytes);

        (Size::new(px_width, px_height), pixels)
    }

    pub fn render_to_rgba_into(
        &self,
        options: &RenderOptions,
        pixels: &mut [u8],
        font_size: Size,
        rect: Rectangle,
        px_width: i32,
        px_height: i32,
        line_bytes: i32,
    ) {
        // Bail out early if buffer mismatched
        if pixels.len() != (px_width * px_height * 4) as usize {
            return;
        }

        // Palette cache
        let mut palette_cache = [(0u8, 0u8, 0u8); 256];
        for i in 0..self.palette.len() {
            palette_cache[i] = self.palette.get_rgb(i as u32);
        }

        let selection_active = options.selection.is_some();
        let selection_ref = options.selection.as_ref();

        // Optional selection colors (if both are set)
        let explicit_sel_colors = options.selection_fg.as_ref().zip(options.selection_bg.as_ref()).map(|(fg, bg)| {
            let (f_r, f_g, f_b) = fg.get_rgb();
            let (b_r, b_g, b_b) = bg.get_rgb();
            (f_r, f_g, f_b, b_r, b_g, b_b)
        });

        // Decide parallel vs sequential (avoid rayon overhead on tiny renders)
        let use_parallel = rect.get_height() > 3;

        if use_parallel {
            use rayon::prelude::*;

            // Process each character row in parallel
            // We need to split the pixels buffer into non-overlapping chunks
            let row_size = (font_size.height * line_bytes) as usize;

            // Use par_chunks_mut to get parallel mutable access to disjoint slices
            pixels.par_chunks_mut(row_size).enumerate().for_each(|(y, row_pixels)| {
                let y = y as i32;

                // Process this character row
                for x in 0..rect.get_width() {
                    let pos = Position::new(x + rect.start.x, y + rect.start.y);
                    let ch = self.get_char(pos);

                    // Resolve font
                    let font = self.get_font(ch.get_font_page()).unwrap_or_else(|| self.get_font(0).unwrap());

                    // Foreground index (apply bold high bit)
                    let mut fg = ch.attribute.get_foreground();
                    if ch.attribute.is_bold() && fg < 8 {
                        fg += 8;
                    }
                    let bg = ch.attribute.get_background();

                    if ch.attribute.is_blinking() && !options.blink_on {
                        fg = bg;
                    }

                    let is_selected = selection_active && selection_ref.map(|sel| sel.is_inside(pos)).unwrap_or(false);

                    let (f_r, f_g, f_b, b_r, b_g, b_b) = if is_selected {
                        if let Some((efr, efg, efb, ebr, ebg, ebb)) = explicit_sel_colors {
                            (efr, efg, efb, ebr, ebg, ebb)
                        } else {
                            // Invert fallback
                            let (f_r, f_g, f_b) = palette_cache[bg as usize];
                            let (b_r, b_g, b_b) = palette_cache[fg as usize];
                            (f_r, f_g, f_b, b_r, b_g, b_b)
                        }
                    } else {
                        let (f_r, f_g, f_b) = palette_cache[fg as usize];
                        let (b_r, b_g, b_b) = palette_cache[bg as usize];
                        (f_r, f_g, f_b, b_r, b_g, b_b)
                    };

                    let cell_pixel_w = font_size.width;
                    let cell_pixel_h = font_size.height;
                    let base_pixel_x = x * cell_pixel_w;

                    // Background fill first
                    unsafe {
                        for cy in 0..cell_pixel_h {
                            let line_offset = (cy * line_bytes + base_pixel_x * 4) as usize;
                            let mut o = line_offset;
                            for _cx in 0..cell_pixel_w {
                                *row_pixels.get_unchecked_mut(o) = b_r;
                                *row_pixels.get_unchecked_mut(o + 1) = b_g;
                                *row_pixels.get_unchecked_mut(o + 2) = b_b;
                                *row_pixels.get_unchecked_mut(o + 3) = 0xFF;
                                o += 4;
                            }
                        }
                    }

                    // Foreground glyph overlay
                    if let Some(glyph) = font.get_glyph(ch.ch) {
                        let max_cy = glyph.data.len().min(cell_pixel_h as usize);
                        for cy in 0..max_cy {
                            let row_bits = glyph.data[cy];
                            let line_offset = (cy as i32 * line_bytes + base_pixel_x * 4) as usize;
                            unsafe {
                                for cx in 0..cell_pixel_w.min(8) {
                                    if row_bits & (128 >> cx) != 0 {
                                        let o = line_offset + (cx * 4) as usize;
                                        *row_pixels.get_unchecked_mut(o) = f_r;
                                        *row_pixels.get_unchecked_mut(o + 1) = f_g;
                                        *row_pixels.get_unchecked_mut(o + 2) = f_b;
                                    }
                                }
                            }
                        }
                    }

                    // Overlay attributes
                    if ch.attribute.is_underlined() || ch.attribute.is_overlined() || ch.attribute.is_crossed_out() {
                        // Underline
                        if ch.attribute.is_underlined() {
                            let lines = if ch.attribute.is_double_underlined() {
                                vec![cell_pixel_h - 2, cell_pixel_h - 1]
                            } else {
                                vec![cell_pixel_h - 1]
                            };
                            for ul_y in lines {
                                if ul_y >= 0 && ul_y < cell_pixel_h {
                                    unsafe {
                                        let line_offset = (ul_y * line_bytes + base_pixel_x * 4) as usize;
                                        let mut o = line_offset;
                                        for _cx in 0..cell_pixel_w {
                                            *row_pixels.get_unchecked_mut(o) = f_r;
                                            *row_pixels.get_unchecked_mut(o + 1) = f_g;
                                            *row_pixels.get_unchecked_mut(o + 2) = f_b;
                                            o += 4;
                                        }
                                    }
                                }
                            }
                        }
                        // Overline
                        if ch.attribute.is_overlined() {
                            unsafe {
                                let line_offset = (base_pixel_x * 4) as usize;
                                let mut o = line_offset;
                                for _cx in 0..cell_pixel_w {
                                    *row_pixels.get_unchecked_mut(o) = f_r;
                                    *row_pixels.get_unchecked_mut(o + 1) = f_g;
                                    *row_pixels.get_unchecked_mut(o + 2) = f_b;
                                    o += 4;
                                }
                            }
                        }
                        // Strike
                        if ch.attribute.is_crossed_out() {
                            let mid_y = cell_pixel_h / 2;
                            unsafe {
                                let line_offset = (mid_y * line_bytes + base_pixel_x * 4) as usize;
                                let mut o = line_offset;
                                for _cx in 0..cell_pixel_w {
                                    *row_pixels.get_unchecked_mut(o) = f_r;
                                    *row_pixels.get_unchecked_mut(o + 1) = f_g;
                                    *row_pixels.get_unchecked_mut(o + 2) = f_b;
                                    o += 4;
                                }
                            }
                        }
                    }
                }
            });
        } else {
            // Sequential processing for small renders
            for y in 0..rect.get_height() {
                for x in 0..rect.get_width() {
                    let pos = Position::new(x + rect.start.x, y + rect.start.y);
                    let ch = self.get_char(pos);

                    // Resolve font
                    let font = self.get_font(ch.get_font_page()).unwrap_or_else(|| self.get_font(0).unwrap());

                    // Foreground index (apply bold high bit)
                    let mut fg = ch.attribute.get_foreground();
                    if ch.attribute.is_bold() && fg < 8 {
                        fg += 8;
                    }
                    let bg = ch.attribute.get_background();

                    if ch.attribute.is_blinking() && !options.blink_on {
                        fg = bg;
                    }

                    let is_selected = selection_active && selection_ref.map(|sel| sel.is_inside(pos)).unwrap_or(false);

                    let (f_r, f_g, f_b, b_r, b_g, b_b) = if is_selected {
                        if let Some((efr, efg, efb, ebr, ebg, ebb)) = explicit_sel_colors {
                            (efr, efg, efb, ebr, ebg, ebb)
                        } else {
                            // Invert fallback
                            let (f_r, f_g, f_b) = palette_cache[bg as usize];
                            let (b_r, b_g, b_b) = palette_cache[fg as usize];
                            (f_r, f_g, f_b, b_r, b_g, b_b)
                        }
                    } else {
                        let (f_r, f_g, f_b) = palette_cache[fg as usize];
                        let (b_r, b_g, b_b) = palette_cache[bg as usize];
                        (f_r, f_g, f_b, b_r, b_g, b_b)
                    };

                    let cell_pixel_w = font_size.width;
                    let cell_pixel_h = font_size.height;
                    let base_pixel_x = x * cell_pixel_w;
                    let base_pixel_y = y * cell_pixel_h;

                    // Background fill first
                    unsafe {
                        for cy in 0..cell_pixel_h {
                            let line_start = ((base_pixel_y + cy) * line_bytes + base_pixel_x * 4) as usize;
                            let mut o = line_start;
                            for _cx in 0..cell_pixel_w {
                                *pixels.get_unchecked_mut(o) = b_r;
                                *pixels.get_unchecked_mut(o + 1) = b_g;
                                *pixels.get_unchecked_mut(o + 2) = b_b;
                                *pixels.get_unchecked_mut(o + 3) = 0xFF;
                                o += 4;
                            }
                        }
                    }

                    // Foreground glyph overlay
                    if let Some(glyph) = font.get_glyph(ch.ch) {
                        let max_cy = glyph.data.len().min(cell_pixel_h as usize);
                        for cy in 0..max_cy {
                            let row_bits = glyph.data[cy];
                            let line_start = ((base_pixel_y + cy as i32) * line_bytes + base_pixel_x * 4) as usize;
                            unsafe {
                                for cx in 0..cell_pixel_w.min(8) {
                                    if row_bits & (128 >> cx) != 0 {
                                        let o = line_start + (cx * 4) as usize;
                                        *pixels.get_unchecked_mut(o) = f_r;
                                        *pixels.get_unchecked_mut(o + 1) = f_g;
                                        *pixels.get_unchecked_mut(o + 2) = f_b;
                                    }
                                }
                            }
                        }
                    }

                    // Overlay attributes (same as parallel version)
                    if ch.attribute.is_underlined() || ch.attribute.is_overlined() || ch.attribute.is_crossed_out() {
                        if ch.attribute.is_underlined() {
                            let lines = if ch.attribute.is_double_underlined() {
                                vec![cell_pixel_h - 2, cell_pixel_h - 1]
                            } else {
                                vec![cell_pixel_h - 1]
                            };
                            for ul_y in lines {
                                if ul_y >= 0 && ul_y < cell_pixel_h {
                                    unsafe {
                                        let line_start = ((base_pixel_y + ul_y) * line_bytes + base_pixel_x * 4) as usize;
                                        let mut o = line_start;
                                        for _cx in 0..cell_pixel_w {
                                            *pixels.get_unchecked_mut(o) = f_r;
                                            *pixels.get_unchecked_mut(o + 1) = f_g;
                                            *pixels.get_unchecked_mut(o + 2) = f_b;
                                            o += 4;
                                        }
                                    }
                                }
                            }
                        }
                        if ch.attribute.is_overlined() {
                            unsafe {
                                let line_start = (base_pixel_y * line_bytes + base_pixel_x * 4) as usize;
                                let mut o = line_start;
                                for _cx in 0..cell_pixel_w {
                                    *pixels.get_unchecked_mut(o) = f_r;
                                    *pixels.get_unchecked_mut(o + 1) = f_g;
                                    *pixels.get_unchecked_mut(o + 2) = f_b;
                                    o += 4;
                                }
                            }
                        }
                        if ch.attribute.is_crossed_out() {
                            let mid_y = base_pixel_y + cell_pixel_h / 2;
                            unsafe {
                                let line_start = (mid_y * line_bytes + base_pixel_x * 4) as usize;
                                let mut o = line_start;
                                for _cx in 0..cell_pixel_w {
                                    *pixels.get_unchecked_mut(o) = f_r;
                                    *pixels.get_unchecked_mut(o + 1) = f_g;
                                    *pixels.get_unchecked_mut(o + 2) = f_b;
                                    o += 4;
                                }
                            }
                        }
                    }
                }
            }
        }

        // Sixels overlay (no parallel needed; typically fewer)
        if !self.layers.is_empty() {
            let font_dims = font_size;
            for layer in &self.layers {
                for sixel in &layer.sixels {
                    // Clip test
                    let sx_char = layer.get_offset().x + sixel.position.x;
                    let sy_char = layer.get_offset().y + sixel.position.y;
                    if sy_char > rect.bottom() || sx_char > rect.right() {
                        continue;
                    }

                    let sx = sx_char - rect.start.x;
                    let sy = sy_char - rect.start.y;
                    if sx < 0 || sy < 0 {
                        continue;
                    }

                    let sx_px = sx * font_dims.width;
                    let sy_px = sy * font_dims.height;

                    let sixel_line_bytes = (sixel.get_width() * 4) as usize;
                    let max_y = (sy_px + sixel.get_height()).min(px_height);
                    let mut sixel_line = 0usize;

                    for py in sy_px..max_y {
                        let offset = (py * line_bytes + sx_px * 4) as usize;
                        let src_o = sixel_line * sixel_line_bytes;
                        if src_o + sixel_line_bytes > sixel.picture_data.len() || offset + sixel_line_bytes > pixels.len() {
                            break;
                        }
                        pixels[offset..offset + sixel_line_bytes].copy_from_slice(&sixel.picture_data[src_o..src_o + sixel_line_bytes]);
                        sixel_line += 1;
                    }
                }
            }
        }
    }
}
