use base64::{Engine, engine::general_purpose};
use libyaff::{GlyphDefinition, YaffFont};

use crate::EngineResult;
use std::{collections::HashMap, error::Error, path::PathBuf, str::FromStr, sync::Mutex};

use super::Size;

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum BitFontType {
    BuiltIn,
    Library,
    Custom,
}

#[derive(Debug)]
pub struct BitFont {
    pub yaff_font: YaffFont,
    glyph_cache: Mutex<HashMap<char, GlyphDefinition>>,
    pub path_opt: Option<PathBuf>,
    font_type: BitFontType,
}

impl PartialEq for BitFont {
    fn eq(&self, other: &Self) -> bool {
        self.yaff_font == other.yaff_font
    }
}

impl Clone for BitFont {
    fn clone(&self) -> Self {
        Self {
            yaff_font: self.yaff_font.clone(),
            glyph_cache: Mutex::new(self.glyph_cache.lock().unwrap().clone()),
            path_opt: self.path_opt.clone(),
            font_type: self.font_type,
        }
    }
}

impl Default for BitFont {
    fn default() -> Self {
        BitFont::from_ansi_font_page(0).unwrap()
    }
}

impl BitFont {
    pub fn name(&self) -> &str {
        self.yaff_font.name.as_deref().unwrap_or("")
    }

    pub fn size(&self) -> Size {
        let mut width: i32 = 8;
        let mut height: i32 = 16;
        if let Some(h) = self.yaff_font.line_height {
            height = h;
        } else if let Some((_x, y)) = self.yaff_font.bounding_box {
            height = y as i32;
        }

        if let Some(cs) = self.yaff_font.bounding_box {
            width = cs.0 as i32;
        } else if let Some((x, _y)) = self.yaff_font.cell_size {
            width = x as i32;
        }

        Size::new(width as i32, height as i32)
    }

    pub fn font_type(&self) -> BitFontType {
        self.font_type
    }

    pub fn is_default(&self) -> bool {
        self.name() == DEFAULT_FONT_NAME
    }

    /// Get a glyph for the given character, using cache for performance
    pub fn get_glyph(&self, ch: char) -> Option<GlyphDefinition> {
        // Check cache first
        {
            let cache = self.glyph_cache.lock().unwrap();
            if let Some(glyph_def) = cache.get(&ch) {
                return Some(glyph_def.clone());
            }
        }

        // Find and cache the glyph
        if let Some(glyph_def) = self.find_glyph_in_font(ch) {
            self.glyph_cache.lock().unwrap().insert(ch, glyph_def.clone());
            return Some(glyph_def);
        }

        None
    }

    /// Find a glyph definition for the given character
    fn find_glyph_in_font(&self, ch: char) -> Option<GlyphDefinition> {
        use libyaff::Label;

        eprintln!("DEBUG: find_glyph_in_font for '{}' (U+{:04X}, byte {})", ch, ch as u32, ch as u8);
        eprintln!("  Font has {} glyphs", self.yaff_font.glyphs.len());

        let result = self
            .yaff_font
            .glyphs
            .iter()
            .find(|g| {
                let matches = g.labels.iter().any(|label| match label {
                    Label::Codepoint(codes) => {
                        let match_found = codes.contains(&(ch as u16));
                        if match_found {
                            eprintln!("  Found via Codepoint label: {:?}", codes);
                        }
                        match_found
                    }
                    Label::Unicode(codes) => {
                        let match_found = codes.contains(&(ch as u32));
                        if match_found {
                            eprintln!("  Found via Unicode label: {:?}", codes);
                        }
                        match_found
                    }
                    _ => false,
                });
                matches
            })
            .cloned();

        if result.is_none() {
            eprintln!("  NO MATCH FOUND!");
            // Debug: print first few glyphs to see structure
            for (i, g) in self.yaff_font.glyphs.iter().take(3).enumerate() {
                eprintln!("  Glyph {}: labels={:?}", i, g.labels);
            }
        } else {
            eprintln!("  Match found!");
        }

        result
    }

