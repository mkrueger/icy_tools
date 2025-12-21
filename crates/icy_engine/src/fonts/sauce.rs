//! SAUCE Font Mapping
//!
//! Maps SAUCE font names to dedicated font files.
//! Implements fallback chain according to SAUCE specification.
//!
//! Font data converted from Moebius (blocktronics/moebius) using convert_moebius_fonts tool.
//!
//! ## Missing Codepages (referenced in Moebius but no files available)
//! - CP720 (Arabic)
//! - CP819 (Latin-1/ISO-8859-1)
//! - CP858 (Western + Euro)
//! - CP867 (KAM/Kamenický)
//! - CP872 (Cyrillic + Euro)
//! - CP667 (MAZ/Mazovia)
//! - CP790, CP895, CP991 (unofficial)
//!
//! ## Missing Height Variants (use fallback to VGA/EGA)
//! - VGA25G (.F19) missing for: CP737, CP775, CP855, CP857, CP862, CP864, CP866, CP869

use super::BitFont;

/// Dedicated font data embedded in binary
pub struct SauceFontMapping {
    /// SAUCE font name (as specified in SAUCE TInfoS field)
    pub sauce_name: &'static str,
    /// Font data (PSF2 format)
    pub data: &'static [u8],
}

// ============================================================================
// IBM PC Fonts - CP437 (base, no suffix)
// ============================================================================
const IBM_VGA: &[u8] = include_bytes!("../../data/fonts/Sauce/ibm/IBM_VGA.psf");
const IBM_VGA50: &[u8] = include_bytes!("../../data/fonts/Sauce/ibm/IBM_VGA50.psf");
const IBM_VGA25G: &[u8] = include_bytes!("../../data/fonts/Sauce/ibm/IBM_VGA25G.psf");
const IBM_EGA: &[u8] = include_bytes!("../../data/fonts/Sauce/ibm/IBM_EGA.psf");
const IBM_EGA43: &[u8] = include_bytes!("../../data/fonts/Sauce/ibm/IBM_EGA43.psf");

// CP437 explicit
const IBM_VGA_437: &[u8] = include_bytes!("../../data/fonts/Sauce/ibm/IBM_VGA_437.psf");
const IBM_VGA50_437: &[u8] = include_bytes!("../../data/fonts/Sauce/ibm/IBM_VGA50_437.psf");
const IBM_VGA25G_437: &[u8] = include_bytes!("../../data/fonts/Sauce/ibm/IBM_VGA25G_437.psf");
const IBM_EGA_437: &[u8] = include_bytes!("../../data/fonts/Sauce/ibm/IBM_EGA_437.psf");
const IBM_EGA43_437: &[u8] = include_bytes!("../../data/fonts/Sauce/ibm/IBM_EGA43_437.psf");

// ============================================================================
// IBM PC Fonts - CP737 (Greek)
// ============================================================================
const IBM_VGA_737: &[u8] = include_bytes!("../../data/fonts/Sauce/ibm/IBM_VGA_737.psf");
const IBM_VGA50_737: &[u8] = include_bytes!("../../data/fonts/Sauce/ibm/IBM_VGA50_737.psf");
const IBM_EGA_737: &[u8] = include_bytes!("../../data/fonts/Sauce/ibm/IBM_EGA_737.psf");
const IBM_EGA43_737: &[u8] = include_bytes!("../../data/fonts/Sauce/ibm/IBM_EGA43_737.psf");

// ============================================================================
// IBM PC Fonts - CP775 (Baltic)
// ============================================================================
const IBM_VGA_775: &[u8] = include_bytes!("../../data/fonts/Sauce/ibm/IBM_VGA_775.psf");
const IBM_VGA50_775: &[u8] = include_bytes!("../../data/fonts/Sauce/ibm/IBM_VGA50_775.psf");
const IBM_EGA_775: &[u8] = include_bytes!("../../data/fonts/Sauce/ibm/IBM_EGA_775.psf");
const IBM_EGA43_775: &[u8] = include_bytes!("../../data/fonts/Sauce/ibm/IBM_EGA43_775.psf");

