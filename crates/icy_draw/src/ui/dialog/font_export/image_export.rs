//! Image export for bitmap fonts
//!
//! Creates a 16x16 grid image of all 256 characters

use icy_engine::BitFont;
use std::path::Path;

/// Export a font to an image file
///
/// Creates a 16x16 grid of characters where each cell is font_width x font_height pixels.
/// The resulting image is (16 * font_width) x (16 * font_height) pixels.
pub fn export_font_to_image(font: &BitFont, path: &Path, format: image::ImageFormat) -> Result<(), String> {
    let font_width = font.size().width as u32;
    let font_height = font.size().height as u32;

    // Image dimensions: 16x16 grid of characters
    let img_width = 16 * font_width;
    let img_height = 16 * font_height;

    // Create grayscale image (white background, black foreground)
    let mut img = image::GrayImage::new(img_width, img_height);

    // Fill with white (background)
    for pixel in img.pixels_mut() {
        *pixel = image::Luma([255u8]);
    }

    // Draw each character
    for ch_code in 0..256u32 {
        let row = ch_code / 16;
        let col = ch_code % 16;

        // Get glyph data
        let ch = unsafe { char::from_u32_unchecked(ch_code) };
        let glyph = font.glyph(ch);

        // Calculate position in image
        let base_x = col * font_width;
        let base_y = row * font_height;

        // Draw the glyph pixels
        let bitmap_pixels = glyph.to_bitmap_pixels();
        for (y, row_pixels) in bitmap_pixels.iter().enumerate() {
            if y >= font_height as usize {
                break;
            }

            for (x, &is_set) in row_pixels.iter().enumerate() {
                if x >= font_width as usize {
                    break;
                }

                if is_set {
                    let px = base_x + x as u32;
                    let py = base_y + y as u32;
                    if px < img_width && py < img_height {
                        img.put_pixel(px, py, image::Luma([0u8])); // Black for foreground
                    }
                }
            }
        }
    }

    // Save the image
    img.save_with_format(path, format).map_err(|e| format!("Failed to save image: {}", e))
}
