use crate::{Position, Rectangle, RenderOptions, TextBuffer, TextPane};

use super::Size;

impl TextBuffer {
    pub fn render_to_rgba(&self, options: &RenderOptions, scan_lines: bool) -> (Size, Vec<u8>) {
        let Some(font) = self.get_font(0) else {
            log::error!("render_to_rgba: no font available");
            return (Size::new(0, 0), Vec::new());
        };
        let font_size = font.size();

        // Validate buffer dimensions
        if self.get_width() <= 0 || self.get_height() <= 0 {
            log::error!("render_to_rgba: invalid buffer dimensions {}x{}", self.get_width(), self.get_height());
            return (Size::new(0, 0), Vec::new());
        }

        let rect = options.rect.as_rectangle_with_width(self.get_width());
        let px_width = rect.get_width() * font_size.width;
        let px_height = rect.get_height() * font_size.height;

        println!(
            "[render_to_rgba] rect={}x{} font={}x{} px={}x{}",
            rect.get_width(),
            rect.get_height(),
            font_size.width,
            font_size.height,
            px_width,
            px_height
        );

        // Check for overflow before allocation
        let total_pixels = (px_width as u64).checked_mul(px_height as u64);
        if total_pixels.is_none() || total_pixels.unwrap() > 100_000_000 {
            log::error!(
                "render_to_rgba: dimensions too large {}x{} ({}x{} chars)",
                px_width,
                px_height,
                rect.get_width(),
                rect.get_height()
            );
            return (Size::new(0, 0), Vec::new());
        }

        let line_width = px_width as usize;

        let scan_lines = options.override_scan_lines.unwrap_or(scan_lines);
        let mut pixels_u32 = if scan_lines {
            // Render to temporary buffer first
            let mut pixels_u32 = vec![0u32; (px_width * px_height) as usize];
            self.render_to_rgba_into(options, &mut pixels_u32, font_size, rect, px_width, px_height);

            // Double the height by copying each scanline (working with u32)
            let doubled_size = pixels_u32.len() * 2;
            let mut doubled_pixels = Vec::with_capacity(doubled_size);

            // Process line by line
            for y in 0..px_height as usize {
                let row_start = y * line_width;
                let row_end = row_start + line_width;

                // Copy the line once
                doubled_pixels.extend_from_slice(&pixels_u32[row_start..row_end]);
                // Duplicate the line for scanline effect
                doubled_pixels.extend_from_slice(&pixels_u32[row_start..row_end]);
            }

            doubled_pixels
        } else {
            // Render directly with u32
            let mut pixels_u32 = vec![0u32; (px_width * px_height) as usize];
            self.render_to_rgba_into(options, &mut pixels_u32, font_size, rect, px_width, px_height);
            pixels_u32
        };

        // Calculate output height
        let out_height = if scan_lines { px_height * 2 } else { px_height };

        // Convert Vec<u32> to Vec<u8> without copying
        let pixels = unsafe {
            let ptr = pixels_u32.as_mut_ptr().cast::<u8>();
            let len = pixels_u32.len() * 4;
            let cap = pixels_u32.capacity() * 4;
            std::mem::forget(pixels_u32);
            Vec::from_raw_parts(ptr, len, cap)
        };
        (Size::new(px_width, out_height), pixels)
    }