// ============================================================================
// IBM PC Fonts - CP850 (Western Europe)
// ============================================================================
const IBM_VGA_850: &[u8] = include_bytes!("../../data/fonts/Sauce/ibm/IBM_VGA_850.psf");
const IBM_VGA50_850: &[u8] = include_bytes!("../../data/fonts/Sauce/ibm/IBM_VGA50_850.psf");
const IBM_VGA25G_850: &[u8] = include_bytes!("../../data/fonts/Sauce/ibm/IBM_VGA25G_850.psf");
const IBM_EGA_850: &[u8] = include_bytes!("../../data/fonts/Sauce/ibm/IBM_EGA_850.psf");
const IBM_EGA43_850: &[u8] = include_bytes!("../../data/fonts/Sauce/ibm/IBM_EGA43_850.psf");

// ============================================================================
// IBM PC Fonts - CP851 (Greek)
// ============================================================================
const IBM_VGA_851: &[u8] = include_bytes!("../../data/fonts/Sauce/ibm/IBM_VGA_851.psf");
const IBM_VGA50_851: &[u8] = include_bytes!("../../data/fonts/Sauce/ibm/IBM_VGA50_851.psf");
const IBM_VGA25G_851: &[u8] = include_bytes!("../../data/fonts/Sauce/ibm/IBM_VGA25G_851.psf");
const IBM_EGA_851: &[u8] = include_bytes!("../../data/fonts/Sauce/ibm/IBM_EGA_851.psf");
const IBM_EGA43_851: &[u8] = include_bytes!("../../data/fonts/Sauce/ibm/IBM_EGA43_851.psf");

// ============================================================================
// IBM PC Fonts - CP852 (Central Europe)
// ============================================================================
const IBM_VGA_852: &[u8] = include_bytes!("../../data/fonts/Sauce/ibm/IBM_VGA_852.psf");
const IBM_VGA50_852: &[u8] = include_bytes!("../../data/fonts/Sauce/ibm/IBM_VGA50_852.psf");
const IBM_VGA25G_852: &[u8] = include_bytes!("../../data/fonts/Sauce/ibm/IBM_VGA25G_852.psf");
const IBM_EGA_852: &[u8] = include_bytes!("../../data/fonts/Sauce/ibm/IBM_EGA_852.psf");
const IBM_EGA43_852: &[u8] = include_bytes!("../../data/fonts/Sauce/ibm/IBM_EGA43_852.psf");

// ============================================================================
// IBM PC Fonts - CP853 (Multilingual)
// ============================================================================
const IBM_VGA_853: &[u8] = include_bytes!("../../data/fonts/Sauce/ibm/IBM_VGA_853.psf");
const IBM_VGA50_853: &[u8] = include_bytes!("../../data/fonts/Sauce/ibm/IBM_VGA50_853.psf");
const IBM_VGA25G_853: &[u8] = include_bytes!("../../data/fonts/Sauce/ibm/IBM_VGA25G_853.psf");
const IBM_EGA_853: &[u8] = include_bytes!("../../data/fonts/Sauce/ibm/IBM_EGA_853.psf");
const IBM_EGA43_853: &[u8] = include_bytes!("../../data/fonts/Sauce/ibm/IBM_EGA43_853.psf");

// ============================================================================
// IBM PC Fonts - CP855 (Cyrillic)
// ============================================================================
const IBM_VGA_855: &[u8] = include_bytes!("../../data/fonts/Sauce/ibm/IBM_VGA_855.psf");
const IBM_VGA50_855: &[u8] = include_bytes!("../../data/fonts/Sauce/ibm/IBM_VGA50_855.psf");
const IBM_EGA_855: &[u8] = include_bytes!("../../data/fonts/Sauce/ibm/IBM_EGA_855.psf");
const IBM_EGA43_855: &[u8] = include_bytes!("../../data/fonts/Sauce/ibm/IBM_EGA43_855.psf");

// ============================================================================
// IBM PC Fonts - CP857 (Turkish)
// ============================================================================
const IBM_VGA_857: &[u8] = include_bytes!("../../data/fonts/Sauce/ibm/IBM_VGA_857.psf");
const IBM_VGA50_857: &[u8] = include_bytes!("../../data/fonts/Sauce/ibm/IBM_VGA50_857.psf");
const IBM_EGA_857: &[u8] = include_bytes!("../../data/fonts/Sauce/ibm/IBM_EGA_857.psf");
const IBM_EGA43_857: &[u8] = include_bytes!("../../data/fonts/Sauce/ibm/IBM_EGA43_857.psf");

