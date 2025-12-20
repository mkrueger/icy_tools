use crate::{Position, Rectangle, RenderOptions, TextBuffer, TextPane, XTERM_256_PALETTE};

use super::Size;

/// Scale an RGBA image vertically by a factor (e.g., 1.2 for VGA aspect ratio correction)
/// Uses bilinear interpolation for smooth scaling
fn scale_image_vertical(pixels: Vec<u8>, width: i32, height: i32, scale: f32) -> (i32, Vec<u8>) {
    let new_height = (height as f32 * scale).round() as i32;
    if new_height <= 0 || width <= 0 {
        return (height, pixels);
    }

    let stride = width as usize * 4;
    let mut scaled = vec![0u8; stride * new_height as usize];

    for new_y in 0..new_height {
        // Map new_y back to original image coordinate
        let src_y = new_y as f32 / scale;
        let src_y0 = (src_y.floor() as i32).clamp(0, height - 1) as usize;
        let src_y1 = (src_y0 + 1).min(height as usize - 1);
        let t = src_y.fract();

        let dst_row = new_y as usize * stride;
        let src_row0 = src_y0 * stride;
        let src_row1 = src_y1 * stride;

        for x in 0..width as usize {
            let px = x * 4;

            // We only support binary alpha (0/255) and want to avoid RGB bleed when scaling.
            // Strategy:
            // - Compute alpha via vertical interpolation, then threshold to 0/255
            // - For RGB, only blend samples that are opaque (alpha=255), renormalize weights
            let a0 = pixels[src_row0 + px + 3] as f32;
            let a1 = pixels[src_row1 + px + 3] as f32;
            let a = a0 + (a1 - a0) * t;
            let out_a = if a >= 128.0 { 255u8 } else { 0u8 };

            if out_a == 0 {
                scaled[dst_row + px] = 0;
                scaled[dst_row + px + 1] = 0;
                scaled[dst_row + px + 2] = 0;
                scaled[dst_row + px + 3] = 0;
                continue;
            }

            let mut w0 = 1.0 - t;
            let mut w1 = t;
            if pixels[src_row0 + px + 3] == 0 {
                w0 = 0.0;
            }
            if pixels[src_row1 + px + 3] == 0 {
                w1 = 0.0;
            }

            let w_sum = w0 + w1;
            if w_sum <= f32::EPSILON {
                scaled[dst_row + px] = 0;
                scaled[dst_row + px + 1] = 0;
                scaled[dst_row + px + 2] = 0;
                scaled[dst_row + px + 3] = 0;
                continue;
            }
            w0 /= w_sum;
            w1 /= w_sum;

            for c in 0..3 {
                let v0 = pixels[src_row0 + px + c] as f32;
                let v1 = pixels[src_row1 + px + c] as f32;
                scaled[dst_row + px + c] = (v0 * w0 + v1 * w1).round() as u8;
            }
            scaled[dst_row + px + 3] = 255;
        }
    }

    (new_height, scaled)
}

