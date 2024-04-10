use base64::{engine::general_purpose, Engine};

use crate::{update_crc32, EngineResult, ParserError};
use std::{
    collections::HashMap,
    error::Error,
    fmt::Display,
    fs::File,
    io::Read,
    path::{Path, PathBuf},
};

use super::Size;

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum BitFontType {
    BuiltIn,
    Library,
    Custom,
}

#[derive(Debug, Clone, PartialEq)]
pub struct Glyph {
    pub data: Vec<u8>,
}

impl Display for Glyph {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let mut s = String::new();
        for (y, b) in self.data.iter().enumerate() {
            s.push_str(&format!("{y:2}"));
            for i in 0..8 {
                if *b & (128 >> i) == 0 {
                    s.push('-');
                } else {
                    s.push('#');
                }
            }
            s.push('\n');
        }
        write!(f, "{s}---")
    }
}

impl Glyph {
    /// .
    ///
    /// # Panics
    ///
    /// Panics if .
    pub fn from_clipbard_data(data: &[u8]) -> (Size, Self) {
        let width = u16::from_le_bytes(data[0..2].try_into().unwrap());
        let height = u16::from_le_bytes(data[2..4].try_into().unwrap());
        let mut glyph = Glyph {
            data: vec![0; height as usize],
        };
        glyph.data = data[4..].to_vec();
        ((width, height).into(), glyph)
    }
}

#[derive(Debug, Clone, PartialEq)]
#[allow(dead_code)]
pub struct BitFont {
    pub name: String,
    pub path_opt: Option<PathBuf>,
    pub size: Size,
    pub length: i32,
    font_type: BitFontType,
    pub glyphs: HashMap<char, Glyph>,
    pub checksum: u32,
}

impl Default for BitFont {
    fn default() -> Self {
        BitFont::from_ansi_font_page(0).unwrap()
    }
}

impl BitFont {
    /// .
    ///
    /// # Panics
    ///
    /// Panics if .
    pub fn get_clipboard_data(&self, ch: char) -> Option<Vec<u8>> {
        let Some(glyph) = self.get_glyph(ch) else {
            return None;
        };

        let mut data = Vec::new();
        data.extend_from_slice(&u16::to_le_bytes(self.size.width as u16));
        data.extend_from_slice(&u16::to_le_bytes(self.size.height as u16));
        data.extend_from_slice(&glyph.data);
        Some(data)
    }

    pub fn get_checksum(&self) -> u32 {
        self.checksum
    }

    pub fn calculate_checksum(&mut self) {
        let mut crc = 0;
        for ch in 0..self.length {
            if let Some(glyph) = self.get_glyph(unsafe { char::from_u32_unchecked(ch as u32) }) {
                for b in &glyph.data {
                    crc = update_crc32(crc, *b);
                }
            }
        }
        self.checksum = crc;
    }

    pub fn font_type(&self) -> BitFontType {
        self.font_type
    }

    pub fn is_default(&self) -> bool {
        self.name == DEFAULT_FONT_NAME
    }

    pub fn convert_to_u8_data(&self) -> Vec<u8> {
        let mut result = Vec::new();
        for ch in 0..self.length {
            if let Some(glyph) = self.get_glyph(unsafe { char::from_u32_unchecked(ch as u32) }) {
                result.extend_from_slice(&glyph.data);
            } else {
                log::error!("Glyph not found for char: {}", ch);
                result.extend_from_slice(vec![0; self.size.height as usize].as_slice());
            }
        }
        result
    }

    pub fn get_glyph(&self, ch: char) -> Option<&Glyph> {
        self.glyphs.get(&ch)
    }

    pub fn get_glyph_mut(&mut self, ch: char) -> Option<&mut Glyph> {
        self.glyphs.get_mut(&ch)
    }

    pub fn create_8(name: impl Into<String>, width: u8, height: u8, data: &[u8]) -> Self {
        let mut res = Self {
            name: name.into(),
            path_opt: None,
            size: (width, height).into(),
            length: 256,
            font_type: BitFontType::Custom,
            glyphs: glyphs_from_u8_data(height as usize, data),
            checksum: 0,
        };
        res.calculate_checksum();
        res
    }

