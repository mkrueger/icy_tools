//! ANSI Terminal Font Slots (0-42)
//!
//! Multi-Size Varianten (f08, f14, f16)
//! Auswahl basierend auf Zeilenzahl: 50→f08, 28→f14, 25→f16

use std::sync::OnceLock;

use super::BitFont;

/// Macro to create array of OnceLock
macro_rules! once_lock_array {
    ($n:expr) => {{
        const INIT: [OnceLock<BitFont>; 3] = [OnceLock::new(), OnceLock::new(), OnceLock::new()];
        [INIT; $n]
    }};
}

/// Static cache for each slot/height combination: [slot][height_index]
/// height_index: 0=8px, 1=14px, 2=16px
static FONT_CACHE: [[OnceLock<BitFont>; 3]; ANSI_SLOT_COUNT] = once_lock_array!(ANSI_SLOT_COUNT);

/// ANSI Font Slot mit optionalen Größenvarianten
pub struct AnsiSlotFont {
    pub slot: usize,
    pub name: &'static str,
    pub f08: Option<&'static [u8]>, // 8px Höhe (50 Zeilen)
    pub f14: Option<&'static [u8]>, // 14px Höhe (28 Zeilen)
    pub f16: Option<&'static [u8]>, // 16px Höhe (25 Zeilen)
}

/// Anzahl der ANSI Font Slots (0-42)
pub const ANSI_SLOT_COUNT: usize = 43;

/// Default Font Name (Slot 0)
pub const DEFAULT_FONT_NAME: &str = "Codepage 437 English";
pub const ALT_DEFAULT_FONT_NAME: &str = "IBM VGA";

/// CP437 raw bytes for direct access (Slot 0, 16px)
pub const CP437: &[u8] = include_bytes!("../../data/fonts/ansi/00-Codepage_437_English.f16");

// Include all font data files
// Slot 0: Codepage 437 English
const SLOT_00_F08: &[u8] = include_bytes!("../../data/fonts/ansi/00-Codepage_437_English.f08");
const SLOT_00_F14: &[u8] = include_bytes!("../../data/fonts/ansi/00-Codepage_437_English.f14");
const SLOT_00_F16: &[u8] = include_bytes!("../../data/fonts/ansi/00-Codepage_437_English.f16");

// Slot 1: Codepage 1251 Cyrillic (swiss)
const SLOT_01_F16: &[u8] = include_bytes!("../../data/fonts/ansi/01-Codepage_1251_Cyrillic_swiss.f16");

// Slot 2: Russian koi8-r
const SLOT_02_F08: &[u8] = include_bytes!("../../data/fonts/ansi/02-Russian_koi8-r.f08");
const SLOT_02_F14: &[u8] = include_bytes!("../../data/fonts/ansi/02-Russian_koi8-r.f14");
const SLOT_02_F16: &[u8] = include_bytes!("../../data/fonts/ansi/02-Russian_koi8-r.f16");

// Slot 3: ISO-8859-2 Central European
const SLOT_03_F08: &[u8] = include_bytes!("../../data/fonts/ansi/03-ISO-8859-2_Central_European.f08");
const SLOT_03_F14: &[u8] = include_bytes!("../../data/fonts/ansi/03-ISO-8859-2_Central_European.f14");
const SLOT_03_F16: &[u8] = include_bytes!("../../data/fonts/ansi/03-ISO-8859-2_Central_European.f16");

// Slot 4: ISO-8859-4 Baltic wide VGA 9bit mapped
const SLOT_04_F16: &[u8] = include_bytes!("../../data/fonts/ansi/04-ISO-8859-4_Baltic_wide_VGA_9bit_mapped.f16");

// Slot 5: Codepage 866 (c) Russian
const SLOT_05_F16: &[u8] = include_bytes!("../../data/fonts/ansi/05-Codepage_866_c_Russian.f16");

// Slot 6: ISO-8859-9 Turkish
const SLOT_06_F16: &[u8] = include_bytes!("../../data/fonts/ansi/06-ISO-8859-9_Turkish.f16");

// Slot 7: haik8 codepage (ARMSCII-8 screenmap)
const SLOT_07_F08: &[u8] = include_bytes!("../../data/fonts/ansi/07-haik8_codepage_use_only_with_armscii8_screenmap.f08");
const SLOT_07_F14: &[u8] = include_bytes!("../../data/fonts/ansi/07-haik8_codepage_use_only_with_armscii8_screenmap.f14");
const SLOT_07_F16: &[u8] = include_bytes!("../../data/fonts/ansi/07-haik8_codepage_use_only_with_armscii8_screenmap.f16");