    /// Convert font to raw u8 data for legacy formats
    pub fn convert_to_u8_data(&self) -> Vec<u8> {
        let mut result = Vec::new();
        let size = self.size();
        let length = 256; // Standard ASCII range

        for ch_code in 0..length {
            let ch = unsafe { char::from_u32_unchecked(ch_code as u32) };
            if let Some(glyph_def) = self.find_glyph_in_font(ch) {
                // Convert bitmap to u8 rows
                let mut rows = Vec::new();
                let height = glyph_def.bitmap.height;
                let width = glyph_def.bitmap.width;

                for y in 0..height {
                    let mut packed: u8 = 0;
                    if y < glyph_def.bitmap.pixels.len() {
                        let row = &glyph_def.bitmap.pixels[y];
                        for x in 0..width.min(8) {
                            if x < row.len() && row[x] {
                                packed |= 1 << (7 - x);
                            }
                        }
                    }
                    rows.push(packed);
                }

                // Normalize to font height
                let target = size.height as usize;
                if rows.len() > target {
                    rows.truncate(target);
                } else if rows.len() < target {
                    rows.resize(target, 0);
                }
                result.extend_from_slice(&rows);
            } else {
                // No glyph found, add empty rows
                result.extend_from_slice(vec![0; size.height as usize].as_slice());
            }
        }
        result
    }

    pub fn encode_as_ansi(&self, font_slot: usize) -> String {
        let font_data = self.convert_to_u8_data();
        let data = general_purpose::STANDARD.encode(font_data);
        format!("\x1BPCTerm:Font:{font_slot}:{data}\x1B\\")
    }

    /// Create a font from raw 8-bit data
    pub fn create_8(name: impl Into<String>, width: u8, height: u8, data: &[u8]) -> Self {
        let mut yaff_font = YaffFont::from_raw_bytes(data, width as u32, height as u32).unwrap();
        yaff_font.name = Some(name.into());
        Self {
            path_opt: None,
            font_type: BitFontType::Custom,
            yaff_font,
            glyph_cache: Mutex::new(HashMap::new()),
        }
    }

    /// Alias for create_8 for compatibility
    pub fn from_basic(width: u8, height: u8, data: &[u8]) -> Self {
        Self::create_8("Custom", width, height, data)
    }

    /// Length field for compatibility (always 256 for standard fonts)
    pub fn length(&self) -> usize {
        256
    }

    /// Convert to PSF2 bytes format
    pub fn to_psf2_bytes(&self) -> EngineResult<Vec<u8>> {
        // Use libyaff to convert to PSF2 format
        Ok(libyaff::psf::to_psf2_bytes(&self.yaff_font)?)
    }
}

impl BitFont {
    /// Load font from bytes (PSF1, PSF2, or plain format)
    pub fn from_bytes(name: impl Into<String>, data: &[u8]) -> EngineResult<Self> {
        // Try to parse as YaffFont first (handles PSF1, PSF2)
        match YaffFont::from_bytes(data) {
            Ok(mut yaff_font) => {
                yaff_font.name = Some(name.into());
                Ok(Self {
                    path_opt: None,
                    font_type: BitFontType::BuiltIn,
                    yaff_font,
                    glyph_cache: Mutex::new(HashMap::new()),
                })
            }
            Err(_) => {
                // Try as raw font data
                if data.len() % 256 != 0 {
                    return Err(FontError::UnknownFontFormat(data.len()).into());
                }
                let char_height = data.len() / 256;
                Ok(Self::create_8(name, 8, char_height as u8, data))
            }
        }
    }
}

impl FromStr for BitFont {
    type Err = anyhow::Error;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        // Try to load from file path or font name
        BitFont::from_sauce_name(s)
    }
}

macro_rules! fonts {
    ($( ($i:ident, $file:expr, $name: expr, $width:expr, $height:expr $(, $font_slot:expr)? ) ),* $(,)? ) => {

        $(
            pub const $i: &[u8] = include_bytes!(concat!("../data/fonts/", $file));
        )*

        impl BitFont {
            /// .
            ///
            /// # Panics
            ///
            /// Panics if .
            ///
            /// # Errors
            ///
            /// This function will return an error if .
            pub fn from_ansi_font_page(font_page: usize) -> EngineResult<Self> {
                match font_page {
                    $(
                        $( $font_slot => {BitFont::from_bytes($name, $i)}  )?
                    )*
                    _ => Err(ParserError::UnsupportedFont(font_page).into()),
                }
            }
        }

        pub const FONT_NAMES: &[&str] = &[
            $(
                $name,
            )*
        ];
    };
}

