use super::CompactGlyph;
use crate::{BitFont, BitFontType};

lazy_static::lazy_static! {
    pub static ref FONT: BitFont = BitFont::from_sauce_name("IBM VGA50").unwrap();
    pub static ref EGA_7x8: BitFont = BitFont::from_bytes("EGA 7x8", include_bytes!("../../data/fonts/Rip/Bm437_EverexME_7x8.psf")).unwrap();
    pub static ref VGA_8x14: BitFont = BitFont::from_bytes("VGA 8x14", include_bytes!("../../data/fonts/Rip/IBM_VGA_8x14.psf")).unwrap();

    /// 7x14 variant: same glyphs but with width=7
    pub static ref VGA_7x14: BitFont = {
        let base = &*VGA_8x14;
        let glyphs: [CompactGlyph; 256] = std::array::from_fn(|i| {
            let mut g = base.glyphs()[i];
            g.width = 7;
            g
        });
        BitFont {
            name: "VGA 7x14".to_string(),
            width: 7,
            height: 14,
            glyphs,
            path_opt: None,
            font_type: BitFontType::BuiltIn,
        }
    };

    /// 16x14 variant: double width (each pixel repeated horizontally)
    /// Note: 16px width exceeds CompactGlyph max of 8px, so this returns an 8x14 font
    pub static ref VGA_16x14: BitFont = {
        let base = &*VGA_8x14;
        BitFont {
            name: "VGA 16x14".to_string(),
            width: 8,
            height: 14,
            glyphs: *base.glyphs(),
            path_opt: None,
            font_type: BitFontType::BuiltIn,
        }
    };
}
