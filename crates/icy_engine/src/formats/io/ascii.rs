//! ASCII format (.asc, .txt) I/O implementation.

use std::path::Path;

use crate::{EditableScreen, Position, Result, TextBuffer, TextPane, TextScreen};

use super::super::{LoadData, SaveOptions};

/// Load an ASCII file into a TextScreen.
pub(crate) fn load_ascii(file_name: &Path, data: &[u8], load_data_opt: Option<LoadData>) -> Result<TextScreen> {
    let load_data = load_data_opt.unwrap_or_default();
    let width = load_data.default_terminal_width.unwrap_or(80);
    let mut result = TextScreen::new((width, 25));
    result.terminal_state_mut().is_terminal_buffer = false;

    result.buffer.file_name = Some(file_name.into());
    let mut min_height = -1;
    if let Some(sauce) = &load_data.sauce_opt {
        let lines = result.apply_sauce(sauce);
        if lines.1 > 0 {
            min_height = lines.1 as i32;
        }
    }
    let (file_data, is_unicode) = crate::prepare_data_for_parsing(data);
    if is_unicode {
        result.buffer.buffer_type = crate::BufferType::Unicode;
    }
    crate::load_with_parser(&mut result, &mut icy_parser_core::AsciiParser::default(), file_data, true, min_height)?;
    Ok(result)
}

/// Save a TextBuffer to ASCII format.
pub(crate) fn save_ascii(buf: &TextBuffer, options: &SaveOptions) -> Result<Vec<u8>> {
    let mut result = Vec::new();
    let mut pos = Position::default();
    let height = buf.get_line_count();

    while pos.y < height {
        let line_length = buf.get_line_length(pos.y);
        while pos.x < line_length {
            let ch = buf.get_char(pos);
            if options.modern_terminal_output {
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

        if pos.x < buf.get_width() && pos.y + 1 < height {
            result.push(13);
            result.push(10);
        }

        pos.x = 0;
        pos.y += 1;
    }

    if let Some(sauce) = &options.save_sauce {
        sauce.write(&mut result)?;
    }
    Ok(result)
}

/// Check if SAUCE is required for saving (width != 80).
pub fn _get_save_sauce_default_asc(buf: &TextBuffer) -> (bool, String) {
    if buf.get_width() != 80 {
        return (true, "width != 80".to_string());
    }
    (false, String::new())
}