const DEFAULT_FONT_NAME: &str = "Codepage 437 English";

lazy_static::lazy_static! {
    pub static ref ATARI_XEP80: BitFont = BitFont::from_bytes("Atari XEP80", include_bytes!("../data/fonts/Atari/xep80.psf")).unwrap();
    pub static ref ATARI_XEP80_INT: BitFont = BitFont::from_bytes("Atari XEP80 INT", include_bytes!("../data/fonts/Atari/xep80_int.psf")).unwrap();

    pub static ref ATARI_ST_FONT_8x8: BitFont = BitFont::from_bytes("Atari ST 8x8", include_bytes!("../data/fonts/Atari/atari-st-8x8.yaff")).unwrap();

    pub static ref EGA_7x8: BitFont = BitFont::from_bytes("EGA 7x8", include_bytes!("../data/fonts/Rip/Bm437_EverexME_7x8.yaff")).unwrap();
    pub static ref VGA_8x14: BitFont = BitFont::from_bytes("VGA 8x14", include_bytes!("../data/fonts/Rip/IBM_VGA_8x14.yaff")).unwrap();
    pub static ref VGA_7x14: BitFont = {
        // Derived from VGA_8x14 by horizontally doubling each pixel (8x14 -> 16x14)
        let mut new_font = VGA_8x14.yaff_font.clone();
        new_font.name = Some("VGA 7x14".to_string());
        new_font.bounding_box = Some((7, 14));
        new_font.cell_size = Some((7, 14));
        new_font.line_height = Some(14);

        BitFont {
            yaff_font: new_font,
            glyph_cache: Mutex::new(HashMap::new()),
            path_opt: None,
            font_type: BitFontType::BuiltIn,
        }
    };
    pub static ref VGA_16x14: BitFont = {
        // Derived from VGA_8x14 by horizontally doubling each pixel (8x14 -> 16x14)
        let mut new_font = VGA_8x14.yaff_font.clone();
        new_font.name = Some("VGA 16x14".to_string());
        new_font.bounding_box = Some((16, 14));
        new_font.cell_size = Some((16, 14));
        new_font.line_height = Some(14);
        for glyph in new_font.glyphs.iter_mut() {
            let old_pixels = glyph.bitmap.pixels.clone();
            let new_pixels: Vec<Vec<bool>> = old_pixels
                .into_iter()
                .map(|row| {
                    let mut new_row = Vec::with_capacity(row.len() * 2);
                    for px in row {
                        new_row.push(px);
                        new_row.push(px); // duplicate horizontally
                    }
                    new_row
                })
                .collect();
            glyph.bitmap.width = 16; // update width
            glyph.bitmap.pixels = new_pixels;
        }

        BitFont {
            yaff_font: new_font,
            glyph_cache: Mutex::new(HashMap::new()),
            path_opt: None,
            font_type: BitFontType::BuiltIn,
        }
    };


}

pub const ANSI_FONTS: usize = 42;

