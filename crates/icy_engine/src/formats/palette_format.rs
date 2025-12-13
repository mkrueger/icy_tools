use std::path::Path;

use crate::{Color, FileFormat, Palette};

use regex::Regex;

lazy_static::lazy_static! {
    static ref HEX_REGEX: Regex = Regex::new(r"([0-9a-fA-F]{2})([0-9a-fA-F]{2})([0-9a-fA-F]{2})").unwrap();

    static ref PAL_REGEX: Regex = Regex::new(r"(\d+)\s+(\d+)\s+(\d+)").unwrap();

    static ref GPL_COLOR_REGEX: Regex = Regex::new(r"(\d+)\s+(\d+)\s+(\d+)\s+(.+)").unwrap();
    static ref GPL_NAME_REGEX: Regex = Regex::new(r"\s*#Palette Name:\s*(.*)\s*").unwrap();
    static ref GPL_DESCRIPTION_REGEX: Regex = Regex::new(r"\s*#Description:\s*(.*)\s*").unwrap();

    static ref TXT_COLOR_REGEX: Regex = Regex::new(r"([0-9a-fA-F]{2})([0-9a-fA-F]{2})([0-9a-fA-F]{2})([0-9a-fA-F]{2})").unwrap();
    static ref TXT_NAME_REGEX: Regex = Regex::new(r"\s*;Palette Name:\s*(.*)\s*").unwrap();
    static ref TXT_DESCRIPTION_REGEX: Regex = Regex::new(r"\s*;Description:\s*(.*)\s*").unwrap();

    static ref ICE_COLOR_REGEX: Regex = Regex::new(r"([0-9a-fA-F]{2})([0-9a-fA-F]{2})([0-9a-fA-F]{2})").unwrap();
    static ref ICE_PALETTE_NAME_REGEX: Regex = Regex::new(r"\s*#Palette Name:\s*(.*)\s*").unwrap();
    static ref ICE_AUTHOR_REGEX: Regex = Regex::new(r"\s*#Author:\s*(.*)\s*").unwrap();
    static ref ICE_DESCRIPTION_REGEX: Regex = Regex::new(r"\s*#Description:\s*(.*)\s*").unwrap();
    static ref ICE_COLOR_NAME_REGEX: Regex = Regex::new(r"\s*#Name:\s*(.*)\s*").unwrap();
}

/// Supported palette file formats.
///
/// Note: Some palette formats may share file extensions with text formats (e.g. `.txt`).
/// Prefer detecting via `FileFormat::load_palette()` when loading palettes from disk.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum PaletteFormat {
    /// IcyDraw "ICE Palette" text format.
    Ice,
    /// Simple hex list, one `RRGGBB` per line.
    Hex,
    /// JASC-PAL
    Pal,
    /// GIMP palette (.gpl)
    Gpl,
    /// Paint.NET palette (.txt)
    Txt,
    /// Adobe Swatch Exchange (.ase)
    Ase,
}

impl PaletteFormat {
    pub fn name(self) -> &'static str {
        match self {
            PaletteFormat::Ice => "ICE Palette",
            PaletteFormat::Hex => "Hex",
            PaletteFormat::Pal => "JASC-PAL",
            PaletteFormat::Gpl => "GIMP Palette",
            PaletteFormat::Txt => "Paint.NET Palette",
            PaletteFormat::Ase => "ASE",
        }
    }

    /// Canonical extension (without dot) for saving when the format is explicitly chosen.
    pub fn extension(self) -> &'static str {
        match self {
            PaletteFormat::Ice => "ice",
            PaletteFormat::Hex => "hex",
            PaletteFormat::Pal => "pal",
            PaletteFormat::Gpl => "gpl",
            PaletteFormat::Txt => "txt",
            PaletteFormat::Ase => "ase",
        }
    }

    /// File extensions that can map to this palette format.
    pub fn all_extensions(self) -> &'static [&'static str] {
        match self {
            PaletteFormat::Ice => &["ice"],
            PaletteFormat::Hex => &["hex"],
            PaletteFormat::Pal => &["pal"],
            PaletteFormat::Gpl => &["gpl"],
            PaletteFormat::Txt => &["txt"],
            PaletteFormat::Ase => &["ase"],
        }
    }

    pub fn from_extension(ext: &str) -> Option<Self> {
        let ext = ext.to_ascii_lowercase();
        match ext.as_str() {
            "ice" => Some(PaletteFormat::Ice),
            "hex" => Some(PaletteFormat::Hex),
            "pal" => Some(PaletteFormat::Pal),
            "gpl" => Some(PaletteFormat::Gpl),
            "txt" => Some(PaletteFormat::Txt),
            "ase" => Some(PaletteFormat::Ase),
            _ => None,
        }
    }

    pub fn from_path(path: &Path) -> Option<Self> {
        path.extension().and_then(|e| e.to_str()).and_then(Self::from_extension)
    }
}

