use std::path::Path;

use crate::{ctrla, parse_with_parser, parsers, Buffer, BufferFeatures, EngineResult, OutputFormat, Position, TextAttribute, TextPane};

use super::{LoadData, SaveOptions};

#[derive(Default)]
pub(super) struct CtrlA {}

impl OutputFormat for CtrlA {
    fn get_file_extension(&self) -> &str {
        "msg"
    }

    fn get_name(&self) -> &str {
        "CtrlA"
    }

    fn analyze_features(&self, _features: &BufferFeatures) -> String {
        String::new()
    }

    fn to_bytes(&self, buf: &crate::Buffer, options: &SaveOptions) -> EngineResult<Vec<u8>> {
        if buf.palette.len() != 16 {
            return Err(anyhow::anyhow!("Only 16 color palettes are supported by this format."));
        }
        let mut result = Vec::new();
        let mut last_attr = TextAttribute::default();
        let mut pos = Position::default();
        let height = buf.get_line_count();

        match options.screen_preparation {
            super::ScreenPreperation::None => {}
            super::ScreenPreperation::Home => {
                result.extend(b"\x01'");
            }
            super::ScreenPreperation::ClearScreen => {
                result.extend(b"\x01L");
            }
        }

        let mut was_bold = false;
        let mut was_blink = false;
        let mut was_high_bg = false;

        while pos.y < height {
            let line_length = buf.get_line_length(pos.y);

            while pos.x < line_length {
                let ch = buf.get_char(pos);

                if ch.attribute != last_attr {
                    let is_bold = ch.attribute.get_foreground() > 7;
                    let high_bg = ch.attribute.get_background() > 7;
                    let is_blink = ch.attribute.is_blinking();
                    let mut last_fore = last_attr.get_foreground();
                    let mut last_back = last_attr.get_background();

                    if !is_bold && was_bold || !high_bg && was_high_bg || !is_blink && was_blink {
                        result.extend_from_slice(b"\x01N");
                        was_bold = false;
                        was_high_bg = false;
                        was_blink = false;
                        last_fore = 7;
                        last_back = 0;
                    }

                    if is_bold && !was_bold {
                        result.extend_from_slice(b"\x01H");
                    }
                    if high_bg && !was_high_bg {
                        result.extend_from_slice(b"\x01E");
                    }

                    if is_blink && !was_blink {
                        result.extend_from_slice(b"\x01I");
                    }

                    if ch.attribute.get_foreground() != last_fore {
                        result.push(1);
                        result.push(ctrla::FG[ch.attribute.get_foreground() as usize % 8]);
                    }
                    if ch.attribute.get_background() != last_back {
                        result.push(1);
                        result.push(parsers::ctrla::BG[ch.attribute.get_background() as usize % 8]);
                    }
                    was_bold = is_bold;
                    was_high_bg = high_bg;
                    was_blink = is_blink;
                    last_attr = ch.attribute;
                }

                result.push(if ch.ch == '\0' { b' ' } else { ch.ch as u8 });
                pos.x += 1;
            }

            // do not end with eol
            if pos.y + 1 < height {
                result.push(13);
                result.push(10);
            }

            pos.x = 0;
            pos.y += 1;
        }
        Ok(result)
    }

    fn load_buffer(&self, file_name: &Path, data: &[u8], load_data_opt: Option<LoadData>) -> EngineResult<crate::Buffer> {
        let mut result = Buffer::new((80, 25));
        result.is_terminal_buffer = false;
        result.file_name = Some(file_name.into());
        let load_data = load_data_opt.unwrap_or_default();
        if let Some(sauce) = load_data.sauce_opt {
            result.load_sauce(sauce);
        }

        let (text, is_unicode) = crate::convert_ansi_to_utf8(data);
        if is_unicode {
            result.buffer_type = crate::BufferType::Unicode;
        }
        parse_with_parser(&mut result, &mut parsers::ctrla::Parser::default(), &text, true)?;
        Ok(result)
    }
}
