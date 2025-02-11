use std::path::Path;

use crate::{parse_with_parser, parsers, Buffer, BufferFeatures, EngineResult, OutputFormat, Position, TextAttribute, TextPane};

use super::{LoadData, SaveOptions};

#[derive(Default)]
pub(super) struct Renegade {}

impl OutputFormat for Renegade {
    fn get_file_extension(&self) -> &str {
        "an1"
    }

    fn get_alt_extensions(&self) -> Vec<String> {
        vec![
            "an2".to_string(),
            "an3".to_string(),
            "an4".to_string(),
            "an5".to_string(),
            "an6".to_string(),
            "an7".to_string(),
            "an8".to_string(),
            "an9".to_string(),
        ]
    }

    fn get_name(&self) -> &str {
        "Renegade"
    }

    fn analyze_features(&self, _features: &BufferFeatures) -> String {
        String::new()
    }

    fn to_bytes(&self, buf: &mut crate::Buffer, _options: &SaveOptions) -> EngineResult<Vec<u8>> {
        if buf.palette.len() != 16 {
            return Err(anyhow::anyhow!("Only 16 color palettes are supported by this format."));
        }
        let mut result = Vec::new();
        let mut last_attr = TextAttribute::default();
        let mut pos = Position::default();
        let height = buf.get_line_count();

        while pos.y < height {
            let line_length = buf.get_line_length(pos.y);
            while pos.x < line_length {
                let ch = buf.get_char(pos);
                if ch.attribute != last_attr {
                    let last_fore = last_attr.get_foreground();
                    let last_back = last_attr.get_background();
                    if ch.attribute.get_foreground() != last_fore {
                        result.extend(format!("|{:02}", ch.attribute.get_foreground()).as_bytes());
                    }
                    if ch.attribute.get_background() != last_back {
                        result.extend(format!("|{:02}", 16 + ch.attribute.get_background()).as_bytes());
                    }
                    last_attr = ch.attribute;
                }
                result.push(if ch.ch == '\0' { b' ' } else { ch.ch as u8 });
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
        parse_with_parser(&mut result, &mut parsers::renegade::Parser::default(), &text, true)?;
        Ok(result)
    }
}
