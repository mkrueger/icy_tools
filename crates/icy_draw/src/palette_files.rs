//! Palette import and export, chosen by file extension.

use std::path::Path;

use icy_engine::{formats::PaletteFormat, FileFormat, Palette, SaveOptions, Screen, TextBuffer};

/// Extensions accepted by [`load`].
pub const IMPORT_EXTENSIONS: &[&str] = &["gpl", "pal", "hex", "txt", "ice", "icepal", "ase", "xb", "xbin"];

/// Save dialog filters for [`save`], the first one being the default.
pub const EXPORT_FILTERS: &[(&str, &[&str])] = &[
    ("GIMP Palette", &["gpl"]),
    ("PAL", &["pal"]),
    ("Hex", &["hex"]),
    ("Text", &["txt"]),
    ("ICE Palette", &["ice"]),
    ("Adobe Swatch Exchange", &["ase"]),
    ("XBin", &["xb", "xbin"]),
];

fn extension(path: &Path) -> Option<String> {
    path.extension().and_then(|extension| extension.to_str()).map(str::to_ascii_lowercase)
}

fn palette_format(extension: Option<&str>) -> Result<FileFormat, String> {
    let format = match extension {
        Some("pal") => PaletteFormat::Pal,
        Some("gpl") => PaletteFormat::Gpl,
        Some("hex") => PaletteFormat::Hex,
        Some("txt") => PaletteFormat::Txt,
        Some("ice" | "icepal") => PaletteFormat::Ice,
        Some("ase") => PaletteFormat::Ase,
        _ => return Err("Unsupported palette file type".to_string()),
    };
    Ok(FileFormat::Palette(format))
}

/// Loads a palette file, or the palette of an XBin file.
pub fn load(path: &Path) -> Result<Palette, String> {
    let extension = extension(path);
    if matches!(extension.as_deref(), Some("xb" | "xbin")) {
        let document = FileFormat::XBin.load(path, None).map_err(|error| error.to_string())?;
        return Ok(document.screen.palette().clone());
    }
    let format = palette_format(extension.as_deref())?;
    let bytes = std::fs::read(path).map_err(|error| error.to_string())?;
    format.load_palette(&bytes).map_err(|error| error.to_string())
}

/// Writes `palette` in the format given by the file extension.
pub fn save(palette: &Palette, path: &Path) -> Result<(), String> {
    let extension = extension(path);
    let bytes = if matches!(extension.as_deref(), Some("xb" | "xbin")) {
        let mut buffer = TextBuffer::new((1, 1));
        buffer.palette = palette.clone();
        let options = SaveOptions {
            format: icy_engine::FormatOptions::Compressed(icy_engine::CompressedFormatOptions { compress: false }),
            ..Default::default()
        };
        FileFormat::XBin.to_bytes(&buffer, &options)
    } else {
        palette.export_palette(&palette_format(extension.as_deref())?)
    }
    .map_err(|error| error.to_string())?;
    std::fs::write(path, bytes).map_err(|error| error.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn palette_files_round_trip() {
        let directory = std::env::temp_dir().join(format!("icy_draw_palette_files_{}", std::process::id()));
        std::fs::create_dir_all(&directory).unwrap();
        let mut palette = Palette::dos_default();
        // XBin stores 6-bit channels, so use values that survive that round trip.
        palette.set_color(3, icy_engine::Color::new(255, 85, 0));
        for extension in ["gpl", "pal", "hex", "ice", "ase", "xb"] {
            let path = directory.join(format!("palette.{extension}"));
            save(&palette, &path).unwrap();
            let loaded = load(&path).unwrap();
            assert_eq!(loaded.rgb(3), (255, 85, 0), "{extension}");
        }
        assert!(save(&palette, &directory.join("palette.unknown")).is_err());
        assert!(load(&directory.join("palette.unknown")).is_err());
        let _ = std::fs::remove_dir_all(directory);
    }
}