impl TextBuffer {
    pub fn render_to_rgba(&self, options: &RenderOptions, scan_lines: bool) -> (Size, Vec<u8>) {
        // Use get_font_for_render to get the correct font (9px if letter spacing is enabled)
        let Some(font) = self.font_for_render(0) else {
            log::error!("render_to_rgba: no font available");
            return (Size::new(0, 0), Vec::new());
        };
        let font_size = font.size();

        // Validate buffer dimensions
        if self.width() <= 0 || self.height() <= 0 {
            log::error!("render_to_rgba: invalid buffer dimensions {}x{}", self.width(), self.height());
            return (Size::new(0, 0), Vec::new());
        }

        let rect = options.rect.as_rectangle_with_width(self.width());
        let px_width = rect.width() * font_size.width;
        let px_height = rect.height() * font_size.height;

        // Check for overflow before allocation
        let total_pixels = (px_width as u64).checked_mul(px_height as u64);
        if total_pixels.is_none() || total_pixels.unwrap() > 100_000_000 {
            log::error!(
                "render_to_rgba: dimensions too large {}x{} ({}x{} chars)",
                px_width,
                px_height,
                rect.width(),
                rect.height()
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

        // Apply aspect ratio correction if enabled (VGA pixel aspect ratio correction)
        if self.use_aspect_ratio {
            let stretch_factor = self.get_aspect_ratio_stretch_factor();
            let (scaled_height, scaled_pixels) = scale_image_vertical(pixels, px_width, out_height, stretch_factor);
            (Size::new(px_width, scaled_height), scaled_pixels)
        } else {
            (Size::new(px_width, out_height), pixels)
        }
    }

    /// Render only a specific pixel region (for viewport-based rendering)
    /// Renders the character region and crops to exact pixel bounds
    /// Render only a specific pixel region (for viewport-based rendering)
    /// Renders the character region and crops to exact pixel bounds
    pub fn render_region_to_rgba(&self, px_region: Rectangle, options: &RenderOptions, scan_lines: bool) -> (Size, Vec<u8>) {
        let Some(font) = self.font_for_render(0) else {
            log::error!("render_region_to_rgba: no font available");
            return (Size::new(0, 0), Vec::new());
        };
        let font_size = font.size();

        // Validate buffer dimensions
        if self.width() <= 0 || self.height() <= 0 {
            log::error!("render_region_to_rgba: invalid buffer dimensions {}x{}", self.width(), self.height());
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
        let char_x = char_x.clamp(0, self.width());
        let char_y = char_y.clamp(0, self.height());
        let char_width = (char_right - char_x).clamp(0, self.width() - char_x);
        let char_height = (char_bottom - char_y).clamp(0, self.height() - char_y);

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
        let palette_cache = self.palette.palette_cache_rgba();
        // Fallback colors as u32
        let default_fg = Palette::rgb_to_rgba_u32(255, 255, 255);
        let default_bg = Palette::rgb_to_rgba_u32(0, 0, 0);

        let selection_active = options.selection.is_some();
        let selection_ref = options.selection.as_ref();

        // Optional selection colors (if both are set) - pre-packed as u32
        let explicit_sel_colors = options.selection_fg.as_ref().zip(options.selection_bg.as_ref()).map(|(fg, bg)| {
            let (f_r, f_g, f_b) = fg.rgb();
            let (b_r, b_g, b_b) = bg.rgb();
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
            for x in 0..rect.width() {
                let pos = Position::new(x + rect.start.x, y + rect.start.y);
                let ch = self.char_at(pos);

                // Resolve font - use get_font_for_render for 9px font support
                let font = self.font_for_render(ch.font_page()).unwrap_or_else(|| self.font_for_render(0).unwrap());

                // Foreground index (apply bold high bit)
                let mut fg = ch.attribute.foreground();
                if ch.attribute.is_bold() && !ch.attribute.is_foreground_rgb() && !ch.attribute.is_foreground_ext() && fg < 8 {
                    fg += 8;
                }
                let bg = ch.attribute.background();

                let mut fg_is_rgb = ch.attribute.is_foreground_rgb();
                let bg_is_rgb = ch.attribute.is_background_rgb();

                if ch.attribute.is_blinking() && !options.blink_on {
                    fg = bg;
                    fg_is_rgb = bg_is_rgb;
                }

                let is_selected = selection_active && selection_ref.map(|sel| sel.is_inside(pos)).unwrap_or(false);

                // Get colors directly as u32
                let (fg_u32, bg_u32) = if is_selected {
                    if let Some((sel_fg, sel_bg)) = explicit_sel_colors {
                        (sel_fg, sel_bg)
                    } else {
                        // Invert fallback - swap fg and bg
                        let fg_color = if bg_is_rgb {
                            let (r, g, b) = ch.attribute.background_rgb();
                            Palette::rgb_to_rgba_u32(r, g, b)
                        } else if ch.attribute.is_background_ext() {
                            let idx = ch.attribute.background_ext() as usize;
                            let (r, g, b) = XTERM_256_PALETTE[idx].1.rgb();
                            Palette::rgb_to_rgba_u32(r, g, b)
                        } else {
                            let bg_idx = bg as usize;
                            if bg_idx < palette_cache.len() { palette_cache[bg_idx] } else { default_bg }
                        };
                        let bg_color = if fg_is_rgb {
                            let (r, g, b) = ch.attribute.foreground_rgb();
                            Palette::rgb_to_rgba_u32(r, g, b)
                        } else if ch.attribute.is_foreground_ext() {
                            let idx = ch.attribute.foreground_ext() as usize;
                            let (r, g, b) = XTERM_256_PALETTE[idx].1.rgb();
                            Palette::rgb_to_rgba_u32(r, g, b)
                        } else {
                            let fg_idx = fg as usize;
                            if fg_idx < palette_cache.len() { palette_cache[fg_idx] } else { default_fg }
                        };
                        (fg_color, bg_color)
                    }
                } else {
                    let fg_color = if fg_is_rgb {
                        let (r, g, b) = ch.attribute.foreground_rgb();
                        Palette::rgb_to_rgba_u32(r, g, b)
                    } else if ch.attribute.is_foreground_ext() {
                        let idx = ch.attribute.foreground_ext() as usize;
                        let (r, g, b) = XTERM_256_PALETTE[idx].1.rgb();
                        Palette::rgb_to_rgba_u32(r, g, b)
                    } else {
                        let fg_idx = fg as usize;
                        if fg_idx < palette_cache.len() { palette_cache[fg_idx] } else { default_fg }
                    };
                    let bg_color = if bg_is_rgb {
                        let (r, g, b) = ch.attribute.background_rgb();
                        Palette::rgb_to_rgba_u32(r, g, b)
                    } else if ch.attribute.is_background_ext() {
                        let idx = ch.attribute.background_ext() as usize;
                        let (r, g, b) = XTERM_256_PALETTE[idx].1.rgb();
                        Palette::rgb_to_rgba_u32(r, g, b)
                    } else {
                        let bg_idx = bg as usize;
                        if bg_idx < palette_cache.len() { palette_cache[bg_idx] } else { default_bg }
                    };
                    (fg_color, bg_color)
                };

                let bg_is_transparent = !is_selected && ch.attribute.is_background_transparent();
                let fg_is_transparent = !is_selected && ch.attribute.is_foreground_transparent();

                let cell_pixel_w = font_size.width;
                let cell_pixel_h = font_size.height;
                let base_px = x * cell_pixel_w;

                // Background fill first - leave pixels untouched for transparent background (alpha=0 holes)
                if !bg_is_transparent {
                    unsafe {
                        for cy in 0..cell_pixel_h {
                            let line_offset = (cy * line_width + base_px) as usize;
                            for cx in 0..cell_pixel_w as usize {
                                *row_pixels.get_unchecked_mut(line_offset + cx) = bg_u32;
                            }
                        }
                    }
                }

                // Foreground glyph overlay
                if !fg_is_transparent {
                    if let Some(glyph) = font.glyph(ch.ch) {
                        let max_cy = glyph.bitmap.pixels.len().min(cell_pixel_h as usize);
                        unsafe {
                            for cy in 0..max_cy {
                                let row = glyph.bitmap.pixels.get_unchecked(cy);
                                let line_offset = (cy as i32 * line_width + base_px) as usize;
                                for cx in 0..cell_pixel_w.min(row.len() as i32) as usize {
                                    if *row.get_unchecked(cx) {
                                        *row_pixels.get_unchecked_mut(line_offset + cx) = fg_u32;
                                    }
                                }
                            }
                        }
                    }
                }

                if ch.attribute.is_underlined() || ch.attribute.is_overlined() || ch.attribute.is_crossed_out() {
                    if fg_is_transparent {
                        return;
                    }
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
        let palette_cache = self.palette.palette_cache_rgba();

        let selection_active = options.selection.is_some();
        let selection_ref = options.selection.as_ref();

        // Optional selection colors (if both are set)
        let explicit_sel_colors = options.selection_fg.as_ref().zip(options.selection_bg.as_ref()).map(|(fg, bg)| {
            let (f_r, f_g, f_b) = fg.rgb();
            let (b_r, b_g, b_b) = bg.rgb();
            (Palette::rgb_to_rgba_u32(f_r, f_g, f_b), Palette::rgb_to_rgba_u32(b_r, b_g, b_b))
        });

        // Pre-scan lines to determine which are double-height
        let mut is_double_height_line = vec![false; rect.height() as usize];
        let mut is_bottom_half_line = vec![false; rect.height() as usize];

        let mut y = 0;
        while y < rect.height() {
            let abs_y = y + rect.start.y;
            // Check if any character in this line has double-height
            for x in 0..rect.width() {
                let pos = Position::new(x + rect.start.x, abs_y);
                let ch = self.char_at(pos);
                if ch.attribute.is_double_height() {
                    is_double_height_line[y as usize] = true;
                    // Mark the next line as bottom half (if it exists)
                    if (y + 1) < rect.height() {
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
                    let above_ch = self.char_at(above_pos);

                    // Only render bottom half if the character above has double-height flag
                    if !above_ch.attribute.is_double_height() {
                        // Character above is not double-height, just render blank space
                        // Background fill only (no glyph)
                        let bg_u32 = if above_ch.attribute.is_background_rgb() {
                            let (r, g, b) = above_ch.attribute.background_rgb();
                            Palette::rgb_to_rgba_u32(r, g, b)
                        } else {
                            let bg = above_ch.attribute.background();
                            let bg_idx = bg as usize;
                            if bg_idx < palette_cache.len() {
                                palette_cache[bg_idx]
                            } else {
                                Palette::rgb_to_rgba_u32(0, 0, 0)
                            }
                        };

                        let bg_is_transparent = above_ch.attribute.is_background_transparent();

                        let cell_pixel_w = font_size.width;
                        let cell_pixel_h = font_size.height;
                        let base_pixel_x = x * cell_pixel_w;
                        let base_pixel_y = y * cell_pixel_h;

                        if !bg_is_transparent {
                            unsafe {
                                for cy in 0..cell_pixel_h {
                                    let line_start = ((base_pixel_y + cy) * line_width + base_pixel_x) as usize;
                                    for cx in 0..cell_pixel_w as usize {
                                        *pixels.get_unchecked_mut(line_start + cx) = bg_u32;
                                    }
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

            let ch = self.char_at(pos);

            // Determine what to render and how
            let is_in_double_height_line = is_double_height_line[y as usize];
            let is_rendering_bottom_half = is_bottom_half_line[y as usize];
            let render_ch = if is_rendering_bottom_half {
                // We already checked above that this character has double-height
                self.char_at(Position::new(pos.x, pos.y - 1))
            } else {
                ch
            };

            // Resolve font
            let font = self.font_for_render(render_ch.font_page()).unwrap_or_else(|| self.font_for_render(0).unwrap());

            // Foreground index (apply bold high bit)
            let mut fg = render_ch.attribute.foreground();
            if render_ch.attribute.is_bold() && !render_ch.attribute.is_foreground_rgb() && !render_ch.attribute.is_foreground_ext() && fg < 8 {
                fg += 8;
            }
            let bg = render_ch.attribute.background();

            let mut fg_is_rgb = render_ch.attribute.is_foreground_rgb();
            let bg_is_rgb = render_ch.attribute.is_background_rgb();

            if render_ch.attribute.is_blinking() && !options.blink_on || render_ch.attribute.is_concealed() {
                fg = bg;
                fg_is_rgb = bg_is_rgb;
            }

            let is_selected = selection_active && selection_ref.map(|sel| sel.is_inside(pos)).unwrap_or(false);

            let (fg_u32, bg_u32) = if is_selected {
                if let Some((sel_fg_u32, sel_bg_u32)) = explicit_sel_colors {
                    (sel_fg_u32, sel_bg_u32)
                } else {
                    // Invert fallback - swap fg and bg
                    let fg_color = if bg_is_rgb {
                        let (r, g, b) = render_ch.attribute.background_rgb();
                        Palette::rgb_to_rgba_u32(r, g, b)
                    } else if render_ch.attribute.is_background_ext() {
                        let idx = render_ch.attribute.background_ext() as usize;
                        let (r, g, b) = XTERM_256_PALETTE[idx].1.rgb();
                        Palette::rgb_to_rgba_u32(r, g, b)
                    } else {
                        let bg_idx = bg as usize;
                        if bg_idx < palette_cache.len() {
                            palette_cache[bg_idx]
                        } else {
                            Palette::rgb_to_rgba_u32(0, 0, 0)
                        }
                    };
                    let bg_color = if fg_is_rgb {
                        let (r, g, b) = render_ch.attribute.foreground_rgb();
                        Palette::rgb_to_rgba_u32(r, g, b)
                    } else if render_ch.attribute.is_foreground_ext() {
                        let idx = render_ch.attribute.foreground_ext() as usize;
                        let (r, g, b) = XTERM_256_PALETTE[idx].1.rgb();
                        Palette::rgb_to_rgba_u32(r, g, b)
                    } else {
                        let fg_idx = fg as usize;
                        if fg_idx < palette_cache.len() {
                            palette_cache[fg_idx]
                        } else {
                            Palette::rgb_to_rgba_u32(255, 255, 255)
                        }
                    };
                    (fg_color, bg_color)
                }
            } else {
                let fg_color = if fg_is_rgb {
                    let (r, g, b) = render_ch.attribute.foreground_rgb();
                    Palette::rgb_to_rgba_u32(r, g, b)
                } else if render_ch.attribute.is_foreground_ext() {
                    let idx = render_ch.attribute.foreground_ext() as usize;
                    let (r, g, b) = XTERM_256_PALETTE[idx].1.rgb();
                    Palette::rgb_to_rgba_u32(r, g, b)
                } else {
                    let fg_idx = fg as usize;
                    if fg_idx < palette_cache.len() {
                        palette_cache[fg_idx]
                    } else {
                        Palette::rgb_to_rgba_u32(255, 255, 255)
                    }
                };
                let bg_color = if bg_is_rgb {
                    let (r, g, b) = render_ch.attribute.background_rgb();
                    Palette::rgb_to_rgba_u32(r, g, b)
                } else if render_ch.attribute.is_background_ext() {
                    let idx = render_ch.attribute.background_ext() as usize;
                    let (r, g, b) = XTERM_256_PALETTE[idx].1.rgb();
                    Palette::rgb_to_rgba_u32(r, g, b)
                } else {
                    let bg_idx = bg as usize;
                    if bg_idx < palette_cache.len() {
                        palette_cache[bg_idx]
                    } else {
                        Palette::rgb_to_rgba_u32(0, 0, 0)
                    }
                };
                (fg_color, bg_color)
            };

            let cell_pixel_w = font_size.width;
            let cell_pixel_h = font_size.height;
            let base_pixel_x = x * cell_pixel_w;
            let base_pixel_y = y * cell_pixel_h;

            let bg_is_transparent = !is_selected && render_ch.attribute.is_background_transparent();
            let fg_is_transparent = !is_selected && render_ch.attribute.is_foreground_transparent();

            // Background fill first
            if !bg_is_transparent {
                unsafe {
                    for cy in 0..cell_pixel_h {
                        let line_start = ((base_pixel_y + cy) * line_width + base_pixel_x) as usize;
                        for cx in 0..cell_pixel_w as usize {
                            *pixels.get_unchecked_mut(line_start + cx) = bg_u32;
                        }
                    }
                }
            }

            // Decide how to render the glyph
            if !fg_is_transparent && (is_rendering_bottom_half || (is_in_double_height_line && render_ch.attribute.is_double_height())) {
                // Render double-height (either top or bottom half)
                if let Some(glyph) = font.glyph(render_ch.ch) {
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

                            for cx in 0..cell_pixel_w.min(row.len() as i32) as usize {
                                if *row.get_unchecked(cx) {
                                    *pixels.get_unchecked_mut(line_start + cx) = fg_u32;
                                }
                            }
                        }
                    }
                }
            } else if !fg_is_transparent {
                // Normal height rendering (including non-double-height chars in double-height lines)
                if let Some(glyph) = font.glyph(render_ch.ch) {
                    let max_cy = glyph.bitmap.pixels.len().min(cell_pixel_h as usize);
                    unsafe {
                        for cy in 0..max_cy {
                            let row = glyph.bitmap.pixels.get_unchecked(cy);
                            let line_start = ((base_pixel_y + cy as i32) * line_width + base_pixel_x) as usize;

                            for cx in 0..cell_pixel_w.min(row.len() as i32) as usize {
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
            if !fg_is_transparent && !is_rendering_bottom_half && (ch.attribute.is_underlined() || ch.attribute.is_overlined() || ch.attribute.is_crossed_out())
            {
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
        for y in 0..rect.height() {
            for x in 0..rect.width() {
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
                let sx_char = layer.offset().x + sixel.position.x;
                let sy_char = layer.offset().y + sixel.position.y;

                // Calculate sixel dimensions in character coordinates
                let sixel_width_chars = (sixel.width() + font_dims.width - 1) / font_dims.width;
                let sixel_height_chars = (sixel.height() + font_dims.height - 1) / font_dims.height;

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
                let max_y = (dest_y_px + sixel.height() - skip_y_px).min(px_height);
                let visible_width = (sixel.width() - skip_x_px).min(line_width - dest_x_px);

                if visible_width <= 0 {
                    continue;
                }

                let mut sixel_line = skip_y_px as usize;

                // Sixel data as u32 slice
                let sixel_u32 = unsafe { std::slice::from_raw_parts(sixel.picture_data.as_ptr().cast::<u32>(), sixel.picture_data.len() / 4) };
                let sixel_line_width = sixel.width() as usize;
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
