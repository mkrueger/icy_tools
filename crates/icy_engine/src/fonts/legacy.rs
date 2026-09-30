// ========================================
// Lazy Static Fonts for Special Use Cases
// ========================================

use super::BitFont;

lazy_static::lazy_static! {
    // Atari XEP80 fonts
    pub static ref ATARI_XEP80: BitFont = BitFont::from_bytes("Atari XEP80", include_bytes!("../../data/fonts/Atari/xep80.psf")).unwrap();
    pub static ref ATARI_XEP80_INT: BitFont = BitFont::from_bytes("Atari XEP80 INT", include_bytes!("../../data/fonts/Atari/xep80_int.psf")).unwrap();

    // Viewdata/Teletext font
    pub static ref VIEWDATA: BitFont = BitFont::from_bytes("Viewdata", include_bytes!("../../data/fonts/Viewdata/saa5050.psf")).unwrap();

    // C64 fonts
    pub static ref C64_UNSHIFTED: BitFont = BitFont::from_bytes("C64 PETSCII unshifted", include_bytes!("../../data/fonts/Commodore/C64_PETSCII_unshifted.psf")).unwrap();
    pub static ref C64_SHIFTED: BitFont = BitFont::from_bytes("C64 PETSCII shifted", include_bytes!("../../data/fonts/Commodore/C64_PETSCII_shifted.psf")).unwrap();

    // Character ROMs of the other Commodore machines; "upper" is upper case with graphics,
    // "lower" lower and upper case. The C128's 40 column upper case set is the C64's.
    pub static ref C128_LOWER: BitFont = BitFont::from_bytes("C128 PETSCII lower", include_bytes!("../../data/fonts/Commodore/C128_PETSCII_lower.psf")).unwrap();
    pub static ref C16_UPPER: BitFont = BitFont::from_bytes("C16 PETSCII upper", include_bytes!("../../data/fonts/Commodore/C16_PETSCII_upper.psf")).unwrap();
    pub static ref C16_LOWER: BitFont = BitFont::from_bytes("C16 PETSCII lower", include_bytes!("../../data/fonts/Commodore/C16_PETSCII_lower.psf")).unwrap();
    pub static ref VIC20_UPPER: BitFont = BitFont::from_bytes("VIC-20 PETSCII upper", include_bytes!("../../data/fonts/Commodore/VIC20_PETSCII_upper.psf")).unwrap();
    pub static ref VIC20_LOWER: BitFont = BitFont::from_bytes("VIC-20 PETSCII lower", include_bytes!("../../data/fonts/Commodore/VIC20_PETSCII_lower.psf")).unwrap();
    pub static ref PET_UPPER: BitFont = BitFont::from_bytes("PET PETSCII upper", include_bytes!("../../data/fonts/Commodore/PET_PETSCII_upper.psf")).unwrap();
    pub static ref PET_LOWER: BitFont = BitFont::from_bytes("PET PETSCII lower", include_bytes!("../../data/fonts/Commodore/PET_PETSCII_lower.psf")).unwrap();

    // Atari font
    pub static ref ATARI: BitFont = BitFont::from_bytes("Atari ATASCII", include_bytes!("../../data/fonts/Atari/Atari_ATASCII.psf")).unwrap();
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::Size;

    #[test]
    fn commodore_character_roms_load_as_8x8_fonts() {
        let fonts = [
            &*C64_UNSHIFTED,
            &*C64_SHIFTED,
            &*C128_LOWER,
            &*C16_UPPER,
            &*C16_LOWER,
            &*VIC20_UPPER,
            &*VIC20_LOWER,
            &*PET_UPPER,
            &*PET_LOWER,
        ];
        for font in fonts {
            assert_eq!(font.size(), Size::new(8, 8), "{}", font.name());
            // Screen code 0x81 is the reverse of 0x01 on every machine.
            let (normal, reverse) = (font.glyph('\u{01}'), font.glyph('\u{81}'));
            assert!(normal.data[..8].iter().zip(&reverse.data[..8]).all(|(a, b)| *a == !b), "{}", font.name());
        }
        // Screen code 1 is "a" in the lower case sets and "A" in the upper case ones.
        assert_ne!(C128_LOWER.glyph('\u{01}').data, C16_UPPER.glyph('\u{01}').data);
        assert_ne!(
            C128_LOWER.glyph('\u{02}').data,
            C64_UNSHIFTED.glyph('\u{02}').data,
            "the C128 has its own lower case"
        );
    }
}
