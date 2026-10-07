//! Bitmap font export formats shared by both frontends.

#[path = "ui/dialog/font_export/image_export.rs"]
pub mod image_export;

use icy_engine::BitFont;
use icy_engine_edit::bitfont::{MAX_FONT_HEIGHT, MAX_FONT_WIDTH};
use std::io::Cursor;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum FontExportFormat {
    #[default]
    Png,
    Bmp,
    Psf,
    Raw,
    Yaff,
    AnsiDcs,
    Com,
}

impl FontExportFormat {
    pub const ALL: [Self; 7] = [Self::Png, Self::Bmp, Self::Psf, Self::Raw, Self::Yaff, Self::AnsiDcs, Self::Com];

    pub fn all() -> Vec<Self> {
        Self::ALL.to_vec()
    }

    pub fn extension(self, height: i32) -> String {
        match self {
            Self::Png => "png".into(),
            Self::Bmp => "bmp".into(),
            Self::Psf => "psf".into(),
            Self::Raw => format!("f{height:02}"),
            Self::Yaff => "yaff".into(),
            Self::AnsiDcs => "ans".into(),
            Self::Com => "com".into(),
        }
    }

    pub fn display_name(self) -> &'static str {
        match self {
            Self::Png => "PNG Image",
            Self::Bmp => "BMP Image",
            Self::Psf => "PSF (Linux Console)",
            Self::Raw => "Raw Binary (.fXX)",
            Self::Yaff => "YAFF (Text-based)",
            Self::AnsiDcs => "ANSI DCS",
            Self::Com => "DOS COM Executable",
        }
    }
}

impl std::fmt::Display for FontExportFormat {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.write_str(self.display_name())
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum ComExportFormat {
    #[default]
    NonTsr,
    Tsr40Col,
    Tsr80Col,
    TsrAll,
}

impl ComExportFormat {
    pub const ALL: [Self; 4] = [Self::NonTsr, Self::Tsr40Col, Self::Tsr80Col, Self::TsrAll];

    pub fn all() -> Vec<Self> {
        Self::ALL.to_vec()
    }

    pub fn display_name(self) -> &'static str {
        match self {
            Self::NonTsr => "Non-TSR (simple)",
            Self::Tsr40Col => "TSR (40-column)",
            Self::Tsr80Col => "TSR (80-column)",
            Self::TsrAll => "TSR (all modes)",
        }
    }
}

impl std::fmt::Display for ComExportFormat {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.write_str(self.display_name())
    }
}

pub fn encode(font: &BitFont, format: FontExportFormat, com_format: ComExportFormat) -> Result<Vec<u8>, String> {
    let size = font.size();
    if !(1..=MAX_FONT_WIDTH).contains(&size.width) || !(1..=MAX_FONT_HEIGHT).contains(&size.height) {
        return Err(crate::fl!("font-export-invalid-size", width = MAX_FONT_WIDTH, height = MAX_FONT_HEIGHT));
    }
    match format {
        FontExportFormat::Png | FontExportFormat::Bmp => {
            let format = if format == FontExportFormat::Png {
                image::ImageFormat::Png
            } else {
                image::ImageFormat::Bmp
            };
            let mut output = Cursor::new(Vec::new());
            image::DynamicImage::ImageLuma8(image_export::font_image(font))
                .write_to(&mut output, format)
                .map_err(|error| error.to_string())?;
            Ok(output.into_inner())
        }
        FontExportFormat::Psf => font.to_psf2_bytes().map_err(|error| error.to_string()),
        FontExportFormat::Raw => Ok(font.convert_to_u8_data()),
        FontExportFormat::Yaff => Ok(libyaff::to_yaff_string(&font.to_yaff_font()).into_bytes()),
        FontExportFormat::AnsiDcs => Ok(font.encode_as_ansi(0).into_bytes()),
        FontExportFormat::Com => {
            let data = font.convert_to_u8_data();
            match com_format {
                ComExportFormat::NonTsr => Ok(non_tsr_com(size.height as u8, &data)),
                ComExportFormat::Tsr40Col => Ok(tsr_com(size.height as u8, &data, true, false)),
                ComExportFormat::Tsr80Col => Ok(tsr_com(size.height as u8, &data, false, true)),
                ComExportFormat::TsrAll => Ok(tsr_com(size.height as u8, &data, true, true)),
            }
        }
    }
}

fn non_tsr_com(height: u8, data: &[u8]) -> Vec<u8> {
    // Fontraption's 25-byte INT 10h font loader; height is at 0x15.
    #[rustfmt::skip]
    let header = [
        0x56, 0x49, 0x4C, 0x45, 0x1A, 0x00, 0x83, 0xC4, 0x03, 0xB8,
        0x10, 0x11, 0xBD, 0x19, 0x01, 0xB9, 0x00, 0x01, 0x99, 0xBB,
        0x00, height, 0xCD, 0x10, 0xC3,
    ];
    let mut output = Vec::with_capacity(header.len() + data.len());
    output.extend_from_slice(&header);
    output.extend_from_slice(data);
    output
}