// Slot 8: ISO-8859-8 Hebrew
const SLOT_08_F08: &[u8] = include_bytes!("../../data/fonts/ansi/08-ISO-8859-8_Hebrew.f08");
const SLOT_08_F14: &[u8] = include_bytes!("../../data/fonts/ansi/08-ISO-8859-8_Hebrew.f14");
const SLOT_08_F16: &[u8] = include_bytes!("../../data/fonts/ansi/08-ISO-8859-8_Hebrew.f16");

// Slot 9: Ukrainian font koi8-u
const SLOT_09_F08: &[u8] = include_bytes!("../../data/fonts/ansi/09-Ukrainian_font_koi8-u.f08");
const SLOT_09_F14: &[u8] = include_bytes!("../../data/fonts/ansi/09-Ukrainian_font_koi8-u.f14");
const SLOT_09_F16: &[u8] = include_bytes!("../../data/fonts/ansi/09-Ukrainian_font_koi8-u.f16");

// Slot 10: ISO-8859-15 West European (thin)
const SLOT_10_F16: &[u8] = include_bytes!("../../data/fonts/ansi/10-ISO-8859-15_West_European_thin.f16");

// Slot 11: ISO-8859-4 Baltic VGA 9bit mapped
const SLOT_11_F08: &[u8] = include_bytes!("../../data/fonts/ansi/11-ISO-8859-4_Baltic_VGA_9bit_mapped.f08");
const SLOT_11_F14: &[u8] = include_bytes!("../../data/fonts/ansi/11-ISO-8859-4_Baltic_VGA_9bit_mapped.f14");
const SLOT_11_F16: &[u8] = include_bytes!("../../data/fonts/ansi/11-ISO-8859-4_Baltic_VGA_9bit_mapped.f16");

// Slot 12: Russian koi8-r (b)
const SLOT_12_F16: &[u8] = include_bytes!("../../data/fonts/ansi/12-Russian_koi8-r_b.f16");

// Slot 13: ISO-8859-4 Baltic wide
const SLOT_13_F16: &[u8] = include_bytes!("../../data/fonts/ansi/13-ISO-8859-4_Baltic_wide.f16");

// Slot 14: ISO-8859-5 Cyrillic
const SLOT_14_F08: &[u8] = include_bytes!("../../data/fonts/ansi/14-ISO-8859-5_Cyrillic.f08");
const SLOT_14_F14: &[u8] = include_bytes!("../../data/fonts/ansi/14-ISO-8859-5_Cyrillic.f14");
const SLOT_14_F16: &[u8] = include_bytes!("../../data/fonts/ansi/14-ISO-8859-5_Cyrillic.f16");

// Slot 15: ARMSCII-8 Character set
const SLOT_15_F08: &[u8] = include_bytes!("../../data/fonts/ansi/15-ARMSCII-8_Character_set.f08");
const SLOT_15_F14: &[u8] = include_bytes!("../../data/fonts/ansi/15-ARMSCII-8_Character_set.f14");
const SLOT_15_F16: &[u8] = include_bytes!("../../data/fonts/ansi/15-ARMSCII-8_Character_set.f16");

// Slot 16: ISO-8859-15 West European
const SLOT_16_F08: &[u8] = include_bytes!("../../data/fonts/ansi/16-ISO-8859-15_West_European.f08");
const SLOT_16_F14: &[u8] = include_bytes!("../../data/fonts/ansi/16-ISO-8859-15_West_European.f14");
const SLOT_16_F16: &[u8] = include_bytes!("../../data/fonts/ansi/16-ISO-8859-15_West_European.f16");

// Slot 17: Codepage 850 Multilingual Latin I (thin)
const SLOT_17_F16: &[u8] = include_bytes!("../../data/fonts/ansi/17-Codepage_850_Multilingual_Latin_I_thin.f16");

// Slot 18: Codepage 850 Multilingual Latin I
const SLOT_18_F08: &[u8] = include_bytes!("../../data/fonts/ansi/18-Codepage_850_Multilingual_Latin_I.f08");
const SLOT_18_F14: &[u8] = include_bytes!("../../data/fonts/ansi/18-Codepage_850_Multilingual_Latin_I.f14");
const SLOT_18_F16: &[u8] = include_bytes!("../../data/fonts/ansi/18-Codepage_850_Multilingual_Latin_I.f16");

