use std::{collections::HashMap, sync::Arc};

use parking_lot::Mutex;

use crate::{BitFont, BitFontType};

lazy_static::lazy_static! {

    pub static ref FONT : BitFont = BitFont::from_sauce_name("IBM VGA50").unwrap();
    pub static ref EGA_7x8: BitFont = BitFont::from_bytes("EGA 7x8", include_bytes!("../../data/fonts/Rip/Bm437_EverexME_7x8.yaff")).unwrap();
    pub static ref VGA_8x14: BitFont = BitFont::from_bytes("VGA 8x14", include_bytes!("../../data/fonts/Rip/IBM_VGA_8x14.yaff")).unwrap();

    pub static ref VGA_7x14: BitFont = {
        let mut new_font = VGA_8x14.yaff_font.clone();
        new_font.name = Some("VGA 7x14".to_string());
        new_font.bounding_box = Some((7, 14));
        new_font.cell_size = Some((7, 14));
        new_font.line_height = Some(14);
        let glyph_lookup = BitFont::build_glyph_lookup(&new_font);

        BitFont {
            yaff_font: new_font,
            glyph_cache: Mutex::new(HashMap::new()),
            glyph_lookup: Arc::new(glyph_lookup),
            path_opt: None,
            font_type: BitFontType::BuiltIn,
        }
    };

    pub static ref VGA_16x14: BitFont = {
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
                        new_row.push(px);
                    }
                    new_row
                })
                .collect();
            glyph.bitmap.width = 16;
            glyph.bitmap.pixels = new_pixels;
        }
        let glyph_lookup = BitFont::build_glyph_lookup(&new_font);

        BitFont {
            yaff_font: new_font,
            glyph_cache: Mutex::new(HashMap::new()),
            glyph_lookup: Arc::new(glyph_lookup),
            path_opt: None,
            font_type: BitFontType::BuiltIn,
        }
    };
}