    /// Render only a specific pixel region (for viewport-based rendering)
    /// Renders the character region and crops to exact pixel bounds
    /// Render only a specific pixel region (for viewport-based rendering)
    /// Renders the character region and crops to exact pixel bounds
    pub fn render_region_to_rgba(&self, px_region: Rectangle, options: &RenderOptions, scan_lines: bool) -> (Size, Vec<u8>) {
        let Some(font) = self.get_font(0) else {
            log::error!("render_region_to_rgba: no font available");
            return (Size::new(0, 0), Vec::new());
        };
        let font_size = font.size();

        // Validate buffer dimensions
        if self.get_width() <= 0 || self.get_height() <= 0 {
            log::error!("render_region_to_rgba: invalid buffer dimensions {}x{}", self.get_width(), self.get_height());
            return (Size::new(0, 0), Vec::new());
        }

        // Validate px_region dimensions
        if px_region.size.width <= 0 || px_region.size.height <= 0 {
            log::warn!(
                "render_region_to_rgba: invalid region dimensions {}x{}",
                px_region.size.width,
                px_region.size.height
            );
            return (Size::new(0, 0), Vec::new());
        }

        let scan_lines = options.override_scan_lines.unwrap_or(scan_lines);

        // Convert pixel region to character region (round outwards)
        let char_x = px_region.start.x / font_size.width;
        let char_y = px_region.start.y / font_size.height;
        let char_right = (px_region.start.x + px_region.size.width + font_size.width - 1) / font_size.width;
        let char_bottom = (px_region.start.y + px_region.size.height + font_size.height - 1) / font_size.height;

        // Clamp to buffer bounds
        let char_x = char_x.clamp(0, self.get_width());
        let char_y = char_y.clamp(0, self.get_height());
        let char_width = (char_right - char_x).clamp(0, self.get_width() - char_x);
        let char_height = (char_bottom - char_y).clamp(0, self.get_height() - char_y);

        // Early exit if nothing to render
        if char_width <= 0 || char_height <= 0 {
            return (Size::new(0, 0), Vec::new());
        }

        // Create render options
        let region_options = RenderOptions {
            rect: Rectangle::from_coords(char_x, char_y, char_x + char_width, char_y + char_height).into(),
            blink_on: options.blink_on,
            selection: options.selection,
            selection_fg: options.selection_fg.clone(),
            selection_bg: options.selection_bg.clone(),
            override_scan_lines: None,
        };

        // Render the character region
        let (full_size, full_pixels) = self.render_to_rgba(&region_options, scan_lines);

        // Check if render produced valid output
        if full_size.width <= 0 || full_size.height <= 0 || full_pixels.is_empty() {
            return (Size::new(0, 0), Vec::new());
        }

        // Calculate crop bounds with safe arithmetic
        let crop_x = (px_region.start.x - char_x * font_size.width).max(0);
        let crop_y = (px_region.start.y - char_y * font_size.height).max(0);

        // Ensure we don't go out of bounds
        let crop_width = px_region.size.width.min(full_size.width.saturating_sub(crop_x)).max(0);
        let crop_height = px_region.size.height.min(full_size.height.saturating_sub(crop_y)).max(0);

        // Early exit if crop dimensions are invalid
        if crop_width <= 0 || crop_height <= 0 {
            return (Size::new(0, 0), Vec::new());
        }

        // Fast path: no cropping needed
        if crop_x == 0 && crop_y == 0 && crop_width == full_size.width && crop_height == full_size.height {
            return (full_size, full_pixels);
        }

        let src_stride = full_size.width as usize * 4;
        let dst_stride = crop_width as usize * 4;

        // Check for potential overflow before allocation
        let total_bytes = (crop_width as u64).saturating_mul(crop_height as u64).saturating_mul(4);
        if total_bytes > 100_000_000 || total_bytes == 0 {
            log::error!("render_region_to_rgba: crop dimensions too large or zero {}x{}", crop_width, crop_height);
            return (Size::new(0, 0), Vec::new());
        }

        let mut full_pixels = full_pixels;

        // Fast path: only vertical cropping (no X offset, same width)
        if crop_x == 0 && crop_width == full_size.width {
            let src_start = crop_y as usize * src_stride;
            let total_bytes = crop_height as usize * src_stride;
            if src_start + total_bytes <= full_pixels.len() {
                full_pixels.copy_within(src_start..src_start + total_bytes, 0);
                full_pixels.truncate(total_bytes);
                return (Size::new(crop_width, crop_height), full_pixels);
            } else {
                log::error!("render_region_to_rgba: vertical crop out of bounds");
                return (Size::new(0, 0), Vec::new());
            }
        }

        // General case: both X and Y cropping
        let crop_x_bytes = crop_x as usize * 4;
        let mut src_offset = crop_y as usize * src_stride + crop_x_bytes;
        let mut dst_offset = 0usize;

        for _ in 0..crop_height as usize {
            if src_offset + dst_stride > full_pixels.len() {
                log::error!("render_region_to_rgba: general crop out of bounds");
                break;
            }
            full_pixels.copy_within(src_offset..src_offset + dst_stride, dst_offset);
            src_offset += src_stride;
            dst_offset += dst_stride;
        }

        let final_size = (crop_width as usize * crop_height as usize * 4).min(full_pixels.len());
        full_pixels.truncate(final_size);
        (Size::new(crop_width, crop_height), full_pixels)
    }