// Slot 19: Codepage 865 Norwegian (thin)
const SLOT_19_F16: &[u8] = include_bytes!("../../data/fonts/ansi/19-Codepage_865_Norwegian_thin.f16");

// Slot 20: Codepage 1251 Cyrillic
const SLOT_20_F08: &[u8] = include_bytes!("../../data/fonts/ansi/20-Codepage_1251_Cyrillic.f08");
const SLOT_20_F14: &[u8] = include_bytes!("../../data/fonts/ansi/20-Codepage_1251_Cyrillic.f14");
const SLOT_20_F16: &[u8] = include_bytes!("../../data/fonts/ansi/20-Codepage_1251_Cyrillic.f16");

// Slot 21: ISO-8859-7 Greek
const SLOT_21_F08: &[u8] = include_bytes!("../../data/fonts/ansi/21-ISO-8859-7_Greek.f08");
const SLOT_21_F14: &[u8] = include_bytes!("../../data/fonts/ansi/21-ISO-8859-7_Greek.f14");
const SLOT_21_F16: &[u8] = include_bytes!("../../data/fonts/ansi/21-ISO-8859-7_Greek.f16");

// Slot 22: Russian koi8-r (c)
const SLOT_22_F16: &[u8] = include_bytes!("../../data/fonts/ansi/22-Russian_koi8-r_c.f16");

// Slot 23: ISO-8859-4 Baltic
const SLOT_23_F08: &[u8] = include_bytes!("../../data/fonts/ansi/23-ISO-8859-4_Baltic.f08");
const SLOT_23_F14: &[u8] = include_bytes!("../../data/fonts/ansi/23-ISO-8859-4_Baltic.f14");
const SLOT_23_F16: &[u8] = include_bytes!("../../data/fonts/ansi/23-ISO-8859-4_Baltic.f16");

// Slot 24: ISO-8859-1 West European
const SLOT_24_F08: &[u8] = include_bytes!("../../data/fonts/ansi/24-ISO-8859-1_West_European.f08");
const SLOT_24_F14: &[u8] = include_bytes!("../../data/fonts/ansi/24-ISO-8859-1_West_European.f14");
const SLOT_24_F16: &[u8] = include_bytes!("../../data/fonts/ansi/24-ISO-8859-1_West_European.f16");

// Slot 25: Codepage 866 Russian
const SLOT_25_F08: &[u8] = include_bytes!("../../data/fonts/ansi/25-Codepage_866_Russian.f08");
const SLOT_25_F14: &[u8] = include_bytes!("../../data/fonts/ansi/25-Codepage_866_Russian.f14");
const SLOT_25_F16: &[u8] = include_bytes!("../../data/fonts/ansi/25-Codepage_866_Russian.f16");

// Slot 26: Codepage 437 English (thin)
const SLOT_26_F16: &[u8] = include_bytes!("../../data/fonts/ansi/26-Codepage_437_English_thin.f16");

// Slot 27: Codepage 866 (b) Russian
const SLOT_27_F16: &[u8] = include_bytes!("../../data/fonts/ansi/27-Codepage_866_b_Russian.f16");

// Slot 28: Codepage 865 Norwegian
const SLOT_28_F08: &[u8] = include_bytes!("../../data/fonts/ansi/28-Codepage_865_Norwegian.f08");
const SLOT_28_F14: &[u8] = include_bytes!("../../data/fonts/ansi/28-Codepage_865_Norwegian.f14");
const SLOT_28_F16: &[u8] = include_bytes!("../../data/fonts/ansi/28-Codepage_865_Norwegian.f16");

// Slot 29: Ukrainian font cp866u
const SLOT_29_F08: &[u8] = include_bytes!("../../data/fonts/ansi/29-Ukrainian_font_cp866u.f08");
const SLOT_29_F14: &[u8] = include_bytes!("../../data/fonts/ansi/29-Ukrainian_font_cp866u.f14");
const SLOT_29_F16: &[u8] = include_bytes!("../../data/fonts/ansi/29-Ukrainian_font_cp866u.f16");