fonts![
    (CP437, "Ansi/cp437_8x16.psf", DEFAULT_FONT_NAME, 8, 16, 0),
    (CP1251, "Ansi/cp1251_swiss.f16", "Codepage 1251 Cyrillic, (swiss)", 8, 16, 1),
    (KOI8_R, "Ansi/KOI8-R.F16", "Russian koi8-r", 8, 16, 2),
    (ISO8859, "Ansi/ISO-8859-2_Central_European_8x16.f16", "ISO-8859-2 Central European", 8, 16, 3),
    (
        ISO8859_BALTIC_9BIT,
        "Ansi/ISO-8859-4_Baltic_wide_VGA_9bit_mapped_8x16.f16",
        "ISO-8859-4 Baltic wide (VGA 9bit mapped)",
        8,
        16,
        4
    ),
    (CP866, "Ansi/cp866_russian.psf", "Codepage 866 (c) Russian", 8, 16, 5),
    (CP8859_T, "Ansi/ISO-8859-9_Turkish_8x16.f16", "ISO-8859-9 Turkish", 8, 16, 6),
    (HAIK8, "Ansi/HAIK8.F16", "haik8 codepage", 8, 16, 7),
    (ISO8859_HEB, "Ansi/ISO-8859-8_Hebrew_8x16.f16", "ISO-8859-8 Hebrew", 8, 16, 8),
    (KOI8_U, "Ansi/Ukrainian_font_koi8-u_8x16.f16", "Ukrainian font koi8-u", 8, 16, 9),
    (
        ISO8859_WE,
        "Ansi/ISO-8859-15_West_European_thin_8x16.f16",
        "ISO-8859-15 West European, (thin)",
        8,
        16,
        10
    ),
    (
        ISO8859_4_BALTIC,
        "Ansi/ISO-8859-4_Baltic_VGA_9bit_mapped_8x16.f16",
        "ISO-8859-4 Baltic (VGA 9bit mapped)",
        8,
        16,
        11
    ),
    (KOI8_R_B, "Ansi/Russian_koi8-r_b_8x16.f16", "Russian koi8-r (b)", 8, 16, 12),
    (ISO8859_BW, "Ansi/ISO-8859-4_Baltic_wide_8x16.f16", "ISO-8859-4 Baltic wide", 8, 16, 13),
    (ISO8859_5, "Ansi/ISO-8859-5_Cyrillic_8x16.f16", "ISO-8859-5 Cyrillic", 8, 16, 14),
    (ARMSCII_8, "Ansi/ARMSCII-8_Character_set_8x16.f16", "ARMSCII-8 Character set", 8, 16, 15),
    (ISO8859_15, "Ansi/ISO-8859-15_West_European_8x16.f16", "ISO-8859-15 West European", 8, 16, 16),
    (
        CP850_LI,
        "Ansi/Codepage_850_Multilingual_Latin_I_thin_8x16.f16",
        "Codepage 850 Multilingual Latin I, (thin)",
        8,
        16,
        17
    ),
    (
        CP850_ML,
        "Ansi/Codepage_850_Multilingual_Latin_I_8x16.f16",
        "Codepage 850 Multilingual Latin I",
        8,
        16,
        18
    ),
    (CP865, "Ansi/Codepage_865_Norwegian_thin_8x16.f16", "Codepage 865 Norwegian, (thin)", 8, 16, 19),
    (CP1251_CYR, "Ansi/Codepage_1251_Cyrillic_8x16.f16", "Codepage 1251 Cyrillic", 8, 16, 20),
    (ISO8859_7, "Ansi/ISO-8859-7_Greek_8x16.f16", "ISO-8859-7 Greek", 8, 16, 21),
    (KOI8_RC, "Ansi/Russian_koi8-r_c_8x16.f16", "Russian koi8-r (c)", 8, 16, 22),
    (ISO8859_4_BALTIC2, "Ansi/ISO-8859-4_Baltic_8x16.f16", "ISO-8859-4 Baltic", 8, 16, 23),
    (ISO8859_1_WE, "Ansi/ISO-8859-1_West_European_8x16.f16", "ISO-8859-1 West European", 8, 16, 24),
    (CP886_RUS, "Ansi/Codepage_866_Russian_8x16.f16", "Codepage 866 Russian", 8, 16, 25),
    (CP437_THIN, "Ansi/Codepage_437_English_thin_8x16.f16", "Codepage 437 English, (thin)", 8, 16, 26),
    (CP866_R, "Ansi/Codepage_866_b_Russian_8x16.f16", "Codepage 866 (b) Russian", 8, 16, 27),
    (CP865_NOR, "Ansi/Codepage_865_Norwegian_8x16.f16", "Codepage 865 Norwegian", 8, 16, 28),
    (CP866U, "Ansi/Ukrainian_font_cp866u_8x16.f16", "Ukrainian font cp866u", 8, 16, 29),
    (
        ISO8859_1_WE_T,
        "Ansi/ISO-8859-1_West_European_thin_8x16.f16",
        "ISO-8859-1 West European, (thin)",
        8,
        16,
        30
    ),
    (
        CP1131_BEL,
        "Ansi/Codepage_1131_Belarusian_swiss_8x16.f16",
        "Codepage 1131 Belarusian, (swiss)",
        8,
        16,
        31
    ),
    (C64_UNSHIFTED, "Commodore/C64_PETSCII_unshifted.psf", "Commodore 64 (Unshifted)", 8, 8, 32),
    (C64_SHIFTED, "Commodore/C64_PETSCII_shifted.psf", "Commodore 64 (Shifted)", 8, 8, 33),
    (C128_UPPER, "Commodore/Commodore_128_UPPER_8x16.f16", "Commodore 128 (UPPER)", 8, 8, 34),
    (C128_LOWER, "Commodore/Commodore_128_Lower_8x16.f16", "Commodore 128 (Lower)", 8, 8, 35),
    (ATARI, "Atari/Atari_ATASCII.psf", "Atari", 8, 8, 36),
    (AMIGA_P0T_NOODLE, "Amiga/P0T-NOoDLE.psf", "P0T NOoDLE (Amiga)", 8, 16, 37),
    (AMIGA_MOSOUL, "Amiga/mOsOul.psf", "mO'sOul (Amiga)", 8, 16, 38),
    (AMIGA_MICROKNIGHTP, "Amiga/MicroKnight+.psf", "MicroKnight Plus (Amiga)", 8, 16, 39),
    (AMIGA_TOPAZ_2P, "Amiga/Topaz2+.psf", "Topaz Plus (Amiga)", 8, 16, 40),
    (AMIGA_MICROKNIGHT, "Amiga/MicroKnight.psf", "MicroKnight (Amiga)", 8, 16, 41),
    (AMIGA_TOPAZ_2, "Amiga/Topaz2.psf", "Topaz (Amiga)", 8, 16, 42),
    (VIEWDATA, "Viewdata/saa5050.psf", "Viewdata", 6, 16),
];