fn tsr_com(height: u8, data: &[u8], modes_40: bool, modes_80: bool) -> Vec<u8> {
    // Fontraption's resident INT 10h handler and initialization tail.
    let offset_init = 0x63 + data.len();
    #[rustfmt::skip]
    let mut header = vec![
        0xE9, 0x00, 0x00,
        0x00, 0x00, 0x80, 0xFC, 0x00, 0x75, 0x10, 0x3C, 0x03,
        0x77, 0xF2, 0x53, 0x89, 0xC3, 0x2E, 0x8A, 0x9F, 0x2D, 0x01, 0x4B, 0x5B,
        0x74, 0x17, 0x3D, 0x00, 0x12, 0x75, 0xE1, 0x80, 0xFB, 0x21, 0x75, 0xDC,
        0xB0, 0x21, 0xCF, 0x0D,
        0x56, 0x49, 0x4C, 0x45, 0x1A,
        0x00, 0x00, 0x00, 0x00,
        0x9C, 0x0E, 0xE8, 0xCA, 0xFF, 0x50, 0x51, 0x52, 0x53, 0x55, 0x56,
        0x57, 0x1E, 0x06, 0x0E, 0x0E, 0x1F, 0x07, 0xE8, 0x0F, 0x00, 0x9C, 0x0E,
        0xE8, 0xB5, 0xFF, 0x07, 0x1F, 0x5F, 0x5E, 0x5D, 0x5B, 0x5A, 0x59, 0x58,
        0xCF, 0xB8, 0x10, 0x11, 0xBD, 0x63, 0x01, 0xBB, 0x00,
        height, 0xB9, 0x00, 0x01, 0x99, 0xC3,
    ];
    let jump = (offset_init - 3) as u16;
    header[1..3].copy_from_slice(&jump.to_le_bytes());
    header[0x2D..0x2F].fill(u8::from(modes_40));
    header[0x2F..0x31].fill(u8::from(modes_80));

    #[rustfmt::skip]
    let mut tail = vec![
        0xB8, 0x00, 0x12, 0xB3, 0x21, 0xCD, 0x10, 0x3C, 0x21, 0x75, 0x3E, 0xBA,
        0x00, 0x00,
        0xE8, 0x77, 0x00, 0x31, 0xC0, 0x8E, 0xD8, 0x8E, 0xC0, 0xC5,
        0x1E, 0x40, 0x00, 0x81, 0x7F, 0x23, 0x56, 0x49, 0x75, 0x1E, 0x81, 0x7F,
        0x25, 0x4C, 0x45, 0x75, 0x17, 0xFA, 0xBE, 0x01, 0x01, 0xBF, 0x40, 0x00,
        0xA5, 0xA5, 0xFB, 0x1E, 0x07, 0xB4, 0x49, 0xCD, 0x21, 0x72, 0x05, 0xBA,
        0x00, 0x00,
        0xEB, 0x03, 0xBA,
        0x00, 0x00,
        0x0E, 0x1F, 0xE8, 0x40, 0x00,
        0xC3, 0xB8, 0x10, 0x35, 0xCD, 0x21, 0xFE, 0x06, 0x00, 0x01, 0x89, 0x1E,
        0x01, 0x01, 0x8C, 0x06, 0x03, 0x01, 0xBA, 0x05, 0x01, 0xB4, 0x25, 0xCD,
        0x21, 0xB4, 0x0F, 0xCD, 0x10, 0x3C, 0x03, 0x77, 0x0F, 0xBB, 0x2D, 0x01,
        0xD7, 0xFE, 0xC8, 0x75, 0x07, 0x0E, 0x07, 0xE8,
        0x00, 0x00,
        0xCD, 0x10,
        0x8E, 0x06, 0x2C, 0x00, 0xB4, 0x49, 0xCD, 0x21, 0xBA,
        0x00, 0x00,
        0xB8, 0x00, 0x31, 0xCD, 0x21, 0xB4, 0x09, 0xCD, 0x21, 0xC3,
        b'L', b'a', b's', b't', b' ', b'T', b'S', b'R', b' ', b'f', b'o', b'n', b't', b' ', b'$',
        b'r', b'e', b'm', b'o', b'v', b'e', b'd', 13, 10, b'$',
        b'u', b'n', b'r', b'e', b'm', b'o', b'v', b'a', b'b', b'l', b'e', b'!', 13, 10, b'$',
    ];
    for (patch, value) in [
        (0x0C, (offset_init + 0x18D) as u16),
        (0x3C, (offset_init + 0x19C) as u16),
        (0x41, (offset_init + 0x1A6) as u16),
        (0x74, (0u16).wrapping_sub((offset_init + 0x21) as u16)),
        (0x81, ((offset_init + 0x0F) >> 4) as u16),
    ] {
        tail[patch..patch + 2].copy_from_slice(&value.to_le_bytes());
    }
    let mut output = Vec::with_capacity(header.len() + data.len() + tail.len());
    output.extend_from_slice(&header);
    output.extend_from_slice(data);
    output.extend_from_slice(&tail);
    output
}

