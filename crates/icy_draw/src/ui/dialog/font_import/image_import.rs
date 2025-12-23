//! Image to font conversion
//!
//! Converts a raster image (assumed to be a 16x16 grid of characters) to a BitFont.
//! Optionally uses Floyd-Steinberg dithering via quantette for high-quality 2-color conversion.

use std::path::Path;

use icy_engine::BitFont;
use quantette::{deps::palette::Srgb, dither::FloydSteinberg, Image, PaletteSize, Pipeline};

/// Import a font from an image file
///
/// The image is expected to be a 16x16 grid of characters.
/// Each cell will be scaled to the specified font width/height.
/// When `use_dithering` is true, Floyd-Steinberg dithering is applied for smooth gradients.
/// When false, a simple threshold (50% brightness) is used for sharp edges.
pub fn import_font_from_image(path: &Path, font_width: i32, font_height: i32, use_dithering: bool) -> Result<BitFont, String> {
    // Validate dimensions
    if font_width < 1 {
        return Err(format!("Font width must be positive, got {}", font_width));
    }
    if font_height < 1 {
        return Err(format!("Font height must be positive, got {}", font_height));
    }

    // Load image
    let img = image::open(path).map_err(|e| format!("Failed to load image: {}", e))?;
    let rgb_img = img.to_rgb8();

    let img_width = rgb_img.width() as i32;
    let img_height = rgb_img.height() as i32;

    // Calculate cell size in the source image
    let cell_width = img_width / 16;
    let cell_height = img_height / 16;

    if cell_width < 1 || cell_height < 1 {
        return Err("Image too small (must be at least 16x16 pixels)".to_string());
    }

    // Convert image pixels to Srgb<u8> for quantette
    let pixels: Vec<Srgb<u8>> = rgb_img.pixels().map(|p| Srgb::new(p.0[0], p.0[1], p.0[2])).collect();

    let quantette_img = Image::new(img_width as u32, img_height as u32, pixels).map_err(|e| format!("Failed to create quantette image: {}", e))?;

    // Use Pipeline with 2 colors and optionally Floyd-Steinberg dithering
    let palette_size = PaletteSize::try_from(2u16).map_err(|_| "Failed to create palette size")?;

    let indexed = if use_dithering {
        Pipeline::new()
            .palette_size(palette_size)
            .ditherer(FloydSteinberg::new())
            .input_image(quantette_img.as_ref())
            .output_srgb8_indexed_image()
    } else {
        Pipeline::new()
            .palette_size(palette_size)
            .input_image(quantette_img.as_ref())
            .output_srgb8_indexed_image()
    };

    // Determine which palette index is "white" (foreground)
    // The palette contains 2 colors - we need to find which one is brighter
    let palette = indexed.palette();
    let white_index = if palette.len() >= 2 {
        let luma0 = palette[0].red as u32 + palette[0].green as u32 + palette[0].blue as u32;
        let luma1 = palette[1].red as u32 + palette[1].green as u32 + palette[1].blue as u32;
        if luma1 > luma0 {
            1u8
        } else {
            0u8
        }
    } else if !palette.is_empty() {
        0u8
    } else {
        return Err("Quantization produced empty palette".to_string());
    };

    let indexed_pixels = indexed.indices();

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

                // Sample the indexed pixel (with bounds check)
                let is_set = if src_px < img_width && src_py < img_height {
                    let pixel_idx = (src_py * img_width + src_px) as usize;
                    indexed_pixels.get(pixel_idx).copied().unwrap_or(0) == white_index
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