// Slot 30: ISO-8859-1 West European (thin)
const SLOT_30_F16: &[u8] = include_bytes!("../../data/fonts/ansi/30-ISO-8859-1_West_European_thin.f16");

// Slot 31: Codepage 1131 Belarusian (swiss)
const SLOT_31_F16: &[u8] = include_bytes!("../../data/fonts/ansi/31-Codepage_1131_Belarusian_swiss.f16");

// Slot 32: Commodore 64 UPPER
const SLOT_32_F16: &[u8] = include_bytes!("../../data/fonts/ansi/32-Commodore_64_UPPER.f16");

// Slot 33: Commodore 64 Lower
const SLOT_33_F16: &[u8] = include_bytes!("../../data/fonts/ansi/33-Commodore_64_Lower.f16");

// Slot 34: Commodore 128 UPPER
const SLOT_34_F16: &[u8] = include_bytes!("../../data/fonts/ansi/34-Commodore_128_UPPER.f16");

// Slot 35: Commodore 128 Lower
const SLOT_35_F16: &[u8] = include_bytes!("../../data/fonts/ansi/35-Commodore_128_Lower.f16");

// Slot 36: Atari
const SLOT_36_F16: &[u8] = include_bytes!("../../data/fonts/ansi/36-Atari.f16");

// Slot 37: P0T NOoDLE (Amiga)
const SLOT_37_F14: &[u8] = include_bytes!("../../data/fonts/ansi/37-P0T_NOoDLE_Amiga.f14");
const SLOT_37_F16: &[u8] = include_bytes!("../../data/fonts/ansi/37-P0T_NOoDLE_Amiga.f16");

// Slot 38: mOsOul (Amiga)
const SLOT_38_F16: &[u8] = include_bytes!("../../data/fonts/ansi/38-mOsOul_Amiga.f16");

// Slot 39: MicroKnight Plus (Amiga)
const SLOT_39_F16: &[u8] = include_bytes!("../../data/fonts/ansi/39-MicroKnight_Plus_Amiga.f16");

// Slot 40: Topaz Plus (Amiga)
const SLOT_40_F16: &[u8] = include_bytes!("../../data/fonts/ansi/40-Topaz_Plus_Amiga.f16");

// Slot 41: MicroKnight (Amiga)
const SLOT_41_F16: &[u8] = include_bytes!("../../data/fonts/ansi/41-MicroKnight_Amiga.f16");

// Slot 42: Topaz (Amiga)
const SLOT_42_F14: &[u8] = include_bytes!("../../data/fonts/ansi/42-Topaz_Amiga.f14");
const SLOT_42_F16: &[u8] = include_bytes!("../../data/fonts/ansi/42-Topaz_Amiga.f16");

