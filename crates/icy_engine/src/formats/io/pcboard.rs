use crate::{EditableScreen, Position, Result, TagPlacement, TextAttribute, TextBuffer, TextPane, TextScreen};

use super::super::{LoadData, SauceBuilder, SaveOptions, ScreenPreperation};
use icy_sauce::CharacterFormat;

pub(crate) fn save_pcboard(buf: &TextBuffer, options: &SaveOptions) -> Result<Vec<u8>> {
    if buf.palette.len() != 16 {
        return Err(crate::EngineError::Only16ColorPalettesSupported);
    }
    let mut result: Vec<u8> = Vec::new();
    let mut last_attr = TextAttribute::default();
    let mut pos = Position::default();
    let height = buf.line_count();
    let mut first_char = true;
    let char_opts = options.character_options();
    if char_opts.unicode {
        // write UTF-8 BOM as unicode indicator.
        result.extend([0xEF, 0xBB, 0xBF]);
    }

    match char_opts.screen_prep {
        ScreenPreperation::None | ScreenPreperation::Home => {} // home not supported
        ScreenPreperation::ClearScreen => {
            result.extend(b"@CLS@");
        }
    }
    while pos.y < height {
        let line_length = buf.line_length(pos.y);

        while pos.x < line_length {
            let mut found_tag = false;
            for tag in &buf.tags {
                if tag.is_enabled && tag.tag_placement == TagPlacement::InText && tag.position.y == pos.y as i32 && tag.position.x == pos.x as i32 {
                    if first_char || tag.attribute != last_attr {
                        result.extend_from_slice(format!("@X{:02X}", tag.attribute.as_u8(crate::IceMode::Blink)).as_bytes());
                        last_attr = tag.attribute;
                    }

                    result.extend(tag.replacement_value.as_bytes());
                    pos.x += (tag.len() as i32).max(1);
                    found_tag = true;
                    break;
                }
            }
            if found_tag {
                continue;
            }

            let ch = buf.char_at(pos);

            if first_char || ch.attribute != last_attr {
                result.extend_from_slice(format!("@X{:02X}", ch.attribute.as_u8(crate::IceMode::Blink)).as_bytes());
                last_attr = ch.attribute;
            }

            if char_opts.unicode {
                if ch.ch == '\0' {
                    result.push(b' ')
                } else {
                    let uni_ch = buf.buffer_type.convert_to_unicode(ch.ch);
                    result.extend(uni_ch.to_string().as_bytes().to_vec());
                }
            } else {
                result.push(if ch.ch == '\0' { b' ' } else { ch.ch as u8 });
            }
            first_char = false;
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
    let mut end_tags = 0;

    for tag in &buf.tags {
        if tag.is_enabled && tag.tag_placement == crate::TagPlacement::WithGotoXY {
            if end_tags == 0 {
                result.extend_from_slice(b"\x1b[s");
            }
            end_tags += 1;

            if first_char || tag.attribute != last_attr {
                result.extend_from_slice(format!("@X{:02X}", tag.attribute.as_u8(crate::IceMode::Blink)).as_bytes());
                last_attr = tag.attribute;
            }

            result.extend(format!("\x1B[{};{}H", tag.position.y + 1, tag.position.x + 1).as_bytes());
            result.extend(tag.replacement_value.as_bytes());
        }
    }

    if end_tags > 0 {
        result.extend_from_slice(b"\x1b[u");
    }

    if let Some(meta) = &options.sauce {
        let sauce = buf.build_character_sauce(meta, CharacterFormat::PCBoard);
        sauce.write(&mut result)?;
    }
    Ok(result)
}

/// Note: SAUCE is applied externally by FileFormat::from_bytes().
pub(crate) fn load_pcboard(data: &[u8], load_data_opt: Option<&LoadData>) -> Result<TextScreen> {
    let width = load_data_opt.and_then(|ld| ld.default_terminal_width()).unwrap_or(80);
    let mut result = TextScreen::new((width, 25));

    result.terminal_state_mut().is_terminal_buffer = false;

    let (file_data, is_unicode) = crate::prepare_data_for_parsing(data);
    if is_unicode {
        result.buffer.buffer_type = crate::BufferType::Unicode;
    }
    crate::load_with_parser(&mut result, &mut icy_parser_core::PcBoardParser::default(), file_data, true, -1)?;
    Ok(result)
}

pub fn _get_save_sauce_default_pcb(buf: &TextBuffer) -> (bool, String) {
    if buf.width() != 80 {
        return (true, "width != 80".to_string());
    }

    (false, String::new())
}
