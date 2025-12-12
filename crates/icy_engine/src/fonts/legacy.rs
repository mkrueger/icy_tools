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

    // Atari font
    pub static ref ATARI: BitFont = BitFont::from_bytes("Atari ATASCII", include_bytes!("../../data/fonts/Atari/Atari_ATASCII.psf")).unwrap();
}