    pub fn from_basic(width: u8, height: u8, data: &[u8]) -> Self {
        let mut res = Self {
            name: String::new(),
            path_opt: None,
            size: (width, height).into(),
            length: 256,
            font_type: BitFontType::Custom,
            glyphs: glyphs_from_u8_data(height as usize, data),
            checksum: 0,
        };
        res.calculate_checksum();
        res
    }

    const PSF1_MAGIC: u16 = 0x0436;
    const PSF1_MODE512: u8 = 0x01;
    // const PSF1_MODEHASTAB: u8 = 0x02;
    // const PSF1_MODEHASSEQ: u8 = 0x04;
    // const PSF1_MAXMODE: u8 = 0x05;

    fn load_psf1(font_name: impl Into<String>, data: &[u8]) -> Self {
        let mode = data[2];
        let charsize = data[3];
        let length = if mode & BitFont::PSF1_MODE512 == BitFont::PSF1_MODE512 { 512 } else { 256 };

        let mut res = Self {
            name: font_name.into(),
            path_opt: None,
            size: (8, charsize).into(),
            length,
            font_type: BitFontType::BuiltIn,
            glyphs: glyphs_from_u8_data(charsize as usize, &data[4..]),
            checksum: 0,
        };
        res.calculate_checksum();
        res
    }

    fn load_plain_font(font_name: impl Into<String>, data: &[u8]) -> EngineResult<Self> {
        if data.len() % 256 != 0 {
            return Err(FontError::UnknownFontFormat(data.len()).into());
        }
        let char_height = data.len() / 256;
        let size = Size::new(8, char_height as i32);
        let mut res = Self {
            name: font_name.into(),
            path_opt: None,
            size,
            length: 256,
            font_type: BitFontType::BuiltIn,
            glyphs: glyphs_from_u8_data(char_height, data),
            checksum: 0,
        };
        res.calculate_checksum();
        Ok(res)
    }

    const PSF2_MAGIC: u32 = 0x864a_b572;
    // bits used in flags
    //const PSF2_HAS_UNICODE_TABLE: u8 = 0x01;
    // max version recognized so far
    const PSF2_MAXVERSION: u32 = 0x00;
    // UTF8 separators
    //const PSF2_SEPARATOR: u8 = 0xFF;
    //const PSF2_STARTSEQ: u8 = 0xFE;

    fn load_psf2(font_name: impl Into<String>, data: &[u8]) -> EngineResult<Self> {
        let version = u32::from_le_bytes(data[4..8].try_into().unwrap());
        if version > BitFont::PSF2_MAXVERSION {
            return Err(FontError::UnsupportedVersion(version).into());
        }
        let headersize = u32::from_le_bytes(data[8..12].try_into().unwrap()) as usize;
        // let flags = u32::from_le_bytes(data[12..16].try_into().unwrap());
        let length = u32::from_le_bytes(data[16..20].try_into().unwrap()) as i32;
        let charsize = u32::from_le_bytes(data[20..24].try_into().unwrap()) as i32;
        if length * charsize + headersize as i32 != data.len() as i32 {
            return Err(FontError::LengthMismatch(data.len(), (length * charsize) as usize + headersize).into());
        }
        let height = u32::from_le_bytes(data[24..28].try_into().unwrap()) as usize;
        let width = u32::from_le_bytes(data[28..32].try_into().unwrap()) as usize;

        let mut r = BitFont {
            name: font_name.into(),
            path_opt: None,
            size: (width, height).into(),
            length,
            font_type: BitFontType::BuiltIn,
            glyphs: glyphs_from_u8_data(height, &data[headersize..]),
            checksum: 0,
        };
        r.calculate_checksum();
        Ok(r)
    }

    /// .
    ///
    /// # Panics
    ///
    /// Panics if .
    ///
    /// # Errors
    ///
    /// This function will return an error if .
    pub fn to_psf2_bytes(&self) -> EngineResult<Vec<u8>> {
        let mut data = Vec::new();
        // Write PSF2 header.
        data.extend(u32::to_le_bytes(BitFont::PSF2_MAGIC)); // magic
        data.extend(u32::to_le_bytes(0)); // version
        data.extend(u32::to_le_bytes(8 * 4)); // headersize
        data.extend(u32::to_le_bytes(0)); // flags
        data.extend(u32::to_le_bytes(self.length as u32)); // length
        data.extend(u32::to_le_bytes(self.size.height as u32)); // charsize
        data.extend(u32::to_le_bytes(self.size.height as u32)); // height
        data.extend(u32::to_le_bytes(self.size.width as u32)); // width

        // glyphs
        for i in 0..self.length {
            data.extend(&self.get_glyph(unsafe { char::from_u32_unchecked(i as u32) }).unwrap().data);
        }

        Ok(data)
    }

