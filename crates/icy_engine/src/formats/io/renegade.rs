use crate::{EditableScreen, Position, Result, TextAttribute, TextBuffer, TextPane, TextScreen};

use super::super::{AnsiSaveOptionsV2, LoadData};

pub(crate) fn save_renegade(buf: &TextBuffer, options: &AnsiSaveOptionsV2) -> Result<Vec<u8>> {
    let _ = options;
    if buf.palette.len() != 16 {
        return Err(crate::EngineError::Only16ColorPalettesSupported);
    }
    let mut result = Vec::new();
    let mut last_attr = TextAttribute::default();
    let mut pos = Position::default();
    let height = buf.line_count();

    while pos.y < height {
        let line_length = buf.line_length(pos.y);
        while pos.x < line_length {
            let ch = buf.char_at(pos);
            if ch.attribute != last_attr {
                let last_fore = last_attr.foreground();
                let last_back = last_attr.background();
                if ch.attribute.foreground() != last_fore {
                    result.extend(format!("|{:02}", ch.attribute.foreground()).as_bytes());
                }
                if ch.attribute.background() != last_back {
                    result.extend(format!("|{:02}", 16 + ch.attribute.background()).as_bytes());
                }
                last_attr = ch.attribute;
            }
            result.push(if ch.ch == '\0' { b' ' } else { ch.ch as u8 });
            pos.x += 1;
        }

        // do not end with eol
        if pos.x < buf.width() && pos.y + 1 < height {
            result.push(13);
            result.push(10);
        }

        pos.x = 0;
        pos.y += 1;
    }
    Ok(result)
}

/// Note: SAUCE is applied externally by FileFormat::from_bytes().
pub(crate) fn load_renegade(data: &[u8], load_data_opt: Option<&LoadData>) -> Result<TextScreen> {
    let width = load_data_opt.and_then(|ld| ld.default_terminal_width()).unwrap_or(80);
    let mut result = TextScreen::new((width, 25));

    result.terminal_state_mut().is_terminal_buffer = false;

    let (file_data, is_unicode) = crate::prepare_data_for_parsing(data);
    if is_unicode {
        result.buffer.buffer_type = crate::BufferType::Unicode;
    }
    crate::load_with_parser(&mut result, &mut icy_parser_core::RenegadeParser::default(), file_data, true, -1)?;
    Ok(result)
}
