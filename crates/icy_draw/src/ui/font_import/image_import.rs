//! Image to font conversion
//!
//! Converts a raster image (assumed to be a 16x16 grid of characters) to a BitFont

use std::path::Path;

use icy_engine::BitFont;

/// Import a font from an image file
///
/// The image is expected to be a 16x16 grid of characters.
/// Each cell will be scaled to the specified font width/height.
/// The image is converted to 2 colors using simple thresholding.
pub fn import_font_from_image(path: &Path, font_width: i32, font_height: i32) -> Result<BitFont, String> {
    // Validate dimensions
    if !(4..=16).contains(&font_width) {
        return Err(format!("Font width must be 4-16, got {}", font_width));
    }
    if !(4..=32).contains(&font_height) {
        return Err(format!("Font height must be 4-32, got {}", font_height));
    }

    // Load image
    let img = image::open(path).map_err(|e| format!("Failed to load image: {}", e))?;
    let img = img.to_luma8();

    let img_width = img.width() as i32;
    let img_height = img.height() as i32;

    // Calculate cell size in the source image
    let cell_width = img_width / 16;
    let cell_height = img_height / 16;

    if cell_width < 1 || cell_height < 1 {
        return Err("Image too small (must be at least 16x16 pixels)".to_string());
    }

    // Create glyph data for all 256 characters
    let mut glyphs: Vec<Vec<u8>> = Vec::with_capacity(256);

    for ch_code in 0..256 {
        let row = ch_code / 16;
        let col = ch_code % 16;

        // Source rectangle in image
        let src_x = col * cell_width;
        let src_y = row * cell_height;

        // Create glyph bitmap
        let mut glyph_data = vec![0u8; font_height as usize];

        for py in 0..font_height {
            let mut row_byte = 0u8;

            for px in 0..font_width {
                // Map pixel position to source image
                let src_px = src_x + (px * cell_width / font_width);
                let src_py = src_y + (py * cell_height / font_height);

                // Sample the pixel (with bounds check)
                let is_set = if src_px < img_width && src_py < img_height {
                    let pixel = img.get_pixel(src_px as u32, src_py as u32);
                    pixel.0[0] >= 128 // Bright pixels are "set" (foreground)
                } else {
                    false
                };

                if is_set {
                    row_byte |= 1 << (7 - px);
                }
            }

            glyph_data[py as usize] = row_byte;
        }

        glyphs.push(glyph_data);
    }

    // Build the BitFont
    let font_name = path.file_stem().and_then(|s| s.to_str()).unwrap_or("Imported").to_string();

    // Create font from raw glyph data
    let mut all_data = Vec::with_capacity(256 * font_height as usize);
    for glyph in &glyphs {
        all_data.extend_from_slice(glyph);
    }

    let font = BitFont::create_8(&font_name, font_width as u8, font_height as u8, &all_data);

    Ok(font)
}
