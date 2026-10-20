// ========================================
// Amiga Workbench Fonts (Name + Size lookup)
// ========================================

use super::BitFont;
use crate::Size;
use libyaff::{Bitmap, GlyphDefinition, Label, YaffFont};
use parking_lot::Mutex;
use std::sync::Arc;

#[derive(Clone)]
pub(crate) struct AmigaGlyph {
    pub bitmap: Bitmap,
    pub left_bearing: i32,
    pub shift_up: i32,
    pub advance: i32,
}

pub(crate) struct AmigaFont {
    // Preserve the fixed-cell font API without clipping SkyPix's native glyphs.
    pub compact: BitFont,
    pub size: Size,
    glyphs: [AmigaGlyph; 256],
}

impl AmigaFont {
    fn from_yaff(font: &YaffFont, name: &str, height: i32) -> Self {
        let width = font.cell_size.or(font.bounding_box).map_or(8, |(width, _)| width as i32);
        let convert = |glyph: &GlyphDefinition| {
            let left_bearing = glyph.left_bearing.or(font.global_left_bearing).unwrap_or(0);
            let right_bearing = glyph.right_bearing.or(font.global_right_bearing).unwrap_or(0);
            AmigaGlyph {
                bitmap: glyph.bitmap.clone(),
                left_bearing,
                shift_up: glyph.shift_up.or(font.global_shift_up).unwrap_or(0),
                advance: left_bearing + glyph.bitmap.width as i32 + right_bearing,
            }
        };
        let fallback = font
            .glyphs
            .iter()
            .find(|glyph| glyph.labels.iter().any(|label| matches!(label, Label::Tag(tag) if tag == "default")))
            .map(&convert)
            .unwrap_or(AmigaGlyph {
                bitmap: Bitmap::default(),
                left_bearing: 0,
                shift_up: 0,
                advance: width,
            });
        let mut glyphs = std::array::from_fn(|_| fallback.clone());
        // Encoded Amiga bytes take precedence over Unicode aliases.
        for codepoints in [false, true] {
            for glyph in &font.glyphs {
                for label in &glyph.labels {
                    let codes: Vec<usize> = match label {
                        Label::Codepoint(codes) if codepoints => codes.iter().map(|&code| code as usize).collect(),
                        Label::Unicode(codes) if !codepoints => codes.iter().map(|&code| code as usize).collect(),
                        _ => continue,
                    };
                    for code in codes.into_iter().filter(|&code| code < 256) {
                        glyphs[code] = convert(glyph);
                    }
                }
            }
        }
        Self {
            compact: BitFont::from_yaff_font(font, name),
            size: Size::new(width, height),
            glyphs,
        }
    }

    pub fn glyph(&self, ch: char) -> &AmigaGlyph {
        &self.glyphs[(ch as usize).min(255)]
    }
}

macro_rules! amiga_fonts {
    ($( ($i:ident, $file:expr, $name: expr, $size:expr) ),* $(,)? ) => {
        $(
            pub const $i: &str = include_str!(concat!("../../data/fonts/Amiga/original/", $file));
        )*

        pub fn load_amiga_fonts() -> Vec<(String, i32, &'static str, Mutex<Option<BitFont>>)> {
            let mut fonts = Vec::new();
            $(
                fonts.push(($name.to_string(), $size, $i, Mutex::new(None)));
            )*
            fonts
        }

    }
}

lazy_static::lazy_static! {
    static ref AMIGA_FONTS: Vec<(String, i32, &'static str, Mutex<Option<Arc<AmigaFont>>>)> =
        load_amiga_fonts().into_iter().map(|(name, size, data, _)| (name, size, data, Mutex::new(None))).collect();
}

pub fn get_amiga_font_by_name(name: &str, size: i32) -> Option<BitFont> {
    get_amiga_font(name, size).map(|font| font.compact.clone())
}

pub(crate) fn get_amiga_font(name: &str, size: i32) -> Option<Arc<AmigaFont>> {
    for (font_name, font_size, font_data, opt_font) in AMIGA_FONTS.iter() {
        if font_name.eq_ignore_ascii_case(name) && size == *font_size {
            let mut font_cache = opt_font.lock();

            if let Some(cached_font) = font_cache.as_ref() {
                return Some(cached_font.clone());
            }

            match YaffFont::from_bytes(font_data.as_bytes()) {
                Ok(yaff) => {
                    let font = Arc::new(AmigaFont::from_yaff(&yaff, font_name, *font_size));
                    *font_cache = Some(font.clone());
                    return Some(font);
                }
                Err(err) => {
                    log::error!("Unable to load Amiga font '{font_name}' size {font_size}: {err}");
                    return None;
                }
            }
        }
    }
    None
}

amiga_fonts![
    (AMIGA_TOPAZ_08, "amiga-ks13-topaz-08.yaff", "Topaz.font", 8),
    (AMIGA_TOPAZ_09, "amiga-ks13-topaz-09.yaff", "Topaz.font", 9),
    (AMIGA_TOPAZ_11, "workbench-3.1/Topaz_8x11.yaff", "Topaz.font", 11),
    (AMIGA_DIAMOND_12, "workbench-3.1/Diamond_12.yaff", "Diamond.font", 12),
    (AMIGA_DIAMOND_20, "workbench-3.1/Diamond_20.yaff", "Diamond.font", 20),
    (AMIGA_EMERALD_17, "workbench-3.1/Emerald_17.yaff", "Emerald.font", 17),
    (AMIGA_EMERALD_20, "workbench-3.1/Emerald_20.yaff", "Emerald.font", 20),
    (AMIGA_PEARL_08, "pearl_08.yaff", "pearl.font", 8),
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
    (AMIGA_TIMES_30, "workbench-3.1/Times_30.yaff", "Times.font", 30),
    (AMIGA_TIMES_36, "workbench-3.1/Times_36.yaff", "Times.font", 36),
    (AMIGA_COURIER_11, "workbench-3.1/Courier_11.yaff", "Courier.font", 11),
    (AMIGA_COURIER_13, "workbench-3.1/Courier_13.yaff", "Courier.font", 13),
    (AMIGA_COURIER_15, "workbench-3.1/Courier_15.yaff", "Courier.font", 15),
    (AMIGA_COURIER_18, "workbench-3.1/Courier_18.yaff", "Courier.font", 18),
    (AMIGA_COURIER_24, "workbench-3.1/Courier_24.yaff", "Courier.font", 24),
    (AMIGA_COURIER_30, "workbench-3.1/Courier_30.yaff", "Courier.font", 30),
    (AMIGA_COURIER_36, "workbench-3.1/Courier_36.yaff", "Courier.font", 36),
];
