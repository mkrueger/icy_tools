//! Font import routines shared by both frontends.

#[path = "ui/dialog/font_import/image_import.rs"]
pub mod image_import;
#[path = "ui/dialog/font_import/ttf_import.rs"]
pub mod ttf_import;

use icy_engine::BitFont;
use icy_engine_edit::bitfont::MAX_FONT_HEIGHT;

pub const NATIVE_EXTENSIONS: &[&str] = &[
    "yaff", "psf", "psf2", "psfu", "f06", "f07", "f08", "f09", "f10", "f11", "f12", "f13", "f14", "f15", "f16", "f17", "f18", "f19", "f20", "f22", "f24",
    "f26", "f28", "f30", "f32",
];
pub const TTF_EXTENSIONS: &[&str] = &["ttf", "otf", "ttc", "otc"];
pub const IMAGE_EXTENSIONS: &[&str] = &["png", "jpg", "jpeg", "gif", "bmp", "webp", "tga", "tiff", "ico"];

pub fn is_native_font_extension(ext: &str) -> bool {
    NATIVE_EXTENSIONS.contains(&ext)
}

pub fn is_ttf_extension(ext: &str) -> bool {
    TTF_EXTENSIONS.contains(&ext)
}

pub fn is_image_extension(ext: &str) -> bool {
    IMAGE_EXTENSIONS.contains(&ext)
}

/// Extracts PCMag FontEdit and Fontraption (TSR or non-TSR) DOS COM fonts.
pub fn parse_com_font(name: &str, data: &[u8]) -> Result<BitFont, String> {
    if data.len() < 0x64 {
        return Err("COM file too small to contain font data".to_string());
    }
    let checksum = data[..16]
        .chunks_exact(2)
        .map(|chunk| u16::from_le_bytes([chunk[0], chunk[1]]))
        .fold(0u16, u16::wrapping_add);
    let (height, offset) = if checksum == 0x8696 {
        (data[0x32], 0x63)
    } else if checksum == 0xEF10 {
        (data[0x15], 0x19)
    } else if &data[0x28..0x2C] == b"VILE" {
        (data[0x5D], 0x63)
    } else {
        return Err("Unknown COM font format (not PCMag FontEdit or Fontraption)".to_string());
    };
    if height == 0 || height as i32 > MAX_FONT_HEIGHT {
        return Err(format!("Invalid font height: {height} (must be 1-{MAX_FONT_HEIGHT})"));
    }
    let end = offset + 256 * height as usize;
    if data.len() < end {
        return Err(format!("COM file too small: need {end} bytes for font data, file has {}", data.len()));
    }
    Ok(BitFont::create_8(name, 8, height, &data[offset..end]))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn converters_reject_dimensions_outside_bitmap_editor_limits() {
        let path = std::path::Path::new("unused");
        for (width, height) in [(0, 16), (9, 16), (8, 0), (8, 33), (i32::MAX, i32::MAX)] {
            let expected = if !(1..=8).contains(&width) { "Font width" } else { "Font height" };
            assert!(image_import::import_font_from_image(path, width, height, true)
                .unwrap_err()
                .starts_with(expected));
            assert!(ttf_import::import_font_from_ttf(path, width, height).unwrap_err().starts_with(expected));
        }
    }

    #[test]
    fn imports_all_com_formats_and_rejects_truncated_data() {
        for (checksum, height_offset, data_offset) in [(0x8696u16, 0x32, 0x63), (0xEF10, 0x15, 0x19), (0, 0x5D, 0x63)] {
            let mut data = vec![0; data_offset + 256 * 16];
            data[..2].copy_from_slice(&checksum.to_le_bytes());
            if checksum == 0 {
                data[0x28..0x2C].copy_from_slice(b"VILE");
            }
            data[height_offset] = 16;
            data[data_offset..].fill(0xA5);
            let font = parse_com_font("Test", &data).unwrap();
            assert_eq!(font.size(), icy_engine::Size::new(8, 16));
            assert_eq!(font.convert_to_u8_data(), vec![0xA5; 256 * 16]);
            data.pop();
            assert!(parse_com_font("Test", &data).is_err());
            data[height_offset] = 0;
            assert!(parse_com_font("Test", &data).is_err());
        }
        assert!(parse_com_font("Test", &[]).is_err());
        assert!(parse_com_font("Test", &[0; 256]).is_err());
    }
}
