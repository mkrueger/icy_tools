use std::path::Path;

use crate::{BufferFeatures, EditableScreen, Result, OutputFormat, Position, TextAttribute, TextPane, TextScreen};

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

    fn to_bytes(&self, buf: &mut crate::TextBuffer, _options: &SaveOptions) -> Result<Vec<u8>> {
        if buf.palette.len() != 16 {
            return Err(crate::EngineError::Only16ColorPalettesSupported);
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

    fn load_buffer(&self, file_name: &Path, data: &[u8], load_data_opt: Option<LoadData>) -> Result<crate::TextBuffer> {
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
        crate::load_with_parser(&mut result, &mut icy_parser_core::RenegadeParser::default(), file_data, true, min_height)?;
        Ok(result.buffer)
    }
}
