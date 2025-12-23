use super::super::{apply_sauce_to_buffer, LoadData, SaveOptions};
use crate::{Palette, Position, Result, TextBuffer, TextPane, TextScreen, ATARI, ATARI_DEFAULT_PALETTE};

#[allow(unused)]
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
            let mut ch = attr_ch.ch as u8;
            if attr_ch.attribute.background() > 0 {
                ch += 0x80;
            }

            // escape control chars
            if ch == b'\x1B' || ch == b'\x1C' || ch == b'\x1D' || ch == b'\x1E' || ch == b'\x1F' || ch == b'\x7D' || ch == b'\x7E' || ch == b'\x7F' {
                result.push(b'\x1B');
            }

            result.push(ch);
            pos.x += 1;
        }

        // do not end with eol
        if pos.x < buf.width() && pos.y + 1 < height {
            result.push(155);
        }

        pos.x = 0;
        pos.y += 1;
    }

    Ok(result)
}

pub(crate) fn load_atascii(data: &[u8], _load_data_opt: Option<&LoadData>, sauce_opt: Option<&icy_sauce::SauceRecord>) -> Result<TextScreen> {
    let mut result = TextScreen::new((40, 24));

    result.buffer.clear_font_table();
    let font = ATARI.clone();
    result.buffer.set_font(0, font);
    result.buffer.palette = Palette::from_slice(&ATARI_DEFAULT_PALETTE);

    result.buffer.buffer_type = crate::BufferType::Atascii;
    result.buffer.terminal_state.is_terminal_buffer = false;

    // Apply SAUCE settings
    if let Some(sauce) = sauce_opt {
        apply_sauce_to_buffer(&mut result.buffer, sauce);
    }

    crate::load_with_parser(&mut result, &mut icy_parser_core::AtasciiParser::default(), data, true, 24)?;
    Ok(result)
}