    fn render_to_rgba_into(&self, options: &RenderOptions, pixels: &mut [u32], font_size: Size, rect: Rectangle, px_width: i32, px_height: i32) {
        // Bail out early if buffer mismatched
        if pixels.len() != (px_width * px_height) as usize {
            log::error!(
                "render_to_rgba_into: pixel buffer size mismatch (expected {}, got {})",
                px_width * px_height,
                pixels.len()
            );
            return;
        }

        let line_width = px_width;

        match self.buffer_type {
            super::BufferType::Viewdata => self.render_viewdata_u32(options, pixels, font_size, rect, line_width),
            _ => {
                self.render_optimized_u32(options, pixels, font_size, rect, line_width);
            }
        }
        // Sixel overlay now works on u32 directly
        self.render_sixel_overlay(pixels, font_size, rect, px_width, px_height);
    }

    fn render_optimized_u32(&self, options: &RenderOptions, pixels: &mut [u32], font_size: Size, rect: Rectangle, line_width: i32) {
        use crate::Palette;

        // Palette cache as u32 for direct pixel writes
        let palette_cache = self.palette.get_palette_cache_rgba();
        // Fallback colors as u32
        let default_fg = Palette::rgb_to_rgba_u32(255, 255, 255);
        let default_bg = Palette::rgb_to_rgba_u32(0, 0, 0);

        let selection_active = options.selection.is_some();
        let selection_ref = options.selection.as_ref();

        // Optional selection colors (if both are set) - pre-packed as u32
        let explicit_sel_colors = options.selection_fg.as_ref().zip(options.selection_bg.as_ref()).map(|(fg, bg)| {
            let (f_r, f_g, f_b) = fg.get_rgb();
            let (b_r, b_g, b_b) = bg.get_rgb();
            (Palette::rgb_to_rgba_u32(f_r, f_g, f_b), Palette::rgb_to_rgba_u32(b_r, b_g, b_b))
        });

        use rayon::prelude::*;

        // Process each character row in parallel
        // row_size is in u32 units (pixels per character row)
        let row_size = (font_size.height * line_width) as usize;

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

                // Get colors directly as u32
                let (fg_u32, bg_u32) = if is_selected {
                    if let Some((sel_fg, sel_bg)) = explicit_sel_colors {
                        (sel_fg, sel_bg)
                    } else {
                        // Invert fallback - swap fg and bg
                        let bg_idx = bg as usize;
                        let fg_idx = fg as usize;
                        let fg_color = if bg_idx < palette_cache.len() { palette_cache[bg_idx] } else { default_bg };
                        let bg_color = if fg_idx < palette_cache.len() { palette_cache[fg_idx] } else { default_fg };
                        (fg_color, bg_color)
                    }
                } else {
                    let fg_idx = fg as usize;
                    let bg_idx = bg as usize;
                    let fg_color = if fg_idx < palette_cache.len() { palette_cache[fg_idx] } else { default_fg };
                    let bg_color = if bg_idx < palette_cache.len() { palette_cache[bg_idx] } else { default_bg };
                    (fg_color, bg_color)
                };

                let cell_pixel_w = font_size.width;
                let cell_pixel_h = font_size.height;
                let base_px = x * cell_pixel_w;

                // Background fill first - using unchecked access for performance
                unsafe {
                    for cy in 0..cell_pixel_h {
                        let line_offset = (cy * line_width + base_px) as usize;
                        for cx in 0..cell_pixel_w as usize {
                            *row_pixels.get_unchecked_mut(line_offset + cx) = bg_u32;
                        }
                    }
                }

                // Foreground glyph overlay
                if let Some(glyph) = font.get_glyph(ch.ch) {
                    let max_cy = glyph.bitmap.pixels.len().min(cell_pixel_h as usize);
                    unsafe {
                        for cy in 0..max_cy {
                            let row = glyph.bitmap.pixels.get_unchecked(cy);
                            let line_offset = (cy as i32 * line_width + base_px) as usize;
                            for cx in 0..cell_pixel_w.min(8).min(row.len() as i32) as usize {
                                if *row.get_unchecked(cx) {
                                    *row_pixels.get_unchecked_mut(line_offset + cx) = fg_u32;
                                }
                            }
                        }
                    }
                }

                if ch.attribute.is_underlined() || ch.attribute.is_overlined() || ch.attribute.is_crossed_out() {
                    // Underline
                    if ch.attribute.is_underlined() {
                        let lines: &[i32] = if ch.attribute.is_double_underlined() {
                            &[cell_pixel_h - 2, cell_pixel_h - 1]
                        } else {
                            &[cell_pixel_h - 1]
                        };
                        unsafe {
                            for ul_y in lines {
                                if *ul_y >= 0 && *ul_y < cell_pixel_h {
                                    let line_offset = (*ul_y * line_width + base_px) as usize;
                                    for cx in 0..cell_pixel_w as usize {
                                        *row_pixels.get_unchecked_mut(line_offset + cx) = fg_u32;
                                    }
                                }
                            }
                        }
                    }
                    // Overline
                    if ch.attribute.is_overlined() {
                        let line_offset = base_px as usize;
                        unsafe {
                            for cx in 0..cell_pixel_w as usize {
                                *row_pixels.get_unchecked_mut(line_offset + cx) = fg_u32;
                            }
                        }
                    }
                    // Strike-through
                    if ch.attribute.is_crossed_out() {
                        let mid_y = cell_pixel_h / 2;
                        let line_offset = (mid_y * line_width + base_px) as usize;
                        unsafe {
                            for cx in 0..cell_pixel_w as usize {
                                *row_pixels.get_unchecked_mut(line_offset + cx) = fg_u32;
                            }
                        }
                    }
                }
            }
        });
    }

    fn render_viewdata_u32(&self, options: &RenderOptions, pixels: &mut [u32], font_size: Size, rect: Rectangle, line_width: i32) {
        use crate::Palette;

        // Palette cache (u32 version for faster writes)
        let palette_cache = self.palette.get_palette_cache_rgba();

        let selection_active = options.selection.is_some();
        let selection_ref = options.selection.as_ref();

        // Optional selection colors (if both are set)
        let explicit_sel_colors = options.selection_fg.as_ref().zip(options.selection_bg.as_ref()).map(|(fg, bg)| {
            let (f_r, f_g, f_b) = fg.get_rgb();
            let (b_r, b_g, b_b) = bg.get_rgb();
            (Palette::rgb_to_rgba_u32(f_r, f_g, f_b), Palette::rgb_to_rgba_u32(b_r, b_g, b_b))
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
        let render_char = |pixels: &mut [u32], x: i32, y: i32, pos: Position| {
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
                        let bg_u32 = if bg_idx < palette_cache.len() {
                            palette_cache[bg_idx]
                        } else {
                            Palette::rgb_to_rgba_u32(0, 0, 0)
                        };

                        let cell_pixel_w = font_size.width;
                        let cell_pixel_h = font_size.height;
                        let base_pixel_x = x * cell_pixel_w;
                        let base_pixel_y = y * cell_pixel_h;

                        unsafe {
                            for cy in 0..cell_pixel_h {
                                let line_start = ((base_pixel_y + cy) * line_width + base_pixel_x) as usize;
                                for cx in 0..cell_pixel_w as usize {
                                    *pixels.get_unchecked_mut(line_start + cx) = bg_u32;
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

            let (fg_u32, bg_u32) = if is_selected {
                if let Some((sel_fg_u32, sel_bg_u32)) = explicit_sel_colors {
                    (sel_fg_u32, sel_bg_u32)
                } else {
                    // Invert fallback - handle transparent colors
                    let bg_idx = bg as usize;
                    let fg_idx = fg as usize;

                    let fg_u32 = if bg_idx < palette_cache.len() {
                        palette_cache[bg_idx]
                    } else {
                        Palette::rgb_to_rgba_u32(0, 0, 0)
                    };
                    let bg_u32 = if fg_idx < palette_cache.len() {
                        palette_cache[fg_idx]
                    } else {
                        Palette::rgb_to_rgba_u32(255, 255, 255)
                    };

                    (fg_u32, bg_u32)
                }
            } else {
                let fg_idx = fg as usize;
                let bg_idx = bg as usize;

                let fg_u32 = if fg_idx < palette_cache.len() {
                    palette_cache[fg_idx]
                } else {
                    Palette::rgb_to_rgba_u32(255, 255, 255)
                };
                let bg_u32 = if bg_idx < palette_cache.len() {
                    palette_cache[bg_idx]
                } else {
                    Palette::rgb_to_rgba_u32(0, 0, 0)
                };

                (fg_u32, bg_u32)
            };

            let cell_pixel_w = font_size.width;
            let cell_pixel_h = font_size.height;
            let base_pixel_x = x * cell_pixel_w;
            let base_pixel_y = y * cell_pixel_h;

            // Background fill first
            unsafe {
                for cy in 0..cell_pixel_h {
                    let line_start = ((base_pixel_y + cy) * line_width + base_pixel_x) as usize;
                    for cx in 0..cell_pixel_w as usize {
                        *pixels.get_unchecked_mut(line_start + cx) = bg_u32;
                    }
                }
            }

            // Decide how to render the glyph
            if is_rendering_bottom_half || (is_in_double_height_line && render_ch.attribute.is_double_height()) {
                // Render double-height (either top or bottom half)
                if let Some(glyph) = font.get_glyph(render_ch.ch) {
                    let glyph_height = glyph.bitmap.pixels.len();

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

                            let row = glyph.bitmap.pixels.get_unchecked(source_y);
                            let line_start = ((base_pixel_y + cy) * line_width + base_pixel_x) as usize;

                            for cx in 0..cell_pixel_w.min(8).min(row.len() as i32) as usize {
                                if *row.get_unchecked(cx) {
                                    *pixels.get_unchecked_mut(line_start + cx) = fg_u32;
                                }
                            }
                        }
                    }
                }
            } else {
                // Normal height rendering (including non-double-height chars in double-height lines)
                if let Some(glyph) = font.get_glyph(render_ch.ch) {
                    let max_cy = glyph.bitmap.pixels.len().min(cell_pixel_h as usize);
                    unsafe {
                        for cy in 0..max_cy {
                            let row = glyph.bitmap.pixels.get_unchecked(cy);
                            let line_start = ((base_pixel_y + cy as i32) * line_width + base_pixel_x) as usize;

                            for cx in 0..cell_pixel_w.min(8).min(row.len() as i32) as usize {
                                if *row.get_unchecked(cx) {
                                    *pixels.get_unchecked_mut(line_start + cx) = fg_u32;
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
                    unsafe {
                        for ul_y in lines {
                            if ul_y >= 0 && ul_y < cell_pixel_h {
                                let line_start = ((base_pixel_y + ul_y) * line_width + base_pixel_x) as usize;
                                for cx in 0..cell_pixel_w as usize {
                                    *pixels.get_unchecked_mut(line_start + cx) = fg_u32;
                                }
                            }
                        }
                    }
                }
                if ch.attribute.is_overlined() {
                    let line_start = (base_pixel_y * line_width + base_pixel_x) as usize;
                    unsafe {
                        for cx in 0..cell_pixel_w as usize {
                            *pixels.get_unchecked_mut(line_start + cx) = fg_u32;
                        }
                    }
                }
                if ch.attribute.is_crossed_out() {
                    let mid_y = base_pixel_y + cell_pixel_h / 2;
                    let line_start = (mid_y * line_width + base_pixel_x) as usize;
                    unsafe {
                        for cx in 0..cell_pixel_w as usize {
                            *pixels.get_unchecked_mut(line_start + cx) = fg_u32;
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
    }

    fn render_sixel_overlay(&self, pixels: &mut [u32], font_size: Size, rect: Rectangle, line_width: i32, px_height: i32) {
        if self.layers.is_empty() {
            return;
        }

        let font_dims = font_size;
        for layer in &self.layers {
            for sixel in &layer.sixels {
                // Calculate sixel position in character coordinates
                let sx_char = layer.get_offset().x + sixel.position.x;
                let sy_char = layer.get_offset().y + sixel.position.y;

                // Calculate sixel dimensions in character coordinates
                let sixel_width_chars = (sixel.get_width() + font_dims.width - 1) / font_dims.width;
                let sixel_height_chars = (sixel.get_height() + font_dims.height - 1) / font_dims.height;

                // Skip if sixel is completely outside the rect
                if sy_char + sixel_height_chars <= rect.start.y
                    || sy_char > rect.bottom()
                    || sx_char + sixel_width_chars <= rect.start.x
                    || sx_char > rect.right()
                {
                    continue;
                }

                // Calculate which part of the sixel is visible
                let sx = sx_char - rect.start.x;
                let sy = sy_char - rect.start.y;

                // Calculate pixel offsets for clipping
                let skip_x_px = if sx < 0 { -sx * font_dims.width } else { 0 };
                let skip_y_px = if sy < 0 { -sy * font_dims.height } else { 0 };

                // Calculate destination position (clamped to 0)
                let dest_x_px = sx.max(0) * font_dims.width;
                let dest_y_px = sy.max(0) * font_dims.height;

                // Calculate how many pixels to copy
                let max_y = (dest_y_px + sixel.get_height() - skip_y_px).min(px_height);
                let visible_width = (sixel.get_width() - skip_x_px).min(line_width - dest_x_px);

                if visible_width <= 0 {
                    continue;
                }

                let mut sixel_line = skip_y_px as usize;

                // Sixel data as u32 slice
                let sixel_u32 = unsafe { std::slice::from_raw_parts(sixel.picture_data.as_ptr().cast::<u32>(), sixel.picture_data.len() / 4) };
                let sixel_line_width = sixel.get_width() as usize;
                let line_width_usize = line_width as usize;

                // Pre-compute limits for safe unchecked access
                let pixels_len = pixels.len();
                let sixel_len = sixel_u32.len();

                for py in dest_y_px..max_y {
                    let dest_line_start = py as usize * line_width_usize;
                    let src_line_start = sixel_line * sixel_line_width;

                    // Bounds check before the inner loop
                    if src_line_start >= sixel_len {
                        break;
                    }

                    // Copy pixels with alpha blending - using unchecked access
                    unsafe {
                        for px in 0..visible_width as usize {
                            let dest_idx = dest_line_start + dest_x_px as usize + px;
                            let src_idx = src_line_start + skip_x_px as usize + px;

                            // Bounds check for each pixel
                            if src_idx >= sixel_len || dest_idx >= pixels_len {
                                break;
                            }

                            let src_pixel = *sixel_u32.get_unchecked(src_idx);
                            // Alpha is in the high byte (RGBA format: 0xAABBGGRR in little-endian)
                            let src_alpha = (src_pixel >> 24) as u8;

                            // Only blend if alpha > 0 (visible pixel)
                            if src_alpha > 0 {
                                if src_alpha == 255 {
                                    // Fully opaque - direct u32 copy (fastest path)
                                    *pixels.get_unchecked_mut(dest_idx) = src_pixel;
                                } else {
                                    // Alpha blending using u32 operations
                                    let dest_pixel = *pixels.get_unchecked(dest_idx);
                                    let alpha = src_alpha as u32;
                                    let inv_alpha = 255 - alpha;

                                    // Extract RGBA components
                                    let src_r = (src_pixel & 0xFF) as u32;
                                    let src_g = ((src_pixel >> 8) & 0xFF) as u32;
                                    let src_b = ((src_pixel >> 16) & 0xFF) as u32;

                                    let dst_r = (dest_pixel & 0xFF) as u32;
                                    let dst_g = ((dest_pixel >> 8) & 0xFF) as u32;
                                    let dst_b = ((dest_pixel >> 16) & 0xFF) as u32;

                                    // Blend
                                    let r = (src_r * alpha + dst_r * inv_alpha) / 255;
                                    let g = (src_g * alpha + dst_g * inv_alpha) / 255;
                                    let b = (src_b * alpha + dst_b * inv_alpha) / 255;

                                    *pixels.get_unchecked_mut(dest_idx) = r | (g << 8) | (b << 16) | 0xFF000000;
                                }
                            }
                        }
                    }

                    sixel_line += 1;
                }
            }
        }
    }
}
