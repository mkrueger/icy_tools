use std::path::Path;

use super::{LoadData, SaveOptions};
use crate::{ATARI, ATARI_DEFAULT_PALETTE, BitFont, BufferFeatures, EditableScreen, EngineResult, OutputFormat, Palette, Position, TextPane, TextScreen};

#[derive(Default)]
pub(super) struct Atascii {}

impl OutputFormat for Atascii {
    fn get_file_extension(&self) -> &str {
        "ata"
    }

    fn get_name(&self) -> &str {
        "Atascii"
    }

    fn analyze_features(&self, _features: &BufferFeatures) -> String {
        String::new()
    }

    fn to_bytes(&self, buf: &mut crate::TextBuffer, _options: &SaveOptions) -> EngineResult<Vec<u8>> {
        if buf.buffer_type != crate::BufferType::Atascii {
            return Err(anyhow::anyhow!("Buffer is not an Atascii buffer!"));
        }

        let mut result = Vec::new();
        let mut pos = Position::default();
        let height = buf.get_line_count();

        while pos.y < height {
            let line_length = buf.get_line_length(pos.y);
            while pos.x < line_length {
                let attr_ch = buf.get_char(pos);
                let mut ch = attr_ch.ch as u8;
                if attr_ch.attribute.background_color > 0 {
                    ch += 0x80;
                }

                // escape control chars
                if ch == b'\x1B' || ch == b'\x1C' || ch == b'\x1D' || ch == b'\x1E' || ch == b'\x1F' || ch == b'\x7D' || ch == b'\x7E' || ch == b'\x7F' {
                    result.push(b'\x1B');
                }

                result.push(ch);
                pos.x += 1;
            }

            // do not end with eol
            if pos.x < buf.get_width() && pos.y + 1 < height {
                result.push(155);
            }

            pos.x = 0;
            pos.y += 1;
        }

        Ok(result)
    }

    fn load_buffer(&self, file_name: &Path, data: &[u8], load_data_opt: Option<LoadData>) -> EngineResult<crate::TextBuffer> {
        let mut result = TextScreen::new((40, 24));

        result.buffer.clear_font_table();
        let font = BitFont::from_bytes("", ATARI).unwrap();
        result.buffer.set_font(0, font);
        result.buffer.palette = Palette::from_slice(&ATARI_DEFAULT_PALETTE);

        result.buffer.buffer_type = crate::BufferType::Atascii;
        result.buffer.terminal_state.is_terminal_buffer = false;
        result.buffer.file_name = Some(file_name.into());
        let load_data = load_data_opt.unwrap_or_default();
        if let Some(sauce) = &load_data.sauce_opt {
            result.apply_sauce(sauce);
        }

        crate::load_with_parser(&mut result, &mut icy_parser_core::AtasciiParser::default(), data, true, 24)?;
        Ok(result.buffer)
    }
}
