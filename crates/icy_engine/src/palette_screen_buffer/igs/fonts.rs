use crate::BitFont;

lazy_static::lazy_static! {
    pub static ref ATARI_ST_FONT_6x6: BitFont = BitFont::from_bytes("Atari ST 6x6", include_bytes!("../../../data/fonts/Atari/atari-st-6x6.yaff")).unwrap();
    pub static ref ATARI_ST_FONT_8x8: BitFont = BitFont::from_bytes("Atari ST 8x8", include_bytes!("../../../data/fonts/Atari/atari-st-8x8.yaff")).unwrap();

    pub static ref ATARI_ST_FONT_12x12: BitFont = {
        ATARI_ST_FONT_6x6.double_size()
    };

    pub static ref ATARI_ST_FONT_16x16: BitFont = {
        ATARI_ST_FONT_8x8.double_size()
    };

    pub static ref ATARI_ST_FONT_16x32: BitFont = {
        ATARI_ST_FONT_8x16.double_size()
    };

    pub static ref ATARI_ST_FONT_8x16: BitFont = BitFont::from_bytes("Atari ST 8x8", include_bytes!("../../../data/fonts/Atari/atari-st-8x16.yaff")).unwrap();

}

pub struct FontMetrics {
    pub y_off: i32,
    pub underline_pos: i32,
    pub underline_height: i32,
    pub underline_width: i32,
}

pub fn load_atari_font(text_size: i32) -> (FontMetrics, &'static BitFont) {
    if text_size <= 8 {
        return (
            FontMetrics {
                y_off: 4,
                underline_pos: 5,
                underline_height: 1,
                underline_width: 7,
            },
            &ATARI_ST_FONT_6x6,
        );
    }
    if text_size == 9 {
        return (
            FontMetrics {
                y_off: 6,
                underline_pos: 7,
                underline_height: 1,
                underline_width: 9,
            },
            &ATARI_ST_FONT_8x8,
        );
    }

    if text_size <= 15 {
        // 7x11 Font
        return (
            FontMetrics {
                y_off: 13,
                underline_pos: 15,
                underline_height: 1,
                underline_width: 9,
            },
            &ATARI_ST_FONT_8x16,
        );
    }

    if text_size <= 17 {
        // 12x12 Font (upscaled 6x6)
        return (
            FontMetrics {
                y_off: 9,
                underline_pos: 10,
                underline_height: 2,
                underline_width: 13,
            },
            &ATARI_ST_FONT_12x12,
        );
    }

    if text_size <= 19 {
        // 16x16 Font (upscaled 8x8)
        return (
            FontMetrics {
                y_off: 13,
                underline_pos: 14,
                underline_height: 2,
                underline_width: 17,
            },
            &ATARI_ST_FONT_16x16,
        );
    }

    (
        FontMetrics {
            y_off: 27,
            underline_pos: 30,
            underline_height: 2,
            underline_width: 17,
        },
        &ATARI_ST_FONT_16x32,
    )
}
