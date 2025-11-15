use std::path::Path;

use icy_parser_core::AvatarParser as avt;

use crate::{
    BufferFeatures, EditableScreen, EngineResult, OutputFormat, Position, TagPlacement, TextAttribute, TextBuffer, TextPane, TextScreen, parse_with_parser,
};

use super::{LoadData, SaveOptions};

pub enum AvtReadState {
    Chars,
    RepeatChars,
    ReadCommand,
    MoveCursor,
    ReadColor,
}

#[derive(Default)]
pub(super) struct Avatar {}

impl OutputFormat for Avatar {
    fn get_file_extension(&self) -> &str {
        "avt"
    }

    fn get_name(&self) -> &str {
        "Avatar"
    }

    fn analyze_features(&self, _features: &BufferFeatures) -> String {
        String::new()
    }

    fn to_bytes(&self, buf: &mut crate::TextBuffer, options: &SaveOptions) -> EngineResult<Vec<u8>> {
        if buf.palette.len() != 16 {
            return Err(anyhow::anyhow!("Only 16 color palettes are supported by this format."));
        }

        let mut result = Vec::new();
        let mut last_attr = TextAttribute::default();
        let mut pos = Position::default();
        let height = buf.get_line_count();
        let mut first_char = true;

        match options.screen_preparation {
            super::ScreenPreperation::None => {}
            super::ScreenPreperation::ClearScreen => {
                result.push(avt::constants::CLEAR_SCREEN);
            }
            super::ScreenPreperation::Home => {
                result.push(avt::constants::COMMAND);
                result.push(avt::constants::GOTO_XY); // move caret
                result.push(1); // x
                result.push(1); // y
            }
        }

        // TODO: implement repeat pattern compression (however even TheDraw never bothered to implement this cool RLE from fsc0037)
        while pos.y < height {
            let line_length = buf.get_line_length(pos.y);

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
                let mut ch = buf.get_char(pos);

                while pos.x + 3 < buf.get_width() && ch == buf.get_char(pos + Position::new(1, 0)) {
                    repeat_count += 1;
                    pos.x += 1;
                    ch = buf.get_char(pos);
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
            if pos.x < buf.get_width() && pos.y + 1 < height {
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

                result.push(avt::constants::COMMAND);
                result.push(avt::constants::GOTO_XY); // move caret
                result.push(tag.position.x as u8 + 1); // x
                result.push(tag.position.y as u8 + 1); // y
                result.extend(tag.replacement_value.as_bytes());
            }
        }

        if end_tags > 0 {
            result.extend_from_slice(b"\x1b[u");
        }

        if options.save_sauce {
            buf.write_sauce_info(icy_sauce::SauceDataType::Character, icy_sauce::CharacterFormat::Avatar, &mut result)?;
        }
        Ok(result)
    }

    fn load_buffer(&self, file_name: &Path, data: &[u8], load_data_opt: Option<LoadData>) -> EngineResult<crate::TextBuffer> {
        let load_data = load_data_opt.unwrap_or_default();
        let width = load_data.default_terminal_width.unwrap_or(80);
        let mut result = TextScreen::new((width, 25));
        result.terminal_state_mut().is_terminal_buffer = false;

        result.buffer.file_name = Some(file_name.into());
        if let Some(sauce) = load_data.sauce_opt {
            result.buffer.load_sauce(sauce);
        }
        let (text, is_unicode) = crate::convert_ansi_to_utf8(data);
        if is_unicode {
            result.buffer.buffer_type = crate::BufferType::Unicode;
        }
        parse_with_parser(&mut result, &mut crate::parsers::avatar::Parser::default(), &text, true)?;
        Ok(result.buffer)
    }
}

pub fn get_save_sauce_default_avt(buf: &TextBuffer) -> (bool, String) {
    if buf.get_width() != 80 {
        return (true, "width != 80".to_string());
    }

    if buf.has_sauce() {
        return (true, String::new());
    }

    (false, String::new())
}
