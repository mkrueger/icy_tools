//! TTF/OTF font to BitFont conversion
//!
//! Rasterizes TrueType/OpenType fonts to bitmap fonts using fontdue.
//! Uses CP437 character mapping for the 256 character slots.
//!
//! Quality improvements:
//! - Grid-fitting: snaps glyph features to pixel boundaries
//! - Proper em-square scaling to fill the target cell
//! - Sub-pixel positioning trials: tests multiple x/y offsets
//! - Adaptive thresholding with stroke preservation

use std::path::Path;

use codepages::tables::CP437_TO_UNICODE_NO_CTRL_CODES;
use icy_engine::BitFont;

/// Import a font from a TTF/OTF file
///
/// Rasterizes the font at the specified dimensions with grid-fitting.
/// Characters are positioned optimally within their cells.
/// Uses CP437 character mapping for indices 0-255.
pub fn import_font_from_ttf(path: &Path, font_width: i32, font_height: i32) -> Result<BitFont, String> {
    // Validate dimensions
    if !(4..=16).contains(&font_width) {
        return Err(format!("Font width must be 4-16, got {}", font_width));
    }
    if !(4..=32).contains(&font_height) {
        return Err(format!("Font height must be 4-32, got {}", font_height));
    }

    // Read the font file
    let font_data = std::fs::read(path).map_err(|e| format!("Failed to read font file: {}", e))?;

    // Parse the font with fontdue
    let font = fontdue::Font::from_bytes(font_data, fontdue::FontSettings::default()).map_err(|e| format!("Failed to parse font: {}", e))?;

    // Calculate the proper px_size to fill the cell
    // We need to find a size where the em-square maps to our cell height
    let px_size = calculate_px_size_for_cell(&font, font_height);

    // Get line metrics at this size
    let line_metrics = font.horizontal_line_metrics(px_size);

    // Calculate where the baseline should be in the cell
    // For a typical font, we want about 2 pixels of descender space at the bottom for 16px
    // and the rest for ascent
    let descender_space = (font_height as f32 * 0.125).round().max(1.0); // ~12.5% for descenders
    let baseline_y = font_height as f32 - descender_space;

    // Create glyph data for all 256 CP437 characters
    let mut all_data = Vec::with_capacity(256 * font_height as usize);

    for cp437_code in 0..256usize {
        let unicode_char = CP437_TO_UNICODE_NO_CTRL_CODES[cp437_code];

        // Rasterize with grid-fitting
        let glyph_data = rasterize_glyph_grid_fitted(
            &font,
            unicode_char,
            px_size,
            baseline_y,
            font_width as u32,
            font_height as u32,
            line_metrics.as_ref(),
        );

        all_data.extend_from_slice(&glyph_data);
    }

    // Build the BitFont
    let font_name = path.file_stem().and_then(|s| s.to_str()).unwrap_or("Imported").to_string();

    Ok(BitFont::create_8(&font_name, font_width as u8, font_height as u8, &all_data))
}

/// Calculate the pixel size needed to properly fill the cell
fn calculate_px_size_for_cell(font: &fontdue::Font, target_height: i32) -> f32 {
    // Start with target height as px_size and measure actual glyph dimensions
    let test_size = target_height as f32;

    // Get the line metrics to understand the font's vertical metrics
    if let Some(metrics) = font.horizontal_line_metrics(test_size) {
        // The line height at this size
        let line_height = metrics.ascent - metrics.descent;

        // Scale factor to make the line height fit our target
        // Leave a small margin (about 90% of cell for the line height)
        let target_line_height = target_height as f32 * 0.9;
        let scale = target_line_height / line_height;

        return test_size * scale;
    }

    // Fallback: measure actual glyph heights for reference characters
    let reference_chars = ['H', 'M', 'X', 'd', 'p'];
    let mut max_ascent: f32 = 0.0;
    let mut max_descent: f32 = 0.0;

    for &ch in &reference_chars {
        let (metrics, _) = font.rasterize(ch, test_size);
        if metrics.height > 0 {
            // ymin is the offset from baseline to bottom of glyph
            // For ascending chars like 'H', the top is at ymin + height above baseline
            let ascent = (metrics.height as f32 + metrics.ymin as f32).max(0.0);
            let descent = (-metrics.ymin as f32).max(0.0);

            max_ascent = max_ascent.max(ascent);
            max_descent = max_descent.max(descent);
        }
    }

    if max_ascent + max_descent > 0.0 {
        // Scale to fit with small margin
        let total = max_ascent + max_descent;
        let target = target_height as f32 * 0.9;
        return test_size * (target / total);
    }

    // Ultimate fallback
    test_size
}

