use crate::{Position, Rectangle, RenderOptions, TextBuffer, TextPane};

use super::Size;

impl TextBuffer {
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

        // Pre-scan lines to determine which are double-height
        let mut is_double_height_line = vec![false; rect.get_height() as usize];
        let mut is_bottom_half_line = vec![false; rect.get_height() as usize];

        let mut y = 0;
        while y < rect.get_height() {
            let abs_y = y + rect.start.y;
            // Check if any character in this line has double-height
            for x in 0..rect.get_width() {
                let pos = Position::new(x + rect.start.x, abs_y);
                let ch = self.get_char(pos);
                if ch.attribute.is_double_height() {
                    is_double_height_line[y as usize] = true;
                    // Mark the next line as bottom half (if it exists)
                    if (y + 1) < rect.get_height() {
                        is_bottom_half_line[(y + 1) as usize] = true;
                    }
                    break; // No need to check rest of line
                }
            }

            if is_double_height_line[y as usize] {
                y += 1;
            }

            y += 1;
        }

        // Helper function to render a character
        let render_char = |pixels: &mut [u8], x: i32, y: i32, pos: Position| {
            // Check if this line is a bottom half
            if is_bottom_half_line[y as usize] {
                // Get the character from the line above
                if pos.y > 0 {
                    let above_pos = Position::new(pos.x, pos.y - 1);
                    let above_ch = self.get_char(above_pos);

                    // Only render bottom half if the character above has double-height flag
                    if !above_ch.attribute.is_double_height() {
                        // Character above is not double-height, just render blank space
                        // Background fill only (no glyph)
                        let bg = above_ch.attribute.get_background();
                        let bg_idx = bg as usize;
                        let (b_r, b_g, b_b) = if bg_idx < palette_cache.len() { palette_cache[bg_idx] } else { (0, 0, 0) };

                        let cell_pixel_w = font_size.width;
                        let cell_pixel_h = font_size.height;
                        let base_pixel_x = x * cell_pixel_w;
                        let base_pixel_y = y * cell_pixel_h;

                        unsafe {
                            for cy in 0..cell_pixel_h {
                                let line_start = ((base_pixel_y + cy) * line_bytes + base_pixel_x * 4) as usize;
                                if line_start >= pixels.len() {
                                    break;
                                }
                                let mut o = line_start;
                                for _cx in 0..cell_pixel_w {
                                    if o + 3 >= pixels.len() {
                                        break;
                                    }
                                    *pixels.get_unchecked_mut(o) = b_r;
                                    *pixels.get_unchecked_mut(o + 1) = b_g;
                                    *pixels.get_unchecked_mut(o + 2) = b_b;
                                    *pixels.get_unchecked_mut(o + 3) = 0xFF;
                                    o += 4;
                                }
                            }
                        }
                        return; // Done with this cell
                    }

                    // Otherwise continue with double-height bottom half rendering
                } else {
                    return; // No line above, skip
                }
            }

            let ch = self.get_char(pos);

            // Determine what to render and how
            let is_in_double_height_line = is_double_height_line[y as usize];
            let is_rendering_bottom_half = is_bottom_half_line[y as usize];
            let render_ch = if is_rendering_bottom_half {
                // We already checked above that this character has double-height
                self.get_char(Position::new(pos.x, pos.y - 1))
            } else {
                ch
            };

            // Resolve font
            let font = self.get_font(render_ch.get_font_page()).unwrap_or_else(|| self.get_font(0).unwrap());

            // Foreground index (apply bold high bit)
            let mut fg = render_ch.attribute.get_foreground();
            if render_ch.attribute.is_bold() && fg < 8 {
                fg += 8;
            }
            let bg = render_ch.attribute.get_background();

            if render_ch.attribute.is_blinking() && !options.blink_on || render_ch.attribute.is_concealed() {
                fg = bg;
            }

            let is_selected = selection_active && selection_ref.map(|sel| sel.is_inside(pos)).unwrap_or(false);

            let (f_r, f_g, f_b, b_r, b_g, b_b) = if is_selected {
                if let Some((efr, efg, efb, ebr, ebg, ebb)) = explicit_sel_colors {
                    (efr, efg, efb, ebr, ebg, ebb)
                } else {
                    // Invert fallback - handle transparent colors
                    let bg_idx = bg as usize;
                    let fg_idx = fg as usize;

                    let (f_r, f_g, f_b) = if bg_idx < palette_cache.len() { palette_cache[bg_idx] } else { (0, 0, 0) };

                    let (b_r, b_g, b_b) = if fg_idx < palette_cache.len() {
                        palette_cache[fg_idx]
                    } else {
                        (255, 255, 255)
                    };

                    (f_r, f_g, f_b, b_r, b_g, b_b)
                }
            } else {
                let fg_idx = fg as usize;
                let bg_idx = bg as usize;

                let (f_r, f_g, f_b) = if fg_idx < palette_cache.len() {
                    palette_cache[fg_idx]
                } else {
                    (255, 255, 255)
                };

                let (b_r, b_g, b_b) = if bg_idx < palette_cache.len() { palette_cache[bg_idx] } else { (0, 0, 0) };

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
                    if line_start >= pixels.len() {
                        break;
                    }
                    let mut o = line_start;
                    for _cx in 0..cell_pixel_w {
                        if o + 3 >= pixels.len() {
                            break;
                        }
                        *pixels.get_unchecked_mut(o) = b_r;
                        *pixels.get_unchecked_mut(o + 1) = b_g;
                        *pixels.get_unchecked_mut(o + 2) = b_b;
                        *pixels.get_unchecked_mut(o + 3) = 0xFF;
                        o += 4;
                    }
                }
            }

