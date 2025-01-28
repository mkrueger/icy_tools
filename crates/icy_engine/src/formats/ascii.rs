use std::path::Path;

use crate::{parse_with_parser, parsers, Buffer, BufferFeatures, EngineResult, OutputFormat, Position, TextPane};

use super::{LoadData, SaveOptions};

#[derive(Default)]
pub(crate) struct Ascii {}

impl OutputFormat for Ascii {
    fn get_file_extension(&self) -> &str {
        "asc"
    }

    fn get_name(&self) -> &str {
        "Ascii"
    }

    fn analyze_features(&self, _features: &BufferFeatures) -> String {
        String::new()
    }

    fn to_bytes(&self, buf: &crate::Buffer, options: &SaveOptions) -> EngineResult<Vec<u8>> {
        let mut result = Vec::new();
        let mut pos = Position::default();
        let height = buf.get_line_count();

        while pos.y < height {
            let line_length = buf.get_line_length(pos.y);
            while pos.x < line_length {
                let ch = buf.get_char(pos);
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

        if options.save_sauce {
            buf.write_sauce_info(icy_sauce::SauceDataType::Character, icy_sauce::char_caps::ContentType::Ascii, &mut result)?;
        }
        Ok(result)
    }

    fn load_buffer(&self, file_name: &Path, data: &[u8], load_data_opt: Option<LoadData>) -> EngineResult<crate::Buffer> {
        let load_data = load_data_opt.unwrap_or_default();
        let width = load_data.default_terminal_width.unwrap_or(80);
        let mut result: Buffer = Buffer::new((width, 25));

        result.is_terminal_buffer = false;
        result.file_name = Some(file_name.into());
        if let Some(sauce) = load_data.sauce_opt {
            result.load_sauce(sauce);
        }
        let (text, is_unicode) = crate::convert_ansi_to_utf8(data);
        if is_unicode {
            result.buffer_type = crate::BufferType::Unicode;
        }
        parse_with_parser(&mut result, &mut parsers::ascii::Parser::default(), &text, true)?;
        Ok(result)
    }
}

pub fn get_save_sauce_default_asc(buf: &Buffer) -> (bool, String) {
    if buf.get_width() != 80 {
        return (true, "width != 80".to_string());
    }

    if buf.has_sauce() {
        return (true, String::new());
    }

    (false, String::new())
}