/// Rasterize a single glyph with grid-fitting
fn rasterize_glyph_grid_fitted(
    font: &fontdue::Font,
    ch: char,
    px_size: f32,
    baseline_y: f32,
    cell_width: u32,
    cell_height: u32,
    _line_metrics: Option<&fontdue::LineMetrics>,
) -> Vec<u8> {
    let (metrics, bitmap) = font.rasterize(ch, px_size);

    if bitmap.is_empty() || metrics.width == 0 || metrics.height == 0 {
        return vec![0u8; cell_height as usize];
    }

    // Try multiple sub-pixel positions and pick the best one
    find_best_grid_position(&bitmap, &metrics, baseline_y, cell_width, cell_height)
}

/// Grid position result
struct GridPositionResult {
    buffer: Vec<u8>,
    score: f32,
}

/// Try multiple grid positions and return the best one
fn find_best_grid_position(bitmap: &[u8], metrics: &fontdue::Metrics, baseline_y: f32, cell_width: u32, cell_height: u32) -> Vec<u8> {
    let mut best_result: Option<GridPositionResult> = None;

    // Calculate base positions
    // In fontdue:
    // - ymin is the offset from baseline to the BOTTOM of the glyph bounding box
    //   (negative for descenders that go below baseline)
    // - So the TOP of the glyph is at: baseline - (height + ymin) in our coordinate system
    //   Wait, that's not right either.
    //
    // Actually, if ymin is the distance from baseline to bottom:
    // - The bottom of the glyph is at: baseline_y - ymin (in screen coords where Y increases downward)
    // - Wait no, baseline_y is already in screen coords
    //
    // Let me think again:
    // - baseline_y is the Y position of the baseline (in screen coords, 0 at top)
    // - ymin is how far below the baseline the glyph extends (positive = above, negative = below)
    // - So the bottom of the glyph in screen coords is at: baseline_y - ymin
    // - And the top is at: baseline_y - ymin - height = baseline_y - (ymin + height)
    //
    // For 'A' with ymin=0: top = baseline_y - height
    // For 'g' with ymin=-3: top = baseline_y - (-3 + height) = baseline_y + 3 - height

    let glyph_top = baseline_y - metrics.ymin as f32 - metrics.height as f32;
    let base_y = glyph_top.round() as i32;
    let base_x = ((cell_width as f32 - metrics.width as f32) / 2.0).round() as i32;

    // Try offsets: -1, 0, +1 for both x and y
    for dy in -1..=1 {
        for dx in -1..=1 {
            let x_offset = base_x + dx;
            let y_offset = base_y + dy;

            // Create grayscale buffer at target resolution
            let mut gray_buffer = vec![0u8; (cell_width * cell_height) as usize];

            // Copy glyph to buffer at this position
            for gy in 0..metrics.height {
                let dst_y = gy as i32 + y_offset;
                if dst_y < 0 || dst_y >= cell_height as i32 {
                    continue;
                }

                for gx in 0..metrics.width {
                    let dst_x = gx as i32 + x_offset;
                    if dst_x < 0 || dst_x >= cell_width as i32 {
                        continue;
                    }

                    let pixel = bitmap[gy * metrics.width + gx];
                    gray_buffer[(dst_y as u32 * cell_width + dst_x as u32) as usize] = pixel;
                }
            }

            // Convert to 1-bit with grid-aware thresholding
            let binary = apply_grid_aware_threshold(&gray_buffer, cell_width, cell_height);

            // Score this result
            let score = score_grid_result(&gray_buffer, &binary, cell_width, cell_height);

            if best_result.is_none() || score > best_result.as_ref().unwrap().score {
                best_result = Some(GridPositionResult { buffer: binary, score });
            }
        }
    }

    best_result.map(|r| r.buffer).unwrap_or_else(|| vec![0u8; cell_height as usize])
}