macro_rules! sauce_fonts {
    ($( ($i:ident, $file:expr, $name: expr, $stretch:expr, $stretch_lga:expr) ),* $(,)? ) => {

        $(
            pub const $i: &[u8] = include_bytes!(concat!("../data/fonts/", $file));
        )*

        impl BitFont {
            /// .
            ///
            /// # Panics
            ///
            /// Panics if .
            ///
            /// # Errors
            ///
            /// This function will return an error if .
            pub fn from_sauce_name(sauce_name: &str) -> EngineResult<Self> {
                match sauce_name {
                    $(
                        $name => {BitFont::from_bytes($name, $i)}
                    )*
                    _ => Err(ParserError::UnsupportedSauceFont(sauce_name.to_string()).into()),
                }
            }
        }

        pub const SAUCE_FONT_NAMES: &[&str] = &[
            $(
                $name,
            )*
        ];
    };
}
sauce_fonts![
    // CP 437
    (IBM_VGA_SAUCE, "Ansi/cp437_8x16.psf", "IBM VGA", 1.35, 1.20),
    (IBM_VGA50_SAUCE, "Sauce/cp437/IBM_VGA50.psf", "IBM VGA50", 1.35, 1.20),
    (IBM_VGA25G_SAUCE, "Sauce/cp437/IBM_VGA25G.psf", "IBM VGA25G", 0, 0),
    (IBM_EGA_SAUCE, "Sauce/cp437/IBM_EGA.psf", "IBM EGA", 1.3714, 0),
    (IBM_EGA43_SAUCE, "Sauce/cp437/IBM_EGA43.F08", "IBM EGA43", 1.3714, 0),
    // Amiga
    (AMIGA_TOPAZ_1_SAUCE, "Amiga/Topaz1.psf", "Amiga Topaz 1", 1.4, 0.0),
    (AMIGA_TOPAZ_1P_SAUCE, "Amiga/Topaz1+.psf", "Amiga Topaz 1+", 1.4, 0.0),
    (AMIGA_TOPAZ_2_SAUCE, "Amiga/Topaz2.psf", "Amiga Topaz 2", 1.4, 0.0),
    (AMIGA_TOPAZ_2P_SAUCE, "Amiga/Topaz2+.psf", "Amiga Topaz 2+", 1.4, 0.0),
    (AMIGA_P0T_NOODLE_SAUCE, "Amiga/P0T-NOoDLE.psf", "Amiga P0T-NOoDLE", 1.4, 0.0),
    (AMIGA_MICROKNIGHT_SAUCE, "Amiga/MicroKnight.psf", "Amiga MicroKnight", 1.4, 0.0),
    (AMIGA_MICROKNIGHT_PLUS_SAUCE, "Amiga/MicroKnight+.psf", "Amiga MicroKnight+", 1.4, 0.0),
    (AMIGA_MOSOUL_SAUCE, "Amiga/mOsOul.psf", "Amiga mOsOul", 1.4, 0.0),
    // C64
    (C64_UNSHIFTED_SAUCE, "Commodore/C64_PETSCII_unshifted.psf", "C64 PETSCII unshifted", 1.2, 0.0),
    (C64_SHIFTED_SAUCE, "Commodore/C64_PETSCII_shifted.psf", "C64 PETSCII shifted", 1.2, 0.0),
    // Atari
    (ARMSCII_8_SAUCE, "Ansi/ARMSCII-8_Character_set_8x16.f16", "Atari ATASCII", 1.2, 0.0),
];

