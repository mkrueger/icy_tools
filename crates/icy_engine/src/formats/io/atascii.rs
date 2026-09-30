use super::super::{apply_sauce_to_buffer, LoadData, SaveOptions};
use crate::{Palette, Position, Result, Size, TextBuffer, TextPane, TextScreen, ATARI, ATARI_DEFAULT_PALETTE, ATARI_XEP80, ATARI_XEP80_PALETTE};

/// Codes the Atari screen editor executes instead of printing; as characters they are sent
/// after an ESC. The inverse codes (bit 7) are controls as well, the end of line among them.
fn is_control(ch: u8) -> bool {
    matches!(ch, 0x1B..=0x1F | 0x7D..=0x7F | 0x9B..=0x9F | 0xFD..=0xFF)
}

/// An empty ATASCII screen with the Atari font and colors: the 40 column ANTIC text mode, or
/// the XEP80's 80 columns. Inverse characters are the upper half of the character codes.
pub fn atascii_buffer(columns: i32, height: i32) -> TextBuffer {
    let mut buffer = TextBuffer::new(Size::new(columns, height));
    set_atascii_screen(&mut buffer);
    buffer
}

/// Gives `buffer` the Atari font, colors and character set of its width: the XEP80's beyond
/// 40 columns.
fn set_atascii_screen(buffer: &mut TextBuffer) {
    let xep80 = buffer.width() > 40;
    buffer.clear_font_table();
    let font = if xep80 { ATARI_XEP80.clone() } else { ATARI.clone() };
    buffer.set_font_dimensions(font.size());
    buffer.set_font(0, font);
    buffer.palette = Palette::from_slice(if xep80 { &ATARI_XEP80_PALETTE } else { &ATARI_DEFAULT_PALETTE });
    buffer.buffer_type = crate::BufferType::Atascii;
    buffer.terminal_state.is_terminal_buffer = false;
}

pub(crate) fn save_atascii(buf: &TextBuffer, _options: &SaveOptions) -> Result<Vec<u8>> {
    if buf.buffer_type != crate::BufferType::Atascii {
        return Err(crate::EngineError::BufferTypeMismatch {
            expected: "Atascii".to_string(),
        });
    }

    let mut result = Vec::new();
    let mut pos = Position::default();
    let height = buf.line_count();

    while pos.y < height {
        let line_length = buf.line_length(pos.y);
        while pos.x < line_length {
            let attr_ch = buf.char_at(pos);
            let mut ch = attr_ch.ch as u32 as u8;
            // Inverse is part of the character; a background color asks for it as well.
            if attr_ch.attribute.background() > 0 {
                ch |= 0x80;
            }
            if is_control(ch) {
                result.push(0x1B);
            }
            result.push(ch);
            pos.x += 1;
        }

        // A full line wraps by itself, so only shorter lines end with an EOL.
        if pos.x < buf.width() && pos.y + 1 < height {
            result.push(0x9B);
        }

        pos.x = 0;
        pos.y += 1;
    }

    Ok(result)
}