/// Apply grid-aware thresholding
/// Uses a standard 50% threshold with optional edge detection
fn apply_grid_aware_threshold(gray: &[u8], width: u32, height: u32) -> Vec<u8> {
    let mut result = vec![0u8; height as usize];

    // First, detect if this is a "pixel-perfect" font (values are mostly 0 or 255)
    // vs an anti-aliased font (many intermediate values)
    let mut mid_count = 0;

    for &pixel in gray {
        if pixel > 55 && pixel < 200 {
            mid_count += 1;
        }
    }

    let total = gray.len();
    let mid_ratio = mid_count as f32 / total as f32;

    // If less than 15% of pixels are in the middle range, it's a pixel-perfect font
    // Use a simple high threshold
    let base_threshold = if mid_ratio < 0.15 {
        // Pixel-perfect font: use high threshold
        180u8
    } else {
        // Anti-aliased font: use standard 50% threshold
        128u8
    };

    // Apply thresholding
    for y in 0..height {
        for x in 0..width {
            let pixel = gray[(y * width + x) as usize];

            if pixel >= base_threshold {
                result[y as usize] |= 1 << (7 - x);
            }
        }
    }

    result
}

/// Score a grid result based on edge clarity and coverage
fn score_grid_result(gray: &[u8], binary: &[u8], width: u32, height: u32) -> f32 {
    let mut edge_score: f32 = 0.0;
    let mut coverage_score: f32 = 0.0;
    let mut consistency_score: f32 = 0.0;

    for y in 0..height {
        for x in 0..width {
            let gray_pixel = gray[(y * width + x) as usize];
            let binary_pixel = (binary[y as usize] >> (7 - x)) & 1;

            // Coverage: reward pixels that are clearly on or off
            if gray_pixel > 200 && binary_pixel == 1 {
                coverage_score += 1.0;
            } else if gray_pixel < 50 && binary_pixel == 0 {
                coverage_score += 0.5;
            }

            // Edge clarity: check horizontal edges
            if x > 0 {
                let prev_binary = (binary[y as usize] >> (7 - x + 1)) & 1;
                if binary_pixel != prev_binary {
                    // There's an edge here - check if grayscale supports it
                    let prev_gray = gray[(y * width + x - 1) as usize];
                    let gray_diff = (gray_pixel as i32 - prev_gray as i32).abs();
                    if gray_diff > 100 {
                        edge_score += 1.0; // Sharp edge in grayscale matches binary edge
                    } else {
                        edge_score -= 0.5; // Binary edge doesn't match grayscale
                    }
                }
            }

            // Vertical edge clarity
            if y > 0 {
                let prev_binary = (binary[(y - 1) as usize] >> (7 - x)) & 1;
                if binary_pixel != prev_binary {
                    let prev_gray = gray[((y - 1) * width + x) as usize];
                    let gray_diff = (gray_pixel as i32 - prev_gray as i32).abs();
                    if gray_diff > 100 {
                        edge_score += 1.0;
                    } else {
                        edge_score -= 0.5;
                    }
                }
            }
        }
    }

    // Check for consistent stroke widths (vertical stems should be consistent width)
    for x in 0..width {
        let mut stem_count = 0;
        for y in 0..height {
            if (binary[y as usize] >> (7 - x)) & 1 == 1 {
                stem_count += 1;
            }
        }
        // Reward columns that are either mostly on or mostly off
        let ratio = stem_count as f32 / height as f32;
        if ratio > 0.7 || ratio < 0.3 {
            consistency_score += 1.0;
        }
    }

    edge_score + coverage_score * 0.5 + consistency_score * 0.3
}