            // Decide how to render the glyph
            if is_rendering_bottom_half || (is_in_double_height_line && render_ch.attribute.is_double_height()) {
                // Render double-height (either top or bottom half)
                if let Some(glyph) = font.get_glyph(render_ch.ch) {
                    let glyph_height = glyph.data.len();

                    unsafe {
                        for cy in 0..cell_pixel_h {
                            // Determine which part of the original glyph to sample
                            let source_y = if is_rendering_bottom_half {
                                // Bottom half: map from glyph_height/2 to glyph_height
                                (glyph_height / 2) + (cy as usize * glyph_height / 2 / cell_pixel_h as usize)
                            } else {
                                // Top half: map from 0 to glyph_height/2
                                cy as usize * glyph_height / 2 / cell_pixel_h as usize
                            };

                            if source_y >= glyph_height {
                                continue;
                            }

                            let row_bits = glyph.data[source_y];
                            let line_start = ((base_pixel_y + cy) * line_bytes + base_pixel_x * 4) as usize;
                            if line_start >= pixels.len() {
                                break;
                            }

                            for cx in 0..cell_pixel_w.min(8) {
                                if row_bits & (128 >> cx) != 0 {
                                    let o = line_start + (cx * 4) as usize;
                                    if o + 3 >= pixels.len() {
                                        break;
                                    }
                                    *pixels.get_unchecked_mut(o) = f_r;
                                    *pixels.get_unchecked_mut(o + 1) = f_g;
                                    *pixels.get_unchecked_mut(o + 2) = f_b;
                                }
                            }
                        }
                    }
                }
            } else {
                // Normal height rendering (including non-double-height chars in double-height lines)
                if let Some(glyph) = font.get_glyph(render_ch.ch) {
                    let max_cy = glyph.data.len().min(cell_pixel_h as usize);
                    for cy in 0..max_cy {
                        let row_bits = glyph.data[cy];
                        let line_start = ((base_pixel_y + cy as i32) * line_bytes + base_pixel_x * 4) as usize;
                        if line_start >= pixels.len() {
                            break;
                        }
                        unsafe {
                            for cx in 0..cell_pixel_w.min(8) {
                                if row_bits & (128 >> cx) != 0 {
                                    let o = line_start + (cx * 4) as usize;
                                    if o + 3 >= pixels.len() {
                                        break;
                                    }
                                    *pixels.get_unchecked_mut(o) = f_r;
                                    *pixels.get_unchecked_mut(o + 1) = f_g;
                                    *pixels.get_unchecked_mut(o + 2) = f_b;
                                }
                            }
                        }
                    }
                }
            }

            // Overlay attributes (underline, overline, crossed out) - only for original character's attributes
            // and not when rendering bottom half
            if !is_rendering_bottom_half && (ch.attribute.is_underlined() || ch.attribute.is_overlined() || ch.attribute.is_crossed_out()) {
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
                                if line_start >= pixels.len() {
                                    continue;
                                }
                                let mut o = line_start;
                                for _cx in 0..cell_pixel_w {
                                    if o + 3 >= pixels.len() {
                                        break;
                                    }
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
                        if line_start < pixels.len() {
                            let mut o = line_start;
                            for _cx in 0..cell_pixel_w {
                                if o + 3 >= pixels.len() {
                                    break;
                                }
                                *pixels.get_unchecked_mut(o) = f_r;
                                *pixels.get_unchecked_mut(o + 1) = f_g;
                                *pixels.get_unchecked_mut(o + 2) = f_b;
                                o += 4;
                            }
                        }
                    }
                }
                if ch.attribute.is_crossed_out() {
                    let mid_y = base_pixel_y + cell_pixel_h / 2;
                    unsafe {
                        let line_start = (mid_y * line_bytes + base_pixel_x * 4) as usize;
                        if line_start < pixels.len() {
                            let mut o = line_start;
                            for _cx in 0..cell_pixel_w {
                                if o + 3 >= pixels.len() {
                                    break;
                                }
                                *pixels.get_unchecked_mut(o) = f_r;
                                *pixels.get_unchecked_mut(o + 1) = f_g;
                                *pixels.get_unchecked_mut(o + 2) = f_b;
                                o += 4;
                            }
                        }
                    }
                }
            }
        };

        // Sequential processing (parallel would be complex with line dependencies)
        for y in 0..rect.get_height() {
            for x in 0..rect.get_width() {
                let pos = Position::new(x + rect.start.x, y + rect.start.y);
                render_char(pixels, x, y, pos);
            }
        }

        // Sixels overlay (unchanged)
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