/// Loads ATASCII; a `default_terminal_width` of 80 in `load_data_opt` or 80 SAUCE columns load
/// XEP80 text (.xep).
pub(crate) fn load_atascii(data: &[u8], load_data_opt: Option<&LoadData>, sauce_opt: Option<&icy_sauce::SauceRecord>) -> Result<TextScreen> {
    let columns = if load_data_opt.and_then(LoadData::default_terminal_width) == Some(80) {
        80
    } else {
        40
    };
    let mut result = TextScreen::from_buffer(atascii_buffer(columns, 24));

    if let Some(sauce) = sauce_opt {
        apply_sauce_to_buffer(&mut result.buffer, sauce);
        // SAUCE may name a DOS font; its width picks the ANTIC or the XEP80 screen.
        set_atascii_screen(&mut result.buffer);
    }
    // 0x1A is a line drawing character in ATASCII, so only the EOF marker in front of a SAUCE
    // record is dropped.
    let data = match sauce_opt {
        Some(_) => data.strip_suffix(&[0x1A]).unwrap_or(data),
        None => data,
    };

    crate::load_with_parser(&mut result, &mut icy_parser_core::AtasciiParser::default(), data, true, 24)?;
    Ok(result)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{AttributedChar, FileFormat, SauceBuilder, TextAttribute};

    fn load(bytes: &[u8], columns: usize) -> TextBuffer {
        FileFormat::Atascii
            .from_bytes(bytes, Some(LoadData::new(None, Some(columns))))
            .unwrap()
            .screen
            .buffer
    }

    #[test]
    fn every_character_code_survives_saving_and_loading() {
        for columns in [40, 80] {
            let mut buffer = atascii_buffer(columns, 256 / columns + 2);
            for code in 0..256 {
                let position = Position::new(code % columns, code / columns);
                buffer.layers[0].set_char(position, AttributedChar::new(char::from(code as u8), TextAttribute::default()));
            }
            // A short line after the full ones ends with an EOL.
            let short = 256 / columns + 1;
            buffer.layers[0].set_char((0, short), AttributedChar::new('A', TextAttribute::default()));
            // Blank glyphs such as the XEP80's EOL are saved as spaces unless saved losslessly.
            let mut options = SaveOptions::default();
            options.preprocess.optimize_colors = false;
            let bytes = FileFormat::Atascii.to_bytes(&buffer, &options).unwrap();
            let loaded = load(&bytes, columns as usize);
            assert_eq!(loaded.width(), columns);
            for code in 0..256 {
                let position = Position::new(code % columns, code / columns);
                assert_eq!(loaded.char_at(position).ch as u32, code as u32, "{columns} columns, code {code:#04X}");
            }
            assert_eq!(loaded.char_at((0, short).into()).ch, 'A');
        }
    }

    #[test]
    fn inverse_is_kept_in_the_character() {
        let mut buffer = atascii_buffer(40, 2);
        let mut attribute = TextAttribute::default();
        attribute.set_background(1);
        buffer.layers[0].set_char((0, 0), AttributedChar::new('A', attribute));
        buffer.layers[0].set_char((1, 0), AttributedChar::new(char::from(0xC1), attribute));
        let bytes = FileFormat::Atascii.to_bytes(&buffer, &SaveOptions::default()).unwrap();
        assert_eq!(&bytes[..2], &[0xC1, 0xC1]);
    }

    #[test]
    fn eof_character_is_a_glyph_unless_it_precedes_sauce() {
        let loaded = load(&[0x1A, 0x1A, b'A'], 40);
        assert_eq!([0, 1, 2].map(|x| loaded.char_at((x, 0).into()).ch as u32), [0x1A, 0x1A, u32::from(b'A')]);

        let mut bytes = vec![0x1A, b'A'];
        let sauce = atascii_buffer(80, 1).build_character_sauce(&crate::SauceMetaData::default(), icy_sauce::CharacterFormat::Ascii);
        super::super::super::append_sauce(&mut bytes, sauce).unwrap();
        let loaded = load(&bytes, 40);
        assert_eq!(loaded.char_at((0, 0).into()).ch as u32, 0x1A);
        assert_eq!(loaded.char_at((1, 0).into()).ch, 'A');
        assert_eq!(loaded.char_at((2, 0).into()).ch, ' ', "the EOF marker in front of SAUCE is not shown");
        assert_eq!(loaded.width(), 80, "80 SAUCE columns load the XEP80 screen");
        assert_eq!(loaded.font(0).unwrap().name(), ATARI_XEP80.name());
    }

    #[test]
    fn the_screens_use_their_fonts_cell_size() {
        let antic = atascii_buffer(40, 24);
        assert_eq!(antic.font_dimensions(), Size::new(8, 8));
        let xep80 = atascii_buffer(80, 25);
        assert_eq!(xep80.font_dimensions(), ATARI_XEP80.size());
        assert_eq!(xep80.buffer_type, crate::BufferType::Atascii);
    }
}