// ============================================================================
// IBM PC Fonts - CP860 (Portuguese)
// ============================================================================
const IBM_VGA_860: &[u8] = include_bytes!("../../data/fonts/Sauce/ibm/IBM_VGA_860.psf");
const IBM_VGA50_860: &[u8] = include_bytes!("../../data/fonts/Sauce/ibm/IBM_VGA50_860.psf");
const IBM_VGA25G_860: &[u8] = include_bytes!("../../data/fonts/Sauce/ibm/IBM_VGA25G_860.psf");
const IBM_EGA_860: &[u8] = include_bytes!("../../data/fonts/Sauce/ibm/IBM_EGA_860.psf");
const IBM_EGA43_860: &[u8] = include_bytes!("../../data/fonts/Sauce/ibm/IBM_EGA43_860.psf");

// ============================================================================
// IBM PC Fonts - CP861 (Icelandic)
// ============================================================================
const IBM_VGA_861: &[u8] = include_bytes!("../../data/fonts/Sauce/ibm/IBM_VGA_861.psf");
const IBM_VGA50_861: &[u8] = include_bytes!("../../data/fonts/Sauce/ibm/IBM_VGA50_861.psf");
const IBM_VGA25G_861: &[u8] = include_bytes!("../../data/fonts/Sauce/ibm/IBM_VGA25G_861.psf");
const IBM_EGA_861: &[u8] = include_bytes!("../../data/fonts/Sauce/ibm/IBM_EGA_861.psf");
const IBM_EGA43_861: &[u8] = include_bytes!("../../data/fonts/Sauce/ibm/IBM_EGA43_861.psf");

// ============================================================================
// IBM PC Fonts - CP862 (Hebrew)
// ============================================================================
const IBM_VGA_862: &[u8] = include_bytes!("../../data/fonts/Sauce/ibm/IBM_VGA_862.psf");
const IBM_VGA50_862: &[u8] = include_bytes!("../../data/fonts/Sauce/ibm/IBM_VGA50_862.psf");
const IBM_EGA_862: &[u8] = include_bytes!("../../data/fonts/Sauce/ibm/IBM_EGA_862.psf");
const IBM_EGA43_862: &[u8] = include_bytes!("../../data/fonts/Sauce/ibm/IBM_EGA43_862.psf");

// ============================================================================
// IBM PC Fonts - CP863 (French Canadian)
// ============================================================================
const IBM_VGA_863: &[u8] = include_bytes!("../../data/fonts/Sauce/ibm/IBM_VGA_863.psf");
const IBM_VGA50_863: &[u8] = include_bytes!("../../data/fonts/Sauce/ibm/IBM_VGA50_863.psf");
const IBM_VGA25G_863: &[u8] = include_bytes!("../../data/fonts/Sauce/ibm/IBM_VGA25G_863.psf");
const IBM_EGA_863: &[u8] = include_bytes!("../../data/fonts/Sauce/ibm/IBM_EGA_863.psf");
const IBM_EGA43_863: &[u8] = include_bytes!("../../data/fonts/Sauce/ibm/IBM_EGA43_863.psf");

// ============================================================================
// IBM PC Fonts - CP864 (Arabic)
// ============================================================================
const IBM_VGA_864: &[u8] = include_bytes!("../../data/fonts/Sauce/ibm/IBM_VGA_864.psf");
const IBM_VGA50_864: &[u8] = include_bytes!("../../data/fonts/Sauce/ibm/IBM_VGA50_864.psf");
const IBM_EGA_864: &[u8] = include_bytes!("../../data/fonts/Sauce/ibm/IBM_EGA_864.psf");
const IBM_EGA43_864: &[u8] = include_bytes!("../../data/fonts/Sauce/ibm/IBM_EGA43_864.psf");

// ============================================================================
// IBM PC Fonts - CP865 (Nordic)
// ============================================================================
const IBM_VGA_865: &[u8] = include_bytes!("../../data/fonts/Sauce/ibm/IBM_VGA_865.psf");
const IBM_VGA50_865: &[u8] = include_bytes!("../../data/fonts/Sauce/ibm/IBM_VGA50_865.psf");
const IBM_VGA25G_865: &[u8] = include_bytes!("../../data/fonts/Sauce/ibm/IBM_VGA25G_865.psf");
const IBM_EGA_865: &[u8] = include_bytes!("../../data/fonts/Sauce/ibm/IBM_EGA_865.psf");
const IBM_EGA43_865: &[u8] = include_bytes!("../../data/fonts/Sauce/ibm/IBM_EGA43_865.psf");