macro_rules! amiga_fonts {
    ($( ($i:ident, $file:expr, $name: expr, $size:expr) ),* $(,)? ) => {
        $(
            pub const $i: &str = include_str!(concat!("../data/fonts/Amiga/original/", $file));
        )*

        pub fn load_amiga_fonts() -> Vec<(String, usize, &'static str)> {
            let mut fonts = Vec::new();
            $(
                fonts.push(($name.to_string(), $size, $i));
            )*
            fonts
        }
    }
}

amiga_fonts![
    (AMIGA_TOPAZ_08, "amiga-ks13-topaz-08.yaff", "Topaz.font", 8),
    (AMIGA_TOPAZ_09, "amiga-ks13-topaz-09.yaff", "Topaz.font", 9),
    (AMIGA_TOPAZ_11, "workbench-3.1/Topaz_8x11.yaff", "Topaz.font", 11),
    (AMIGA_DIAMOND_12, "workbench-3.1/Diamond_12.yaff", "Diamond.font", 12),
    (AMIGA_DIAMOND_20, "workbench-3.1/Diamond_20.yaff", "Diamond.font", 20),
    (AMIGA_EMERALD_20, "workbench-1.0/Emerald_20.yaff", "Emerald.font", 20),
    (AMIGA_PEARL_08, "pearl_08.yaff", "Pearl.font", 8),
    (AMIGA_GARNET_09, "workbench-3.1/Garnet_9.yaff", "Garnet.font", 9),
    (AMIGA_GARNET_16, "workbench-3.1/Garnet_16.yaff", "Garnet.font", 16),
    (AMIGA_HELVETICA_09, "workbench-3.1/Helvetica_9.yaff", "Helvetica.font", 9),
    (AMIGA_HELVETICA_11, "workbench-3.1/Helvetica_11.yaff", "Helvetica.font", 11),
    (AMIGA_HELVETICA_13, "workbench-3.1/Helvetica_13.yaff", "Helvetica.font", 13),
    (AMIGA_HELVETICA_15, "workbench-3.1/Helvetica_15.yaff", "Helvetica.font", 15),
    (AMIGA_HELVETICA_18, "workbench-3.1/Helvetica_18.yaff", "Helvetica.font", 18),
    (AMIGA_HELVETICA_24, "workbench-3.1/Helvetica_24.yaff", "Helvetica.font", 24),
    (AMIGA_OPAL_09, "workbench-3.1/Opal_9.yaff", "Opal.font", 9),
    (AMIGA_OPAL_12, "workbench-3.1/Opal_12.yaff", "Opal.font", 12),
    (AMIGA_RUBY_08, "workbench-3.1/Ruby_8.yaff", "Ruby.font", 8),
    (AMIGA_RUBY_12, "workbench-3.1/Ruby_12.yaff", "Ruby.font", 12),
    (AMIGA_RUBY_15, "workbench-3.1/Ruby_15.yaff", "Ruby.font", 15),
    (AMIGA_SAPPHIRE_14, "workbench-3.1/Sapphire_14.yaff", "Sapphire.font", 14),
    (AMIGA_SAPPHIRE_15, "workbench-1.0/Sapphire_15.yaff", "Sapphire.font", 15),
    (AMIGA_SAPPHIRE_18, "workbench-1.0/Sapphire_18.yaff", "Sapphire.font", 18),
    (AMIGA_SAPPHIRE_19, "workbench-3.1/Sapphire_19.yaff", "Sapphire.font", 19),
    (AMIGA_TIMES_11, "workbench-3.1/Times_11.yaff", "Times.font", 11),
    (AMIGA_TIMES_13, "workbench-3.1/Times_13.yaff", "Times.font", 13),
    (AMIGA_TIMES_15, "workbench-3.1/Times_15.yaff", "Times.font", 15),
    (AMIGA_TIMES_18, "workbench-3.1/Times_18.yaff", "Times.font", 18),
    (AMIGA_TIMES_24, "workbench-3.1/Times_24.yaff", "Times.font", 24),
    (AMIGA_COURIER_11, "workbench-3.1/Courier_11.yaff", "Courier.font", 11),
    (AMIGA_COURIER_13, "workbench-3.1/Courier_13.yaff", "Courier.font", 13),
    (AMIGA_COURIER_15, "workbench-3.1/Courier_15.yaff", "Courier.font", 15),
    (AMIGA_COURIER_18, "workbench-3.1/Courier_18.yaff", "Courier.font", 18),
    (AMIGA_COURIER_24, "workbench-3.1/Courier_24.yaff", "Courier.font", 24),
    (AMIGA_COURIER_30, "workbench-3.1/Courier_30.yaff", "Courier.font", 30),
    (AMIGA_COURIER_36, "workbench-3.1/Courier_36.yaff", "Courier.font", 36),
];

