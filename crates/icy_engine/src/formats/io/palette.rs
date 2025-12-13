use std::path::Path;

use crate::formats::PaletteFormat;
use crate::{EngineError, Result};

pub(crate) fn detect_palette_format_from_path_and_bytes(file_path: &Path, data: &[u8]) -> Result<PaletteFormat> {
    let ext = file_path.extension().and_then(|e| e.to_str()).map(|e| e.to_ascii_lowercase());

    match ext.as_deref() {
        Some("pal") => Ok(PaletteFormat::Pal),
        Some("gpl") => Ok(PaletteFormat::Gpl),
        Some("hex") => Ok(PaletteFormat::Hex),
        Some("ase") => Err(EngineError::UnsupportedPaletteFormat {
            expected: "ASE palette loading is not implemented".to_string(),
        }),
        Some("icepal") => Ok(PaletteFormat::Ice),
        Some("ice") => {
            // `.ice` is ambiguous (ANSI vs ICE palette). For palette loading we allow it
            // only if it looks like an ICE palette header.
            if data.starts_with(b"ICE Palette") {
                Ok(PaletteFormat::Ice)
            } else {
                Err(EngineError::UnsupportedPaletteFormat {
                    expected: "ICE Palette header (file starts with 'ICE Palette')".to_string(),
                })
            }
        }
        Some("txt") => {
            // `.txt` is ambiguous. Only accept paint.net palette signature here.
            if data.starts_with(b";paint.net Palette File") {
                Ok(PaletteFormat::Txt)
            } else {
                Err(EngineError::UnsupportedPaletteFormat {
                    expected: "paint.net palette file (starts with ';paint.net Palette File')".to_string(),
                })
            }
        }
        _ => Err(EngineError::UnsupportedPaletteFormat {
            expected: "pal, gpl, hex, txt (paint.net), ice (ICE Palette), icepal, or ase".to_string(),
        }),
    }
}

/*

pub(crate) fn load_palette(file_path: &Path) -> Result<crate::Palette> {
    let data = std::fs::read(file_path)?;
    let format = detect_palette_format_from_path_and_bytes(file_path, &data)?;
    palette_from_bytes(format, &data)
}

pub(crate) fn palette_from_bytes(format: PaletteFormat, bytes: &[u8]) -> Result<crate::Palette> {
    match format {
        PaletteFormat::Ase => Err(EngineError::UnsupportedPaletteFormat {
            expected: "ASE palette loading is not implemented".to_string(),
        }),
        _ => crate::Palette::load_palette(&format, bytes),
    }
}

pub(crate) fn palette_to_bytes(format: PaletteFormat, palette: &crate::Palette) -> Result<Vec<u8>> {
    match format {
        PaletteFormat::Ase => Err(EngineError::UnsupportedPaletteFormat {
            expected: "ASE palette saving is not implemented".to_string(),
        }),
        _ => Ok(palette.export_palette(&format)),
    }
}
*/