// ============================================================================
// IBM PC Fonts - CP866 (Cyrillic/Russian)
// ============================================================================
const IBM_VGA_866: &[u8] = include_bytes!("../../data/fonts/Sauce/ibm/IBM_VGA_866.psf");
const IBM_VGA50_866: &[u8] = include_bytes!("../../data/fonts/Sauce/ibm/IBM_VGA50_866.psf");
const IBM_EGA_866: &[u8] = include_bytes!("../../data/fonts/Sauce/ibm/IBM_EGA_866.psf");
const IBM_EGA43_866: &[u8] = include_bytes!("../../data/fonts/Sauce/ibm/IBM_EGA43_866.psf");

// ============================================================================
// IBM PC Fonts - CP869 (Greek 2)
// ============================================================================
const IBM_VGA_869: &[u8] = include_bytes!("../../data/fonts/Sauce/ibm/IBM_VGA_869.psf");
const IBM_VGA50_869: &[u8] = include_bytes!("../../data/fonts/Sauce/ibm/IBM_VGA50_869.psf");
const IBM_EGA_869: &[u8] = include_bytes!("../../data/fonts/Sauce/ibm/IBM_EGA_869.psf");
const IBM_EGA43_869: &[u8] = include_bytes!("../../data/fonts/Sauce/ibm/IBM_EGA43_869.psf");

// ============================================================================
// IBM PC Fonts - MIK (Bulgarian Cyrillic, uses CP866 data)
// ============================================================================
const IBM_VGA_MIK: &[u8] = include_bytes!("../../data/fonts/Sauce/ibm/IBM_VGA_MIK.psf");
const IBM_VGA50_MIK: &[u8] = include_bytes!("../../data/fonts/Sauce/ibm/IBM_VGA50_MIK.psf");
const IBM_EGA_MIK: &[u8] = include_bytes!("../../data/fonts/Sauce/ibm/IBM_EGA_MIK.psf");
const IBM_EGA43_MIK: &[u8] = include_bytes!("../../data/fonts/Sauce/ibm/IBM_EGA43_MIK.psf");

// ============================================================================
// Amiga Fonts
// ============================================================================
const AMIGA_TOPAZ_1: &[u8] = include_bytes!("../../data/fonts/Sauce/amiga/Amiga_Topaz_1.psf");
const AMIGA_TOPAZ_1_PLUS: &[u8] = include_bytes!("../../data/fonts/Sauce/amiga/Amiga_Topaz_1Plus.psf");
const AMIGA_TOPAZ_2: &[u8] = include_bytes!("../../data/fonts/Sauce/amiga/Amiga_Topaz_2.psf");
const AMIGA_TOPAZ_2_PLUS: &[u8] = include_bytes!("../../data/fonts/Sauce/amiga/Amiga_Topaz_2Plus.psf");
const AMIGA_P0T_NOODLE: &[u8] = include_bytes!("../../data/fonts/Sauce/amiga/Amiga_P0T-NOoDLE.psf");
const AMIGA_MICROKNIGHT: &[u8] = include_bytes!("../../data/fonts/Sauce/amiga/Amiga_MicroKnight.psf");
const AMIGA_MICROKNIGHT_PLUS: &[u8] = include_bytes!("../../data/fonts/Sauce/amiga/Amiga_MicroKnightPlus.psf");
const AMIGA_MOSOUL: &[u8] = include_bytes!("../../data/fonts/Sauce/amiga/Amiga_mOsOul.psf");

// ============================================================================
// Commodore 64 Fonts
// ============================================================================
const C64_PETSCII_UNSHIFTED: &[u8] = include_bytes!("../../data/fonts/Sauce/c64/C64_PETSCII_unshifted.psf");
const C64_PETSCII_SHIFTED: &[u8] = include_bytes!("../../data/fonts/Sauce/c64/C64_PETSCII_shifted.psf");