macro_rules! atari_fonts {
    ($( ($i:ident, $file:expr, $name: expr, $size:expr) ),* $(,)? ) => {
        $(
            pub const $i: &str = include_str!(concat!("../data/fonts/Atari/", $file));
        )*

        pub fn load_atari_fonts() -> Vec<(String, usize, &'static str)> {
            let mut fonts = Vec::new();
            $(
                fonts.push(($name.to_string(), $size, $i));
            )*
            fonts
        }
    }
}

atari_fonts![
    (ATARI_ST_6X6, "atari-st-6x6.yaff", "Atari ST 6x6", 6),
    (ATARI_ST_8X8, "atari-st-8x8.yaff", "Atari ST 8x8", 8),
    (ATARI_ST_8X16, "atari-st-8x16.yaff", "Atari ST 8x16", 16),
];

#[derive(Debug, Clone)]
pub enum FontError {
    FontNotFound,
    MagicNumberMismatch,
    UnsupportedVersion(u32),
    LengthMismatch(usize, usize),
    UnknownFontFormat(usize),
}

impl std::fmt::Display for FontError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            FontError::FontNotFound => write!(f, "font not found."),
            FontError::MagicNumberMismatch => write!(f, "not a valid .psf file."),
            FontError::UnsupportedVersion(ver) => write!(f, "version {ver} not supported"),
            FontError::LengthMismatch(actual, calculated) => {
                write!(f, "length should be {calculated} was {actual}")
            }
            FontError::UnknownFontFormat(size) => {
                let sizes = [8, 14, 16, 19];
                let list = sizes.iter().fold(String::new(), |a, &b| {
                    let empty = a.is_empty();
                    a + &format!("{}{} height ({} bytes)", if empty { "" } else { ", " }, b, &(b * 256))
                });

                write!(f, "Unknown binary font format {size} bytes not supported. Valid format heights are: {list}")
            }
        }
    }
}

impl Error for FontError {
    fn description(&self) -> &str {
        "use std::display"
    }

    fn source(&self) -> Option<&(dyn Error + 'static)> {
        None
    }

    fn cause(&self) -> Option<&dyn Error> {
        self.source()
    }
}

#[derive(Debug, Clone)]
pub enum ParserError {
    UnsupportedFont(usize),
    UnsupportedSauceFont(String),
}

impl std::fmt::Display for ParserError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ParserError::UnsupportedFont(code) => write!(f, "font {} not supported", *code),
            ParserError::UnsupportedSauceFont(name) => write!(f, "font {name} not supported"),
        }
    }
}

impl std::error::Error for ParserError {
    fn description(&self) -> &str {
        "use std::display"
    }

    fn source(&self) -> Option<&(dyn Error + 'static)> {
        None
    }

    fn cause(&self) -> Option<&dyn Error> {
        self.source()
    }
}
