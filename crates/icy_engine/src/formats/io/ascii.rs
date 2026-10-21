//! ASCII format (.asc, .txt) I/O implementation.
use crate::{EditableScreen, Position, Result, TextBuffer, TextPane, TextScreen};

use super::super::{append_sauce, apply_sauce_to_buffer, LoadData, SauceBuilder, SaveOptions};
use icy_sauce::CharacterFormat;

/// Load an ASCII file into a `TextScreen`.
pub(crate) fn load_ascii(data: &[u8], load_data_opt: Option<&LoadData>, sauce_opt: Option<&icy_sauce::SauceRecord>) -> Result<TextScreen> {
    let width = load_data_opt.and_then(super::super::LoadData::default_terminal_width).unwrap_or(80);
    let mut result = TextScreen::new((width, 25));
    result.terminal_state_mut().is_terminal_buffer = false;

    // Apply SAUCE settings early to get correct dimensions
    if let Some(sauce) = sauce_opt {
        apply_sauce_to_buffer(&mut result.buffer, sauce);
    }

    let (file_data, is_unicode) = crate::prepare_data_for_parsing(data);
    if is_unicode || load_data_opt.is_some_and(LoadData::convert_to_utf8) {
        std::str::from_utf8(file_data).map_err(|error| crate::EngineError::Generic(format!("Invalid UTF-8 input: {error}")))?;
        result.buffer.buffer_type = crate::BufferType::Unicode;
        result.set_unicode_width(true);
    }
    crate::load_with_parser(&mut result, &mut icy_parser_core::AsciiParser::default(), file_data, true, -1)?;
    Ok(result)
}

/// Save a `TextBuffer` to ASCII format.
pub(crate) fn save_ascii(buf: &TextBuffer, options: &SaveOptions) -> Result<Vec<u8>> {
    let unicode = options.character_options().unicode || buf.buffer_type == crate::BufferType::Unicode;
    let mut result = if unicode { vec![0xEF, 0xBB, 0xBF] } else { Vec::new() };
    let mut pos = Position::default();
    let height = buf.line_count();

    while pos.y < height {
        let line_length = buf.line_length(pos.y);
        while pos.x < line_length {
            if unicode && buf.is_grapheme_continuation(pos) {
                pos.x += 1;
                continue;
            }
            let ch = buf.char_at(pos);
            if unicode {
                if let Some((text, width)) = buf.grapheme_at(pos) {
                    result.extend_from_slice(text.as_bytes());
                    pos.x += width as i32;
                    continue;
                }
                let char_to_write = if ch.ch == '\0' { ' ' } else { ch.ch };
                let uni = buf.buffer_type.convert_to_unicode(char_to_write);
                for byte in uni.to_string().as_bytes() {
                    result.push(*byte);
                }
            } else {
                result.push(if ch.ch == '\0' { b' ' } else { ch.ch as u8 });
            }
            pos.x += 1;
        }

        if pos.x < buf.width() && pos.y + 1 < height {
            result.push(13);
            result.push(10);
        }

        pos.x = 0;
        pos.y += 1;
    }

    if let Some(meta) = &options.sauce {
        let sauce = buf.build_character_sauce(meta, CharacterFormat::Ascii);
        append_sauce(&mut result, sauce)?;
    }
    Ok(result)
}

/// Check if SAUCE is required for saving (width != 80).
pub fn _get_save_sauce_default_asc(buf: &TextBuffer) -> (bool, String) {
    if buf.width() != 80 {
        return (true, "width != 80".to_string());
    }
    (false, String::new())
}