// ============================================================================
// Atari Fonts
// ============================================================================
const ATARI_ATASCII: &[u8] = include_bytes!("../../data/fonts/Sauce/atari/Atari_ATASCII.psf");

/// Complete SAUCE font mapping table
///
/// According to SAUCE spec, these are the standard font names.
/// See: https://www.acid.org/info/sauce/sauce.htm
///
/// Font data sourced from Moebius (blocktronics/moebius).
pub static SAUCE_FONT_MAP: &[SauceFontMapping] = &[
    // ========================================
    // IBM PC VGA Fonts (CP437 base)
    // ========================================
    SauceFontMapping {
        sauce_name: "IBM VGA",
        data: IBM_VGA,
    },
    SauceFontMapping {
        sauce_name: "IBM VGA50",
        data: IBM_VGA50,
    },
    SauceFontMapping {
        sauce_name: "IBM VGA25G",
        data: IBM_VGA25G,
    },
    SauceFontMapping {
        sauce_name: "IBM EGA",
        data: IBM_EGA,
    },
    SauceFontMapping {
        sauce_name: "IBM EGA43",
        data: IBM_EGA43,
    },
    // CP437 explicit
    SauceFontMapping {
        sauce_name: "IBM VGA 437",
        data: IBM_VGA_437,
    },
    SauceFontMapping {
        sauce_name: "IBM VGA50 437",
        data: IBM_VGA50_437,
    },
    SauceFontMapping {
        sauce_name: "IBM VGA25G 437",
        data: IBM_VGA25G_437,
    },
    SauceFontMapping {
        sauce_name: "IBM EGA 437",
        data: IBM_EGA_437,
    },
    SauceFontMapping {
        sauce_name: "IBM EGA43 437",
        data: IBM_EGA43_437,
    },
    // CP737 (Greek)
    SauceFontMapping {
        sauce_name: "IBM VGA 737",
        data: IBM_VGA_737,
    },
    SauceFontMapping {
        sauce_name: "IBM VGA50 737",
        data: IBM_VGA50_737,
    },
    SauceFontMapping {
        sauce_name: "IBM EGA 737",
        data: IBM_EGA_737,
    },
    SauceFontMapping {
        sauce_name: "IBM EGA43 737",
        data: IBM_EGA43_737,
    },
    // CP775 (Baltic)
    SauceFontMapping {
        sauce_name: "IBM VGA 775",
        data: IBM_VGA_775,
    },
    SauceFontMapping {
        sauce_name: "IBM VGA50 775",
        data: IBM_VGA50_775,
    },
    SauceFontMapping {
        sauce_name: "IBM EGA 775",
        data: IBM_EGA_775,
    },
    SauceFontMapping {
        sauce_name: "IBM EGA43 775",
        data: IBM_EGA43_775,
    },
    // CP850 (Western Europe)
    SauceFontMapping {
        sauce_name: "IBM VGA 850",
        data: IBM_VGA_850,
    },
    SauceFontMapping {
        sauce_name: "IBM VGA50 850",
        data: IBM_VGA50_850,
    },
    SauceFontMapping {
        sauce_name: "IBM VGA25G 850",
        data: IBM_VGA25G_850,
    },
    SauceFontMapping {
        sauce_name: "IBM EGA 850",
        data: IBM_EGA_850,
    },
    SauceFontMapping {
        sauce_name: "IBM EGA43 850",
        data: IBM_EGA43_850,
    },
    // CP851 (Greek)
    SauceFontMapping {
        sauce_name: "IBM VGA 851",
        data: IBM_VGA_851,
    },
    SauceFontMapping {
        sauce_name: "IBM VGA50 851",
        data: IBM_VGA50_851,
    },
    SauceFontMapping {
        sauce_name: "IBM VGA25G 851",
        data: IBM_VGA25G_851,
    },
    SauceFontMapping {
        sauce_name: "IBM EGA 851",
        data: IBM_EGA_851,
    },
    SauceFontMapping {
        sauce_name: "IBM EGA43 851",
        data: IBM_EGA43_851,
    },
    // CP852 (Central Europe)
    SauceFontMapping {
        sauce_name: "IBM VGA 852",
        data: IBM_VGA_852,
    },
    SauceFontMapping {
        sauce_name: "IBM VGA50 852",
        data: IBM_VGA50_852,
    },
    SauceFontMapping {
        sauce_name: "IBM VGA25G 852",
        data: IBM_VGA25G_852,
    },
    SauceFontMapping {
        sauce_name: "IBM EGA 852",
        data: IBM_EGA_852,
    },
    SauceFontMapping {
        sauce_name: "IBM EGA43 852",
        data: IBM_EGA43_852,
    },
    // CP853 (Multilingual)
    SauceFontMapping {
        sauce_name: "IBM VGA 853",
        data: IBM_VGA_853,
    },
    SauceFontMapping {
        sauce_name: "IBM VGA50 853",
        data: IBM_VGA50_853,
    },
    SauceFontMapping {
        sauce_name: "IBM VGA25G 853",
        data: IBM_VGA25G_853,
    },
    SauceFontMapping {
        sauce_name: "IBM EGA 853",
        data: IBM_EGA_853,
    },
    SauceFontMapping {
        sauce_name: "IBM EGA43 853",
        data: IBM_EGA43_853,
    },
    // CP855 (Cyrillic)
    SauceFontMapping {
        sauce_name: "IBM VGA 855",
        data: IBM_VGA_855,
    },
    SauceFontMapping {
        sauce_name: "IBM VGA50 855",
        data: IBM_VGA50_855,
    },
    SauceFontMapping {
        sauce_name: "IBM EGA 855",
        data: IBM_EGA_855,
    },
    SauceFontMapping {
        sauce_name: "IBM EGA43 855",
        data: IBM_EGA43_855,
    },
    // CP857 (Turkish)
    SauceFontMapping {
        sauce_name: "IBM VGA 857",
        data: IBM_VGA_857,
    },
    SauceFontMapping {
        sauce_name: "IBM VGA50 857",
        data: IBM_VGA50_857,
    },
    SauceFontMapping {
        sauce_name: "IBM EGA 857",
        data: IBM_EGA_857,
    },
    SauceFontMapping {
        sauce_name: "IBM EGA43 857",
        data: IBM_EGA43_857,
    },
    // CP860 (Portuguese)
    SauceFontMapping {
        sauce_name: "IBM VGA 860",
        data: IBM_VGA_860,
    },
    SauceFontMapping {
        sauce_name: "IBM VGA50 860",
        data: IBM_VGA50_860,
    },
    SauceFontMapping {
        sauce_name: "IBM VGA25G 860",
        data: IBM_VGA25G_860,
    },
    SauceFontMapping {
        sauce_name: "IBM EGA 860",
        data: IBM_EGA_860,
    },
    SauceFontMapping {
        sauce_name: "IBM EGA43 860",
        data: IBM_EGA43_860,
    },
    // CP861 (Icelandic)
    SauceFontMapping {
        sauce_name: "IBM VGA 861",
        data: IBM_VGA_861,
    },
    SauceFontMapping {
        sauce_name: "IBM VGA50 861",
        data: IBM_VGA50_861,
    },
    SauceFontMapping {
        sauce_name: "IBM VGA25G 861",
        data: IBM_VGA25G_861,
    },
    SauceFontMapping {
        sauce_name: "IBM EGA 861",
        data: IBM_EGA_861,
    },
    SauceFontMapping {
        sauce_name: "IBM EGA43 861",
        data: IBM_EGA43_861,
    },
    // CP862 (Hebrew)
    SauceFontMapping {
        sauce_name: "IBM VGA 862",
        data: IBM_VGA_862,
    },
    SauceFontMapping {
        sauce_name: "IBM VGA50 862",
        data: IBM_VGA50_862,
    },
    SauceFontMapping {
        sauce_name: "IBM EGA 862",
        data: IBM_EGA_862,
    },
    SauceFontMapping {
        sauce_name: "IBM EGA43 862",
        data: IBM_EGA43_862,
    },
    // CP863 (French Canadian)
    SauceFontMapping {
        sauce_name: "IBM VGA 863",
        data: IBM_VGA_863,
    },
    SauceFontMapping {
        sauce_name: "IBM VGA50 863",
        data: IBM_VGA50_863,
    },
    SauceFontMapping {
        sauce_name: "IBM VGA25G 863",
        data: IBM_VGA25G_863,
    },
    SauceFontMapping {
        sauce_name: "IBM EGA 863",
        data: IBM_EGA_863,
    },
    SauceFontMapping {
        sauce_name: "IBM EGA43 863",
        data: IBM_EGA43_863,
    },
    // CP864 (Arabic)
    SauceFontMapping {
        sauce_name: "IBM VGA 864",
        data: IBM_VGA_864,
    },
    SauceFontMapping {
        sauce_name: "IBM VGA50 864",
        data: IBM_VGA50_864,
    },
    SauceFontMapping {
        sauce_name: "IBM EGA 864",
        data: IBM_EGA_864,
    },
    SauceFontMapping {
        sauce_name: "IBM EGA43 864",
        data: IBM_EGA43_864,
    },
    // CP865 (Nordic)
    SauceFontMapping {
        sauce_name: "IBM VGA 865",
        data: IBM_VGA_865,
    },
    SauceFontMapping {
        sauce_name: "IBM VGA50 865",
        data: IBM_VGA50_865,
    },
    SauceFontMapping {
        sauce_name: "IBM VGA25G 865",
        data: IBM_VGA25G_865,
    },
    SauceFontMapping {
        sauce_name: "IBM EGA 865",
        data: IBM_EGA_865,
    },
    SauceFontMapping {
        sauce_name: "IBM EGA43 865",
        data: IBM_EGA43_865,
    },
    // CP866 (Cyrillic/Russian)
    SauceFontMapping {
        sauce_name: "IBM VGA 866",
        data: IBM_VGA_866,
    },
    SauceFontMapping {
        sauce_name: "IBM VGA50 866",
        data: IBM_VGA50_866,
    },
    SauceFontMapping {
        sauce_name: "IBM EGA 866",
        data: IBM_EGA_866,
    },
    SauceFontMapping {
        sauce_name: "IBM EGA43 866",
        data: IBM_EGA43_866,
    },
    // CP869 (Greek 2)
    SauceFontMapping {
        sauce_name: "IBM VGA 869",
        data: IBM_VGA_869,
    },
    SauceFontMapping {
        sauce_name: "IBM VGA50 869",
        data: IBM_VGA50_869,
    },
    SauceFontMapping {
        sauce_name: "IBM EGA 869",
        data: IBM_EGA_869,
    },
    SauceFontMapping {
        sauce_name: "IBM EGA43 869",
        data: IBM_EGA43_869,
    },
    // MIK (Bulgarian Cyrillic)
    SauceFontMapping {
        sauce_name: "IBM VGA MIK",
        data: IBM_VGA_MIK,
    },
    SauceFontMapping {
        sauce_name: "IBM VGA50 MIK",
        data: IBM_VGA50_MIK,
    },
    SauceFontMapping {
        sauce_name: "IBM EGA MIK",
        data: IBM_EGA_MIK,
    },
    SauceFontMapping {
        sauce_name: "IBM EGA43 MIK",
        data: IBM_EGA43_MIK,
    },
    // ========================================
    // Amiga Fonts
    // ========================================
    SauceFontMapping {
        sauce_name: "Amiga Topaz 1",
        data: AMIGA_TOPAZ_1,
    },
    SauceFontMapping {
        sauce_name: "Amiga Topaz 1+",
        data: AMIGA_TOPAZ_1_PLUS,
    },
    SauceFontMapping {
        sauce_name: "Amiga Topaz 2",
        data: AMIGA_TOPAZ_2,
    },
    SauceFontMapping {
        sauce_name: "Amiga Topaz 2+",
        data: AMIGA_TOPAZ_2_PLUS,
    },
    SauceFontMapping {
        sauce_name: "Amiga P0T-NOoDLE",
        data: AMIGA_P0T_NOODLE,
    },
    SauceFontMapping {
        sauce_name: "Amiga MicroKnight",
        data: AMIGA_MICROKNIGHT,
    },
    SauceFontMapping {
        sauce_name: "Amiga MicroKnight+",
        data: AMIGA_MICROKNIGHT_PLUS,
    },
    SauceFontMapping {
        sauce_name: "Amiga mOsOul",
        data: AMIGA_MOSOUL,
    },
    // ========================================
    // Commodore 64 Fonts
    // ========================================
    SauceFontMapping {
        sauce_name: "C64 PETSCII unshifted",
        data: C64_PETSCII_UNSHIFTED,
    },
    SauceFontMapping {
        sauce_name: "C64 PETSCII shifted",
        data: C64_PETSCII_SHIFTED,
    },
    // ========================================
    // Atari Fonts
    // ========================================
    SauceFontMapping {
        sauce_name: "Atari ATASCII",
        data: ATARI_ATASCII,
    },
];