/// Complete table of all ANSI font slots (0-42)
pub static ANSI_SLOT_FONTS: [AnsiSlotFont; ANSI_SLOT_COUNT] = [
    // Slot 0: Codepage 437 English (Default)
    AnsiSlotFont {
        slot: 0,
        name: "Codepage 437 English",
        f08: Some(SLOT_00_F08),
        f14: Some(SLOT_00_F14),
        f16: Some(SLOT_00_F16),
    },
    // Slot 1: Codepage 1251 Cyrillic (swiss)
    AnsiSlotFont {
        slot: 1,
        name: "Codepage 1251 Cyrillic (swiss)",
        f08: None,
        f14: None,
        f16: Some(SLOT_01_F16),
    },
    // Slot 2: Russian koi8-r
    AnsiSlotFont {
        slot: 2,
        name: "Russian koi8-r",
        f08: Some(SLOT_02_F08),
        f14: Some(SLOT_02_F14),
        f16: Some(SLOT_02_F16),
    },
    // Slot 3: ISO-8859-2 Central European
    AnsiSlotFont {
        slot: 3,
        name: "ISO-8859-2 Central European",
        f08: Some(SLOT_03_F08),
        f14: Some(SLOT_03_F14),
        f16: Some(SLOT_03_F16),
    },
    // Slot 4: ISO-8859-4 Baltic wide VGA 9bit mapped
    AnsiSlotFont {
        slot: 4,
        name: "ISO-8859-4 Baltic wide (VGA 9bit mapped)",
        f08: None,
        f14: None,
        f16: Some(SLOT_04_F16),
    },
    // Slot 5: Codepage 866 (c) Russian
    AnsiSlotFont {
        slot: 5,
        name: "Codepage 866 (c) Russian",
        f08: None,
        f14: None,
        f16: Some(SLOT_05_F16),
    },
    // Slot 6: ISO-8859-9 Turkish
    AnsiSlotFont {
        slot: 6,
        name: "ISO-8859-9 Turkish",
        f08: None,
        f14: None,
        f16: Some(SLOT_06_F16),
    },
    // Slot 7: haik8 codepage
    AnsiSlotFont {
        slot: 7,
        name: "haik8 codepage",
        f08: Some(SLOT_07_F08),
        f14: Some(SLOT_07_F14),
        f16: Some(SLOT_07_F16),
    },
    // Slot 8: ISO-8859-8 Hebrew
    AnsiSlotFont {
        slot: 8,
        name: "ISO-8859-8 Hebrew",
        f08: Some(SLOT_08_F08),
        f14: Some(SLOT_08_F14),
        f16: Some(SLOT_08_F16),
    },
    // Slot 9: Ukrainian font koi8-u
    AnsiSlotFont {
        slot: 9,
        name: "Ukrainian font koi8-u",
        f08: Some(SLOT_09_F08),
        f14: Some(SLOT_09_F14),
        f16: Some(SLOT_09_F16),
    },
    // Slot 10: ISO-8859-15 West European (thin)
    AnsiSlotFont {
        slot: 10,
        name: "ISO-8859-15 West European (thin)",
        f08: None,
        f14: None,
        f16: Some(SLOT_10_F16),
    },
    // Slot 11: ISO-8859-4 Baltic VGA 9bit mapped
    AnsiSlotFont {
        slot: 11,
        name: "ISO-8859-4 Baltic (VGA 9bit mapped)",
        f08: Some(SLOT_11_F08),
        f14: Some(SLOT_11_F14),
        f16: Some(SLOT_11_F16),
    },
    // Slot 12: Russian koi8-r (b)
    AnsiSlotFont {
        slot: 12,
        name: "Russian koi8-r (b)",
        f08: None,
        f14: None,
        f16: Some(SLOT_12_F16),
    },
    // Slot 13: ISO-8859-4 Baltic wide
    AnsiSlotFont {
        slot: 13,
        name: "ISO-8859-4 Baltic wide",
        f08: None,
        f14: None,
        f16: Some(SLOT_13_F16),
    },
    // Slot 14: ISO-8859-5 Cyrillic
    AnsiSlotFont {
        slot: 14,
        name: "ISO-8859-5 Cyrillic",
        f08: Some(SLOT_14_F08),
        f14: Some(SLOT_14_F14),
        f16: Some(SLOT_14_F16),
    },
    // Slot 15: ARMSCII-8 Character set
    AnsiSlotFont {
        slot: 15,
        name: "ARMSCII-8 Character set",
        f08: Some(SLOT_15_F08),
        f14: Some(SLOT_15_F14),
        f16: Some(SLOT_15_F16),
    },
    // Slot 16: ISO-8859-15 West European
    AnsiSlotFont {
        slot: 16,
        name: "ISO-8859-15 West European",
        f08: Some(SLOT_16_F08),
        f14: Some(SLOT_16_F14),
        f16: Some(SLOT_16_F16),
    },
    // Slot 17: Codepage 850 Multilingual Latin I (thin)
    AnsiSlotFont {
        slot: 17,
        name: "Codepage 850 Multilingual Latin I (thin)",
        f08: None,
        f14: None,
        f16: Some(SLOT_17_F16),
    },
    // Slot 18: Codepage 850 Multilingual Latin I
    AnsiSlotFont {
        slot: 18,
        name: "Codepage 850 Multilingual Latin I",
        f08: Some(SLOT_18_F08),
        f14: Some(SLOT_18_F14),
        f16: Some(SLOT_18_F16),
    },
    // Slot 19: Codepage 865 Norwegian (thin)
    AnsiSlotFont {
        slot: 19,
        name: "Codepage 865 Norwegian (thin)",
        f08: None,
        f14: None,
        f16: Some(SLOT_19_F16),
    },
    // Slot 20: Codepage 1251 Cyrillic
    AnsiSlotFont {
        slot: 20,
        name: "Codepage 1251 Cyrillic",
        f08: Some(SLOT_20_F08),
        f14: Some(SLOT_20_F14),
        f16: Some(SLOT_20_F16),
    },
    // Slot 21: ISO-8859-7 Greek
    AnsiSlotFont {
        slot: 21,
        name: "ISO-8859-7 Greek",
        f08: Some(SLOT_21_F08),
        f14: Some(SLOT_21_F14),
        f16: Some(SLOT_21_F16),
    },
    // Slot 22: Russian koi8-r (c)
    AnsiSlotFont {
        slot: 22,
        name: "Russian koi8-r (c)",
        f08: None,
        f14: None,
        f16: Some(SLOT_22_F16),
    },
    // Slot 23: ISO-8859-4 Baltic
    AnsiSlotFont {
        slot: 23,
        name: "ISO-8859-4 Baltic",
        f08: Some(SLOT_23_F08),
        f14: Some(SLOT_23_F14),
        f16: Some(SLOT_23_F16),
    },
    // Slot 24: ISO-8859-1 West European
    AnsiSlotFont {
        slot: 24,
        name: "ISO-8859-1 West European",
        f08: Some(SLOT_24_F08),
        f14: Some(SLOT_24_F14),
        f16: Some(SLOT_24_F16),
    },
    // Slot 25: Codepage 866 Russian
    AnsiSlotFont {
        slot: 25,
        name: "Codepage 866 Russian",
        f08: Some(SLOT_25_F08),
        f14: Some(SLOT_25_F14),
        f16: Some(SLOT_25_F16),
    },
    // Slot 26: Codepage 437 English (thin)
    AnsiSlotFont {
        slot: 26,
        name: "Codepage 437 English (thin)",
        f08: None,
        f14: None,
        f16: Some(SLOT_26_F16),
    },
    // Slot 27: Codepage 866 (b) Russian
    AnsiSlotFont {
        slot: 27,
        name: "Codepage 866 (b) Russian",
        f08: None,
        f14: None,
        f16: Some(SLOT_27_F16),
    },
    // Slot 28: Codepage 865 Norwegian
    AnsiSlotFont {
        slot: 28,
        name: "Codepage 865 Norwegian",
        f08: Some(SLOT_28_F08),
        f14: Some(SLOT_28_F14),
        f16: Some(SLOT_28_F16),
    },
    // Slot 29: Ukrainian font cp866u
    AnsiSlotFont {
        slot: 29,
        name: "Ukrainian font cp866u",
        f08: Some(SLOT_29_F08),
        f14: Some(SLOT_29_F14),
        f16: Some(SLOT_29_F16),
    },
    // Slot 30: ISO-8859-1 West European (thin)
    AnsiSlotFont {
        slot: 30,
        name: "ISO-8859-1 West European (thin)",
        f08: None,
        f14: None,
        f16: Some(SLOT_30_F16),
    },
    // Slot 31: Codepage 1131 Belarusian (swiss)
    AnsiSlotFont {
        slot: 31,
        name: "Codepage 1131 Belarusian (swiss)",
        f08: None,
        f14: None,
        f16: Some(SLOT_31_F16),
    },
    // Slot 32: Commodore 64 UPPER
    AnsiSlotFont {
        slot: 32,
        name: "Commodore 64 (UPPER)",
        f08: None,
        f14: None,
        f16: Some(SLOT_32_F16),
    },
    // Slot 33: Commodore 64 Lower
    AnsiSlotFont {
        slot: 33,
        name: "Commodore 64 (Lower)",
        f08: None,
        f14: None,
        f16: Some(SLOT_33_F16),
    },
    // Slot 34: Commodore 128 UPPER
    AnsiSlotFont {
        slot: 34,
        name: "Commodore 128 (UPPER)",
        f08: None,
        f14: None,
        f16: Some(SLOT_34_F16),
    },
    // Slot 35: Commodore 128 Lower
    AnsiSlotFont {
        slot: 35,
        name: "Commodore 128 (Lower)",
        f08: None,
        f14: None,
        f16: Some(SLOT_35_F16),
    },
    // Slot 36: Atari
    AnsiSlotFont {
        slot: 36,
        name: "Atari",
        f08: None,
        f14: None,
        f16: Some(SLOT_36_F16),
    },
    // Slot 37: P0T NOoDLE (Amiga)
    AnsiSlotFont {
        slot: 37,
        name: "P0T NOoDLE (Amiga)",
        f08: None,
        f14: Some(SLOT_37_F14),
        f16: Some(SLOT_37_F16),
    },
    // Slot 38: mOsOul (Amiga)
    AnsiSlotFont {
        slot: 38,
        name: "mOsOul (Amiga)",
        f08: None,
        f14: None,
        f16: Some(SLOT_38_F16),
    },
    // Slot 39: MicroKnight Plus (Amiga)
    AnsiSlotFont {
        slot: 39,
        name: "MicroKnight Plus (Amiga)",
        f08: None,
        f14: None,
        f16: Some(SLOT_39_F16),
    },
    // Slot 40: Topaz Plus (Amiga)
    AnsiSlotFont {
        slot: 40,
        name: "Topaz Plus (Amiga)",
        f08: None,
        f14: None,
        f16: Some(SLOT_40_F16),
    },
    // Slot 41: MicroKnight (Amiga)
    AnsiSlotFont {
        slot: 41,
        name: "MicroKnight (Amiga)",
        f08: None,
        f14: None,
        f16: Some(SLOT_41_F16),
    },
    // Slot 42: Topaz (Amiga)
    AnsiSlotFont {
        slot: 42,
        name: "Topaz (Amiga)",
        f08: None,
        f14: Some(SLOT_42_F14),
        f16: Some(SLOT_42_F16),
    },
];

