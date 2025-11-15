use std::path::Path;

use super::{LoadData, SaveOptions};
use crate::{
    AttributedChar, BitFont, BufferFeatures, C64_DEFAULT_PALETTE, C64_SHIFTED, C64_UNSHIFTED, EditableScreen, EngineResult, OutputFormat, Palette, TextPane,
    TextScreen, parse_with_parser,
};

#[derive(Default)]
pub(super) struct Seq {}

impl OutputFormat for Seq {
    fn get_file_extension(&self) -> &str {
        "seq"
    }

    fn get_name(&self) -> &str {
        "Seq"
    }

    fn analyze_features(&self, _features: &BufferFeatures) -> String {
        String::new()
    }

    fn to_bytes(&self, buf: &mut crate::TextBuffer, _options: &SaveOptions) -> EngineResult<Vec<u8>> {
        if buf.buffer_type != crate::BufferType::Petscii {
            return Err(anyhow::anyhow!("Buffer is not a Petscii buffer!"));
        }

        Err(anyhow::anyhow!("not implemented!"))
    }

    fn load_buffer(&self, file_name: &Path, data: &[u8], load_data_opt: Option<LoadData>) -> EngineResult<crate::TextBuffer> {
        let mut result = TextScreen::new((40, 25));

        result.buffer.clear_font_table();
        result.buffer.set_font(0, BitFont::from_bytes("", C64_UNSHIFTED).unwrap());
        result.buffer.set_font(1, BitFont::from_bytes("", C64_SHIFTED).unwrap());

        for y in 0..result.get_height() {
            for x in 0..result.get_width() {
                let mut ch = AttributedChar::default();
                ch.attribute.set_foreground(14);
                ch.attribute.set_background(6);
                result.set_char((x, y).into(), ch);
            }
        }
        result.buffer.palette = Palette::from_slice(&C64_DEFAULT_PALETTE);
        result.buffer.buffer_type = crate::BufferType::Petscii;
        result.buffer.terminal_state.is_terminal_buffer = false;
        result.buffer.file_name = Some(file_name.into());
        let load_data = load_data_opt.unwrap_or_default();
        if let Some(sauce) = load_data.sauce_opt {
            result.buffer.load_sauce(sauce);
        }

        result.caret.set_foreground(14);
        result.caret.set_background(6);
        let text: String = data.iter().map(|&b| b as char).collect();
        parse_with_parser(&mut result, &mut crate::parsers::petscii::Parser::default(), &text, true)?;
        Ok(result.buffer)
    }
}