/// Load a font by SAUCE name with fallback chain.
///
/// Fallback chain (according to SAUCE spec):
/// 1. Exact match
/// 2. For "IBM VGA50 ###" / "IBM VGA25G ###" → try "IBM VGA ###"
/// 3. For "IBM EGA43 ###" → try "IBM EGA ###"
/// 4. For "IBM VGA/EGA ###" → try "IBM VGA/EGA" (base without codepage)
/// 5. For "Amiga Font+" → try "Amiga Font"
/// 6. CP equivalences: 872↔855, 858↔850, 865↔437
/// 7. Default to "IBM VGA"
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

    // Fallback 1: Height variants - VGA50/VGA25G → VGA, EGA43 → EGA
    if sauce_name.contains("VGA50 ") || sauce_name.contains("VGA25G ") {
        let fallback = sauce_name.replace("VGA50 ", "VGA ").replace("VGA25G ", "VGA ");
        if let Some(font) = try_load_sauce_font(&fallback) {
            return Ok(font);
        }
    }
    if sauce_name.contains("EGA43 ") {
        let fallback = sauce_name.replace("EGA43 ", "EGA ");
        if let Some(font) = try_load_sauce_font(&fallback) {
            return Ok(font);
        }
    }

    // Fallback 2: For "IBM VGA ###" or "IBM EGA ###", try base name without codepage
    if sauce_name.starts_with("IBM VGA") && sauce_name.len() > 7 {
        // Try base variant (VGA/VGA50/VGA25G/EGA/EGA43)
        let parts: Vec<&str> = sauce_name.split_whitespace().collect();
        if parts.len() >= 2 {
            let base = format!("IBM {}", parts[1]);
            if let Some(font) = try_load_sauce_font(&base) {
                return Ok(font);
            }
        }
        // Finally try plain IBM VGA
        if let Some(font) = try_load_sauce_font("IBM VGA") {
            return Ok(font);
        }
    }
    if sauce_name.starts_with("IBM EGA") && sauce_name.len() > 7 {
        let parts: Vec<&str> = sauce_name.split_whitespace().collect();
        if parts.len() >= 2 {
            let base = format!("IBM {}", parts[1]);
            if let Some(font) = try_load_sauce_font(&base) {
                return Ok(font);
            }
        }
        if let Some(font) = try_load_sauce_font("IBM EGA") {
            return Ok(font);
        }
    }

    // Fallback 3: For "Amiga Font+" → try "Amiga Font"
    if sauce_name.ends_with('+') {
        let base_name = &sauce_name[..sauce_name.len() - 1];
        if let Some(font) = try_load_sauce_font(base_name) {
            return Ok(font);
        }
        // Fallback to Topaz 1
        if let Some(font) = try_load_sauce_font("Amiga Topaz 1") {
            return Ok(font);
        }
    }

    // Fallback 4: CP equivalences (872↔855, 858↔850, 865↔437)
    if sauce_name.contains(" 872") {
        let fallback = sauce_name.replace(" 872", " 855");
        if let Some(font) = try_load_sauce_font(&fallback) {
            return Ok(font);
        }
    }
    if sauce_name.contains(" 858") {
        let fallback = sauce_name.replace(" 858", " 850");
        if let Some(font) = try_load_sauce_font(&fallback) {
            return Ok(font);
        }
    }
    if sauce_name.contains(" 865") {
        let fallback = sauce_name.replace(" 865", " 437");
        if let Some(font) = try_load_sauce_font(&fallback) {
            return Ok(font);
        }
    }

    // Final fallback: Default to IBM VGA
    try_load_sauce_font("IBM VGA").ok_or(crate::EngineError::FontNotFound)
}

/// Try to load a SAUCE font by exact name match
fn try_load_sauce_font(sauce_name: &str) -> Option<BitFont> {
    for mapping in SAUCE_FONT_MAP {
        if mapping.sauce_name.eq_ignore_ascii_case(sauce_name) {
            return BitFont::from_bytes(mapping.sauce_name, mapping.data).ok();
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