/// Load an ANSI font from a slot with the specified height variant.
///
/// # Arguments
/// * `slot` - Font slot number (0-42)
/// * `font_height` - Desired font height (8, 14, or 16)
///
/// # Returns
/// * `Ok(&BitFont)` - Reference to the cached font
/// * `Err` - If the slot is invalid or no font variant is available
///
/// # Fallback behavior
/// If the exact height variant is not available, falls back to:
/// - f16 (preferred fallback)
/// - f14 (secondary fallback)  
/// - f08 (last resort)
///
/// # Caching
/// Fonts are cached after first load for performance.
pub fn get_ansi_font(slot: u8, font_height: u8) -> Option<&'static BitFont> {
    let slot = slot as usize;
    if slot >= ANSI_SLOT_COUNT {
        log::warn!("Requested ANSI font slot {} is out of range", slot);
        return None;
    }

    // Map height to cache index: 0=8px, 1=14px, 2=16px
    let height_index = match font_height {
        8 => 0,
        14 => 1,
        _ => 2,
    };

    // Try to get from cache
    if let Some(font) = FONT_CACHE[slot][height_index].get() {
        return Some(font);
    }

    // Load font
    let slot_font = &ANSI_SLOT_FONTS[slot];

    // Try to get the exact height variant, with fallbacks
    let font_data = match font_height {
        8 => slot_font.f08.or(slot_font.f14).or(slot_font.f16),
        14 => slot_font.f14.or(slot_font.f16).or(slot_font.f08),
        _ => slot_font.f16.or(slot_font.f14).or(slot_font.f08),
    };

    match font_data {
        Some(data) => {
            let Ok(font) = BitFont::from_bytes(slot_font.name, data) else {
                log::error!("Failed to parse font data for slot {} ('{}') with height {}", slot, slot_font.name, font_height);
                return None;
            };
            // Store in cache and return reference
            let _ = FONT_CACHE[slot][height_index].set(font);
            Some(FONT_CACHE[slot][height_index].get().unwrap())
        }
        None => {
            log::warn!("No font data available for slot {} ('{}') with height {}", slot, slot_font.name, font_height);
            None
        }
    }
}

/// Get the font height based on terminal line count.
///
/// # Arguments
/// * `lines` - Number of terminal lines
///
/// # Returns
/// * 8 for 50 lines (VGA 8x8)
/// * 14 for 28 lines (EGA 8x14)
/// * 16 for 25 lines or other (VGA 8x16)
pub fn font_height_for_lines(lines: usize) -> u8 {
    match lines {
        50 => 8,
        28 | 43 => 14, // 43 lines was also common for EGA
        _ => 16,       // Default to 16px (25 lines)
    }
}

/// Get all available font slot names
pub fn get_slot_font_names() -> Vec<&'static str> {
    ANSI_SLOT_FONTS.iter().map(|f| f.name).collect()
}

/// Find a slot by name (case-insensitive partial match)
pub fn find_slot_by_name(name: &str) -> Option<usize> {
    let name_lower = name.to_lowercase();
    ANSI_SLOT_FONTS.iter().find(|f| f.name.to_lowercase().contains(&name_lower)).map(|f| f.slot)
}