#[cfg(test)]
mod tests {
    use super::*;
    use base64::Engine;

    #[test]
    fn native_exports_preserve_all_256_glyphs() {
        let font = BitFont::default();
        let raw = font.convert_to_u8_data();
        for format in [FontExportFormat::Psf, FontExportFormat::Raw, FontExportFormat::Yaff] {
            let bytes = encode(&font, format, ComExportFormat::NonTsr).unwrap();
            let loaded = BitFont::from_bytes("Export", &bytes).unwrap();
            assert_eq!(loaded.size(), font.size(), "{format}");
            assert_eq!(loaded.convert_to_u8_data(), raw, "{format}");
        }
        assert_eq!(encode(&font, FontExportFormat::Raw, ComExportFormat::NonTsr).unwrap(), raw);
        assert_eq!(FontExportFormat::Raw.extension(8), "f08");
        assert_eq!(FontExportFormat::Raw.extension(32), "f32");
    }

    #[test]
    fn image_exports_have_exact_glyph_grid_dimensions_and_pixels() {
        let font = BitFont::default();
        let size = font.size();
        for (format, image_format) in [
            (FontExportFormat::Png, image::ImageFormat::Png),
            (FontExportFormat::Bmp, image::ImageFormat::Bmp),
        ] {
            let bytes = encode(&font, format, ComExportFormat::NonTsr).unwrap();
            assert_eq!(image::guess_format(&bytes).unwrap(), image_format);
            let image = image::load_from_memory(&bytes).unwrap().to_luma8();
            assert_eq!(image.dimensions(), ((16 * size.width) as u32, (16 * size.height) as u32));
            for code in 0..256u32 {
                let glyph = font.glyph(char::from(code as u8));
                for y in 0..size.height as usize {
                    for x in 0..size.width as usize {
                        let pixel = image
                            .get_pixel((code % 16) * size.width as u32 + x as u32, (code / 16) * size.height as u32 + y as u32)
                            .0[0];
                        assert_eq!(pixel, if glyph.get_pixel(x, y) { 0 } else { 255 }, "{format}: glyph {code}, pixel {x},{y}");
                    }
                }
            }
        }
    }

    #[test]
    fn ansi_export_contains_the_exact_cterm_upload_payload() {
        let font = BitFont::default();
        let bytes = encode(&font, FontExportFormat::AnsiDcs, ComExportFormat::NonTsr).unwrap();
        let text = std::str::from_utf8(&bytes).unwrap();
        let payload = text.strip_prefix("\x1BPCTerm:Font:0:").unwrap().strip_suffix("\x1B\\").unwrap();
        let decoded = base64::engine::general_purpose::STANDARD.decode(payload).unwrap();
        assert_eq!(decoded.len(), 256 * font.size().height as usize);
        assert_eq!(decoded, font.convert_to_u8_data());
    }

    #[test]
    fn com_exports_roundtrip_and_select_the_correct_modes() {
        for height in [1, 8, 16, 32] {
            let data: Vec<_> = (0..256 * height).map(|index| (index % 256) as u8).collect();
            let font = BitFont::create_8("COM", 8, height as u8, &data);
            for format in ComExportFormat::ALL {
                let bytes = encode(&font, FontExportFormat::Com, format).unwrap();
                let loaded = crate::font_import::parse_com_font("COM", &bytes).unwrap();
                assert_eq!(loaded.size(), font.size());
                assert_eq!(loaded.convert_to_u8_data(), data, "{format}");
                if format == ComExportFormat::NonTsr {
                    assert_eq!(bytes.len(), 0x19 + data.len());
                    assert_eq!(bytes[0x15], height as u8);
                } else {
                    assert_eq!(&bytes[0x28..0x2C], b"VILE");
                    assert_eq!(bytes[0x5D], height as u8);
                    let target = 3 + u16::from_le_bytes([bytes[1], bytes[2]]) as usize;
                    assert_eq!(target, 0x63 + data.len());
                    let modes = match format {
                        ComExportFormat::Tsr40Col => [1, 1, 0, 0],
                        ComExportFormat::Tsr80Col => [0, 0, 1, 1],
                        ComExportFormat::TsrAll => [1, 1, 1, 1],
                        ComExportFormat::NonTsr => unreachable!(),
                    };
                    assert_eq!(&bytes[0x2D..0x31], modes);
                }
            }
        }
    }

    #[test]
    fn empty_font_dimensions_are_explicit_errors() {
        for (width, height) in [(0, 16), (8, 0)] {
            let font = BitFont::create_8("Invalid", width, height, &[]);
            for format in FontExportFormat::ALL {
                assert!(encode(&font, format, ComExportFormat::NonTsr).is_err());
            }
        }
    }
}