impl FileFormat {
    /// .
    ///
    /// # Panics
    ///
    /// Panics if .
    ///
    /// # Errors
    ///
    /// This function will return an error if .
    pub fn load_palette(&self, bytes: &[u8]) -> crate::Result<Palette> {
        let mut colors = Vec::new();
        let mut title = String::new();
        let mut author = String::new();
        let mut description = String::new();
        match self {
            FileFormat::Palette(PaletteFormat::Hex) => match String::from_utf8(bytes.to_vec()) {
                Ok(data) => {
                    for (_, [r, g, b]) in HEX_REGEX.captures_iter(&data).map(|c| c.extract()) {
                        let r = u32::from_str_radix(r, 16)?;
                        let g = u32::from_str_radix(g, 16)?;
                        let b = u32::from_str_radix(b, 16)?;
                        colors.push(Color::new(r as u8, g as u8, b as u8));
                    }
                }
                Err(err) => return Err(crate::EngineError::InvalidPaletteFormat { message: err.to_string() }),
            },
            FileFormat::Palette(PaletteFormat::Pal) => {
                match String::from_utf8(bytes.to_vec()) {
                    Ok(data) => {
                        for (i, line) in data.lines().enumerate() {
                            match i {
                                0 => {
                                    if line != "JASC-PAL" {
                                        return Err(crate::EngineError::UnsupportedPaletteFormat {
                                            expected: "JASC-PAL".to_string(),
                                        });
                                    }
                                }
                                1 | 2 => {
                                    // Ignore
                                }
                                _ => {
                                    for (_, [r, g, b]) in PAL_REGEX.captures_iter(line).map(|c| c.extract()) {
                                        let r = r.parse::<u32>()?;
                                        let g = g.parse::<u32>()?;
                                        let b = b.parse::<u32>()?;
                                        colors.push(Color::new(r as u8, g as u8, b as u8));
                                    }
                                }
                            }
                        }
                    }
                    Err(err) => return Err(crate::EngineError::InvalidPaletteFormat { message: err.to_string() }),
                }
            }
            FileFormat::Palette(PaletteFormat::Gpl) => match String::from_utf8(bytes.to_vec()) {
                Ok(data) => {
                    for (i, line) in data.lines().enumerate() {
                        match i {
                            0 => {
                                if line != "GIMP Palette" {
                                    return Err(crate::EngineError::UnsupportedPaletteFormat {
                                        expected: "GIMP Palette".to_string(),
                                    });
                                }
                            }
                            _ => {
                                if line.starts_with('#') {
                                    if let Some(cap) = GPL_NAME_REGEX.captures(line) {
                                        if let Some(name) = cap.get(1) {
                                            title = name.as_str().to_string();
                                        }
                                    }
                                    if let Some(cap) = GPL_DESCRIPTION_REGEX.captures(line) {
                                        if let Some(name) = cap.get(1) {
                                            description = name.as_str().to_string();
                                        }
                                    }
                                } else if let Some(cap) = GPL_COLOR_REGEX.captures(line) {
                                    let (_, [r, g, b, descr]) = cap.extract();

                                    let r = r.parse::<u32>()?;
                                    let g = g.parse::<u32>()?;
                                    let b = b.parse::<u32>()?;
                                    let mut c = Color::new(r as u8, g as u8, b as u8);
                                    if !descr.is_empty() {
                                        c.name = Some(descr.to_string());
                                    }
                                    colors.push(c);
                                }
                            }
                        }
                    }
                }
                Err(err) => return Err(crate::EngineError::InvalidPaletteFormat { message: err.to_string() }),
            },
            FileFormat::Palette(PaletteFormat::Ice) => match String::from_utf8(bytes.to_vec()) {
                Ok(data) => {
                    let mut next_color_name = String::new();
                    for (i, line) in data.lines().enumerate() {
                        match i {
                            0 => {
                                if line != "ICE Palette" {
                                    return Err(crate::EngineError::UnsupportedPaletteFormat {
                                        expected: "ICE Palette".to_string(),
                                    });
                                }
                            }
                            _ => {
                                if line.starts_with('#') {
                                    if let Some(cap) = ICE_PALETTE_NAME_REGEX.captures(line) {
                                        if let Some(name) = cap.get(1) {
                                            title = name.as_str().to_string();
                                        }
                                    }
                                    if let Some(cap) = ICE_DESCRIPTION_REGEX.captures(line) {
                                        if let Some(name) = cap.get(1) {
                                            description = name.as_str().to_string();
                                        }
                                    }
                                    if let Some(cap) = ICE_AUTHOR_REGEX.captures(line) {
                                        if let Some(name) = cap.get(1) {
                                            author = name.as_str().to_string();
                                        }
                                    }
                                    if let Some(cap) = ICE_COLOR_NAME_REGEX.captures(line) {
                                        if let Some(name) = cap.get(1) {
                                            next_color_name = name.as_str().to_string();
                                        }
                                    }
                                } else if let Some(cap) = ICE_COLOR_REGEX.captures(line) {
                                    let (_, [r, g, b]) = cap.extract();
                                    let r = u32::from_str_radix(r, 16)?;
                                    let g = u32::from_str_radix(g, 16)?;
                                    let b = u32::from_str_radix(b, 16)?;
                                    let mut col = Color::new(r as u8, g as u8, b as u8);
                                    if !next_color_name.is_empty() {
                                        col.name = Some(next_color_name.clone());
                                        next_color_name.clear();
                                    }
                                    colors.push(col);
                                }
                            }
                        }
                    }
                }
                Err(err) => return Err(crate::EngineError::InvalidPaletteFormat { message: err.to_string() }),
            },

            FileFormat::Palette(PaletteFormat::Txt) => match String::from_utf8(bytes.to_vec()) {
                Ok(data) => {
                    for line in data.lines() {
                        if line.starts_with(';') {
                            if let Some(cap) = TXT_NAME_REGEX.captures(line) {
                                if let Some(name) = cap.get(1) {
                                    title = name.as_str().to_string();
                                }
                            }
                            if let Some(cap) = TXT_DESCRIPTION_REGEX.captures(line) {
                                if let Some(name) = cap.get(1) {
                                    description = name.as_str().to_string();
                                }
                            }
                        } else if let Some(cap) = TXT_COLOR_REGEX.captures(line) {
                            let (_, [_a, r, g, b]) = cap.extract();

                            let r = u32::from_str_radix(r, 16)?;
                            let g = u32::from_str_radix(g, 16)?;
                            let b = u32::from_str_radix(b, 16)?;
                            colors.push(Color::new(r as u8, g as u8, b as u8));
                        }
                    }
                }
                Err(err) => return Err(crate::EngineError::InvalidPaletteFormat { message: err.to_string() }),
            },
            FileFormat::Palette(PaletteFormat::Ase) => {
                return Err(crate::EngineError::UnsupportedPaletteFormat {
                    expected: "ASE palette loading is not implemented".to_string(),
                });
            }
            FileFormat::XBin => {
                todo!("Exporting XBin palettes is not implemented yet");
            }

            _ => {
                return Err(crate::EngineError::InvalidPaletteFormat {
                    message: "Not a palette format".to_string(),
                });
            }
        }
        Ok(Palette::from_data(title, description, author, colors))
    }
}