    /// .
    ///
    /// # Panics
    ///
    /// Panics if .
    ///
    /// # Errors
    ///
    /// This function will return an error if .
    pub fn from_bytes(font_name: impl Into<String>, data: &[u8]) -> EngineResult<Self> {
        let magic16 = u16::from_le_bytes(data[0..2].try_into().unwrap());
        if magic16 == BitFont::PSF1_MAGIC {
            return Ok(BitFont::load_psf1(font_name, data));
        }

        let magic32 = u32::from_le_bytes(data[0..4].try_into().unwrap());
        if magic32 == BitFont::PSF2_MAGIC {
            return BitFont::load_psf2(font_name, data);
        }

        BitFont::load_plain_font(font_name, data)
    }

    /// .
    ///
    /// # Panics
    ///
    /// Panics if .
    ///
    /// # Errors
    ///
    /// This function will return an error if .
    pub fn load(file_name: &Path) -> EngineResult<Self> {
        let mut f = File::open(file_name).expect("error while opening file");
        let mut bytes = Vec::new();
        f.read_to_end(&mut bytes).expect("error while reading file");
        let mut font = BitFont::from_bytes(file_name.file_name().unwrap().to_string_lossy(), &bytes);
        if let Ok(ref mut font) = font {
            font.path_opt = Some(file_name.to_path_buf());
        }
        font
    }

    pub fn encode_as_ansi(&self, font_slot: usize) -> String {
        let font_data = self.convert_to_u8_data();
        let data = general_purpose::STANDARD.encode(font_data);
        format!("\x1BPCTerm:Font:{font_slot}:{data}\x1B\\")
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
fn glyphs_from_u8_data(font_height: usize, mut data: &[u8]) -> HashMap<char, Glyph> {
    let mut glyphs = HashMap::new();
    let mut ch = 0;
    while !data.is_empty() {
        let glyph = Glyph {
            data: data[..font_height].into(),
        };
        glyphs.insert(unsafe { char::from_u32_unchecked(ch as u32) }, glyph);

        data = &data[font_height..];
        ch += 1;
    }
    glyphs
}

const DEFAULT_FONT_NAME: &str = "Codepage 437 English";
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
    (C64_UPPER, "Commodore/C64_PETSCII_shifted.psf", "Commodore 64 (UPPER)", 8, 8, 32),
    (C64_LOWER, "Commodore/C64_PETSCII_unshifted.psf", "Commodore 64 (Lower)", 8, 8, 33),
    (C128_UPPER, "Commodore/Commodore_128_UPPER_8x16.f16", "Commodore 128 (UPPER)", 8, 8, 34),
    (C128_LOWER, "Commodore/Commodore_128_Lower_8x16.f16", "Commodore 128 (Lower)", 8, 8, 35),
    (ATARI, "Atari/Atari_ATASCII.psf", "Atari", 8, 8, 36),
    (AMIGA_P0T_NOODLE, "Amiga/P0T-NOoDLE.psf", "P0T NOoDLE (Amiga)", 8, 16, 37),
    (AMIGA_MOSOUL, "Amiga/mOsOul.psf", "mO'sOul (Amiga)", 8, 16, 38),
    (AMIGA_MICROKNIGHTP, "Amiga/MicroKnight+.psf", "MicroKnight Plus (Amiga)", 8, 16, 39),
    (AMIGA_TOPAZ_1P, "Amiga/Topaz1+.psf", "Topaz Plus (Amiga)", 8, 16, 40),
    (AMIGA_MICROKNIGHT, "Amiga/MicroKnight.psf", "MicroKnight (Amiga)", 8, 16, 41),
    (AMIGA_TOPAZ_1, "Amiga/Topaz1.psf", "Topaz (Amiga)", 8, 16, 42),
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
