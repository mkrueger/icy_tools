//! SAUCE Font Mapping
//!
//! Maps SAUCE font names to either ANSI slots or dedicated font files.
//! Implements fallback chain according to SAUCE specification.

use super::BitFont;
use super::ansi::get_ansi_font;

/// Source for a SAUCE font - either an ANSI slot or a dedicated file
pub enum SauceFontSource {
    /// Reference to an ANSI slot (uses slot's f16 variant by default)
    AnsiSlot(usize),
    /// Dedicated font data embedded in binary
    Dedicated(&'static [u8]),
}

/// SAUCE font mapping entry
pub struct SauceFontMapping {
    /// SAUCE font name (as specified in SAUCE TInfoS field)
    pub sauce_name: &'static str,
    /// Font source
    pub source: SauceFontSource,
}

// Dedicated SAUCE fonts (not available as ANSI slots)
const IBM_VGA50: &[u8] = include_bytes!("../../data/fonts/Sauce/cp437/IBM_VGA50.psf");
const IBM_VGA25G: &[u8] = include_bytes!("../../data/fonts/Sauce/cp437/IBM_VGA25G.psf");
const IBM_EGA: &[u8] = include_bytes!("../../data/fonts/Sauce/cp437/IBM_EGA.psf");
const IBM_EGA43: &[u8] = include_bytes!("../../data/fonts/Sauce/cp437/IBM_EGA43.F08");

// Amiga fonts from dedicated files
const AMIGA_TOPAZ_1: &[u8] = include_bytes!("../../data/fonts/Amiga/Topaz1.psf");
const AMIGA_TOPAZ_1_PLUS: &[u8] = include_bytes!("../../data/fonts/Amiga/Topaz1+.psf");
const AMIGA_TOPAZ_2: &[u8] = include_bytes!("../../data/fonts/Amiga/Topaz2.psf");
const AMIGA_TOPAZ_2_PLUS: &[u8] = include_bytes!("../../data/fonts/Amiga/Topaz2+.psf");
const AMIGA_P0T_NOODLE: &[u8] = include_bytes!("../../data/fonts/Amiga/P0T-NOoDLE.psf");
const AMIGA_MICROKNIGHT: &[u8] = include_bytes!("../../data/fonts/Amiga/MicroKnight.psf");
const AMIGA_MICROKNIGHT_PLUS: &[u8] = include_bytes!("../../data/fonts/Amiga/MicroKnight+.psf");
const AMIGA_MOSOUL: &[u8] = include_bytes!("../../data/fonts/Amiga/mOsOul.psf");

// C64 fonts
const C64_PETSCII_UNSHIFTED: &[u8] = include_bytes!("../../data/fonts/Commodore/C64_PETSCII_unshifted.psf");
const C64_PETSCII_SHIFTED: &[u8] = include_bytes!("../../data/fonts/Commodore/C64_PETSCII_shifted.psf");

// Atari fonts
const ATARI_ATASCII: &[u8] = include_bytes!("../../data/fonts/Atari/Atari_ATASCII.psf");

/// Complete SAUCE font mapping table
///
/// According to SAUCE spec, these are the standard font names.
/// See: https://www.acid.org/info/sauce/sauce.htm
pub static SAUCE_FONT_MAP: &[SauceFontMapping] = &[
    // ========================================
    // IBM PC VGA Fonts (CP437)
    // ========================================
    SauceFontMapping {
        sauce_name: "IBM VGA",
        source: SauceFontSource::AnsiSlot(0), // CP437 8x16
    },
    SauceFontMapping {
        sauce_name: "IBM VGA50",
        source: SauceFontSource::Dedicated(IBM_VGA50), // CP437 8x8 for 50-line mode
    },
    SauceFontMapping {
        sauce_name: "IBM VGA25G",
        source: SauceFontSource::Dedicated(IBM_VGA25G), // CP437 8x19 for graphics mode
    },
    SauceFontMapping {
        sauce_name: "IBM EGA",
        source: SauceFontSource::Dedicated(IBM_EGA), // CP437 8x14
    },
    SauceFontMapping {
        sauce_name: "IBM EGA43",
        source: SauceFontSource::Dedicated(IBM_EGA43), // CP437 8x8 for 43-line mode
    },
    // ========================================
    // IBM PC VGA Fonts with Codepages
    // ========================================
    // CP437 variants
    SauceFontMapping {
        sauce_name: "IBM VGA 437",
        source: SauceFontSource::AnsiSlot(0),
    },
    // CP850 - Multilingual Latin I
    SauceFontMapping {
        sauce_name: "IBM VGA 850",
        source: SauceFontSource::AnsiSlot(18),
    },
    // CP865 - Norwegian
    SauceFontMapping {
        sauce_name: "IBM VGA 865",
        source: SauceFontSource::AnsiSlot(28),
    },
    // CP866 - Russian
    SauceFontMapping {
        sauce_name: "IBM VGA 866",
        source: SauceFontSource::AnsiSlot(25),
    },
    // CP1251 - Cyrillic
    SauceFontMapping {
        sauce_name: "IBM VGA 1251",
        source: SauceFontSource::AnsiSlot(20),
    },
    // ========================================
    // Amiga Fonts
    // ========================================
    SauceFontMapping {
        sauce_name: "Amiga Topaz 1",
        source: SauceFontSource::Dedicated(AMIGA_TOPAZ_1),
    },
    SauceFontMapping {
        sauce_name: "Amiga Topaz 1+",
        source: SauceFontSource::Dedicated(AMIGA_TOPAZ_1_PLUS),
    },
    SauceFontMapping {
        sauce_name: "Amiga Topaz 2",
        source: SauceFontSource::Dedicated(AMIGA_TOPAZ_2),
    },
    SauceFontMapping {
        sauce_name: "Amiga Topaz 2+",
        source: SauceFontSource::Dedicated(AMIGA_TOPAZ_2_PLUS),
    },
    SauceFontMapping {
        sauce_name: "Amiga P0T-NOoDLE",
        source: SauceFontSource::Dedicated(AMIGA_P0T_NOODLE),
    },
    SauceFontMapping {
        sauce_name: "Amiga MicroKnight",
        source: SauceFontSource::Dedicated(AMIGA_MICROKNIGHT),
    },
    SauceFontMapping {
        sauce_name: "Amiga MicroKnight+",
        source: SauceFontSource::Dedicated(AMIGA_MICROKNIGHT_PLUS),
    },
    SauceFontMapping {
        sauce_name: "Amiga mOsOul",
        source: SauceFontSource::Dedicated(AMIGA_MOSOUL),
    },
    // ========================================
    // Commodore 64 Fonts
    // ========================================
    SauceFontMapping {
        sauce_name: "C64 PETSCII unshifted",
        source: SauceFontSource::Dedicated(C64_PETSCII_UNSHIFTED),
    },
    SauceFontMapping {
        sauce_name: "C64 PETSCII shifted",
        source: SauceFontSource::Dedicated(C64_PETSCII_SHIFTED),
    },
    // ========================================
    // Atari Fonts
    // ========================================
    SauceFontMapping {
        sauce_name: "Atari ATASCII",
        source: SauceFontSource::Dedicated(ATARI_ATASCII),
    },
];

/// Load a font by SAUCE name with fallback chain.
///
/// Fallback chain (according to SAUCE spec):
/// 1. Exact match
/// 2. For "IBM VGA ###" → try "IBM VGA"
/// 3. For "Amiga Font+" → try "Amiga Font"
/// 4. Default to "IBM VGA" (slot 0)
///
/// # Arguments
/// * `sauce_name` - The font name from SAUCE TInfoS field
///
/// # Returns
/// * `Ok(BitFont)` - The loaded font
/// * `Err` - If no suitable font could be found
pub fn load_sauce_font(sauce_name: &str) -> crate::Result<BitFont> {
    // Try exact match first
    if let Some(font) = try_load_sauce_font(sauce_name) {
        return Ok(font);
    }

    // Fallback 1: For "IBM VGA ###" or "IBM EGA ###", try base name
    if sauce_name.starts_with("IBM VGA ") || sauce_name.starts_with("IBM EGA ") {
        let base_name = if sauce_name.starts_with("IBM VGA") { "IBM VGA" } else { "IBM EGA" };
        if let Some(font) = try_load_sauce_font(base_name) {
            return Ok(font);
        }
    }

    // Fallback 2: For "Amiga Font+" → try "Amiga Font"
    if sauce_name.ends_with('+') {
        let base_name = &sauce_name[..sauce_name.len() - 1];
        if let Some(font) = try_load_sauce_font(base_name) {
            return Ok(font);
        }
    }

    // Fallback 3: Default to IBM VGA (slot 0)
    Ok(get_ansi_font(0, 16).cloned().unwrap())
}

/// Try to load a SAUCE font by exact name match
fn try_load_sauce_font(sauce_name: &str) -> Option<BitFont> {
    for mapping in SAUCE_FONT_MAP {
        if mapping.sauce_name.eq_ignore_ascii_case(sauce_name) {
            let mut font = match &mapping.source {
                SauceFontSource::AnsiSlot(slot) => {
                    // Load ANSI font but rename it to the SAUCE name
                    get_ansi_font(*slot, 16).cloned()
                }
                SauceFontSource::Dedicated(data) => BitFont::from_bytes(mapping.sauce_name, *data).ok(),
            };
            if let Some(font) = font.as_mut() {
                font.set_name(mapping.sauce_name);
            }

            return font;
        }
    }
    None
}

/// Get all available SAUCE font names
pub fn get_sauce_font_names() -> Vec<&'static str> {
    SAUCE_FONT_MAP.iter().map(|m| m.sauce_name).collect()
}

