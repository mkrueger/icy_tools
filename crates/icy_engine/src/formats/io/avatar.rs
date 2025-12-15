use icy_parser_core::avatar_constants;

use crate::{EditableScreen, Position, Result, TagPlacement, TextAttribute, TextBuffer, TextPane, TextScreen};

use super::super::{AnsiSaveOptionsV2, LoadData};

pub(crate) fn save_avatar(buf: &TextBuffer, options: &AnsiSaveOptionsV2) -> Result<Vec<u8>> {
    if buf.palette.len() != 16 {
        return Err(crate::EngineError::Only16ColorPalettesSupported);
    }

    let mut result = Vec::new();
    let mut last_attr = TextAttribute::default();
    let mut pos = Position::default();
    let height = buf.line_count();
    let mut first_char = true;

    match options.screen_preparation {
        super::super::ScreenPreperation::None => {}
        super::super::ScreenPreperation::ClearScreen => {
            result.push(avatar_constants::CLEAR_SCREEN);
        }
        super::super::ScreenPreperation::Home => {
            result.push(avatar_constants::COMMAND);
            result.push(avatar_constants::GOTO_XY); // move caret
            result.push(1); // x
            result.push(1); // y
        }
    }

    // TODO: implement repeat pattern compression (however even TheDraw never bothered to implement this cool RLE from fsc0037)
    while pos.y < height {
        let line_length = buf.line_length(pos.y);

        while pos.x < line_length {
            let mut found_tag = false;
            for tag in &buf.tags {
                if tag.is_enabled && tag.tag_placement == TagPlacement::InText && tag.position.y == pos.y as i32 && tag.position.x == pos.x as i32 {
                    result.extend(tag.replacement_value.as_bytes());
                    pos.x += (tag.len() as i32).max(1);
                    found_tag = true;
                    break;
                }
            }
            if found_tag {
                continue;
            }

            let mut repeat_count = 1;
            let mut ch = buf.char_at(pos);

            while pos.x + 3 < buf.width() && ch == buf.char_at(pos + Position::new(1, 0)) {
                repeat_count += 1;
                pos.x += 1;
                ch = buf.char_at(pos);
            }

            if first_char || ch.attribute != last_attr {
                result.push(22);
                result.push(1);
                result.push(ch.attribute.as_u8(buf.ice_mode));
                last_attr = ch.attribute;
            }
            first_char = false;

            if repeat_count > 1 {
                if repeat_count < 4 && (ch.ch != '\x16' && ch.ch != '\x0C' && ch.ch != '\x19') {
                    result.resize(result.len() + repeat_count, ch.ch as u8);
                } else {
                    result.push(25);
                    result.push(ch.ch as u8);
                    result.push(repeat_count as u8);
                }
                pos.x += 1;

                continue;
            }

            // avt control codes need to be represented as repeat once.
            if ch.ch == '\x16' || ch.ch == '\x0C' || ch.ch == '\x19' {
                result.push(25);
                result.push(ch.ch as u8);
                result.push(1);
            } else {
                result.push(if ch.ch == '\0' { b' ' } else { ch.ch as u8 });
            }
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

            result.push(avatar_constants::COMMAND);
            result.push(avatar_constants::GOTO_XY); // move caret
            result.push(tag.position.x as u8 + 1); // x
            result.push(tag.position.y as u8 + 1); // y
            result.extend(tag.replacement_value.as_bytes());
        }
    }

    if end_tags > 0 {
        result.extend_from_slice(b"\x1b[u");
    }

    if let Some(sauce) = &options.save_sauce {
        sauce.write(&mut result)?;
    }
    Ok(result)
}

pub(crate) fn load_avatar(data: &[u8], load_data_opt: Option<LoadData>) -> Result<TextScreen> {
    let load_data = load_data_opt.unwrap_or_default();
    let width = load_data.default_terminal_width.unwrap_or(80);
    let mut result = TextScreen::new((width, 25));
    result.terminal_state_mut().is_terminal_buffer = false;

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
    crate::load_with_parser(&mut result, &mut icy_parser_core::AvatarParser::default(), file_data, true, min_height)?;
    Ok(result)
}

pub fn _get_save_sauce_default_avt(buf: &TextBuffer) -> (bool, String) {
    if buf.width() != 80 {
        return (true, "width != 80".to_string());
    }

    (false, String::new())
}