/*
/// All SAUCE font names as a static slice
pub static SAUCE_FONT_NAMES: &[&str] = &[
    "IBM VGA",
    "IBM VGA50",
    "IBM VGA25G",
    "IBM EGA",
    "IBM EGA43",
    "IBM VGA 437",
    "IBM VGA50 437",
    "IBM VGA25G 437",
    "IBM EGA 437",
    "IBM EGA43 437",
    "IBM VGA 720",
    "IBM VGA50 720",
    "IBM VGA25G 720",
    "IBM EGA 720",
    "IBM EGA43 720",
    "IBM VGA 737",
    "IBM VGA50 737",
    "IBM VGA25G 737",
    "IBM EGA 737",
    "IBM EGA43 737",
    "IBM VGA 775",
    "IBM VGA50 775",
    "IBM VGA25G 775",
    "IBM EGA 775",
    "IBM EGA43 775",
    "IBM VGA 819",
    "IBM VGA50 819",
    "IBM VGA25G 819",
    "IBM EGA 819",
    "IBM EGA43 819",
    "IBM VGA 850",
    "IBM VGA50 850",
    "IBM VGA25G 850",
    "IBM EGA 850",
    "IBM EGA43 850",
    "IBM VGA 852",
    "IBM VGA50 852",
    "IBM VGA25G 852",
    "IBM EGA 852",
    "IBM EGA43 852",
    "IBM VGA 855",
    "IBM VGA50 855",
    "IBM VGA25G 855",
    "IBM EGA 855",
    "IBM EGA43 855",
    "IBM VGA 857",
    "IBM VGA50 857",
    "IBM VGA25G 857",
    "IBM EGA 857",
    "IBM EGA43 857",
    "IBM VGA 858",
    "IBM VGA50 858",
    "IBM VGA25G 858",
    "IBM EGA 858",
    "IBM EGA43 858",
    "IBM VGA 860",
    "IBM VGA50 860",
    "IBM VGA25G 860",
    "IBM EGA 860",
    "IBM EGA43 860",
    "IBM VGA 861",
    "IBM VGA50 861",
    "IBM VGA25G 861",
    "IBM EGA 861",
    "IBM EGA43 861",
    "IBM VGA 862",
    "IBM VGA50 862",
    "IBM VGA25G 862",
    "IBM EGA 862",
    "IBM EGA43 862",
    "IBM VGA 863",
    "IBM VGA50 863",
    "IBM VGA25G 863",
    "IBM EGA 863",
    "IBM EGA43 863",
    "IBM VGA 864",
    "IBM VGA50 864",
    "IBM VGA25G 864",
    "IBM EGA 864",
    "IBM EGA43 864",
    "IBM VGA 865",
    "IBM VGA50 865",
    "IBM VGA25G 865",
    "IBM EGA 865",
    "IBM EGA43 865",
    "IBM VGA 866",
    "IBM VGA50 866",
    "IBM VGA25G 866",
    "IBM EGA 866",
    "IBM EGA43 866",
    "IBM VGA 869",
    "IBM VGA50 869",
    "IBM VGA25G 869",
    "IBM EGA 869",
    "IBM EGA43 869",
    "IBM VGA 872",
    "IBM VGA50 872",
    "IBM VGA25G 872",
    "IBM EGA 872",
    "IBM EGA43 872",
    "IBM VGA KAM",
    "IBM VGA50 KAM",
    "IBM VGA25G KAM",
    "IBM EGA KAM",
    "IBM EGA43 KAM",
    "IBM VGA MAZ",
    "IBM VGA50 MAZ",
    "IBM VGA25G MAZ",
    "IBM EGA MAZ",
    "IBM EGA43 MAZ",
    "IBM VGA MIK",
    "IBM VGA50 MIK",
    "IBM VGA25G MIK",
    "IBM EGA MIK",
    "IBM EGA43 MIK",
    "Amiga Topaz 1",
    "Amiga Topaz 1+",
    "Amiga Topaz 2",
    "Amiga Topaz 2+",
    "Amiga P0T-NOoDLE",
    "Amiga MicroKnight",
    "Amiga MicroKnight+",
    "Amiga mOsOul",
    "C64 PETSCII unshifted",
    "C64 PETSCII shifted",
    "Atari ATASCII",
];

/// Check if a SAUCE font name is supported
pub fn is_sauce_font_supported(sauce_name: &str) -> bool {
    // Check exact match
    if SAUCE_FONT_MAP.iter().any(|m| m.sauce_name.eq_ignore_ascii_case(sauce_name)) {
        return true;
    }

    // Check if it's a codepage variant that can fall back
    if sauce_name.starts_with("IBM VGA ") || sauce_name.starts_with("IBM EGA ") {
        return true;
    }

    // Check if it's an Amiga+ variant
    if sauce_name.ends_with('+') && sauce_name.starts_with("Amiga ") {
        let base_name = &sauce_name[..sauce_name.len() - 1];
        return SAUCE_FONT_MAP.iter().any(|m| m.sauce_name.eq_ignore_ascii_case(base_name));
    }

    false
}
*/
