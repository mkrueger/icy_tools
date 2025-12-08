use std::path::Path;

use super::{LoadData, Position, SaveOptions, TextAttribute};
use crate::{AttributedChar, BufferFeatures, OutputFormat, Result, TextBuffer, TextPane};

#[derive(Default)]
pub(super) struct Bin {}

impl OutputFormat for Bin {
    fn get_file_extension(&self) -> &str {
        "bin"
    }

    fn get_name(&self) -> &str {
        "Bin"
    }

    fn analyze_features(&self, _features: &BufferFeatures) -> String {
        String::new()
    }

    fn to_bytes(&self, buf: &mut crate::TextBuffer, options: &SaveOptions) -> Result<Vec<u8>> {
        let mut result = Vec::new();

        for y in 0..buf.get_height() {
            for x in 0..buf.get_width() {
                let ch = buf.get_char((x, y).into());
                result.push(ch.ch as u8);
                result.push(ch.attribute.as_u8(buf.ice_mode));
            }
        }
        if let Some(sauce) = &options.save_sauce {
            sauce.write(&mut result)?;
        }
        Ok(result)
    }

    fn load_buffer(&self, file_name: &Path, data: &[u8], load_data_opt: Option<LoadData>) -> Result<crate::TextBuffer> {
        let mut result = TextBuffer::new((160, 25));
        result.terminal_state.is_terminal_buffer = false;
        result.file_name = Some(file_name.into());
        let load_data = load_data_opt.unwrap_or_default();
        let max_height = load_data.max_height();
        if let Some(sauce) = &load_data.sauce_opt {
            super::apply_sauce_to_buffer(&mut result, sauce);
        }
        let mut o = 0;
        let mut pos = Position::default();
        loop {
            // Check height limit before processing a new row
            if let Some(max_h) = max_height {
                if pos.y >= max_h {
                    result.set_height(pos.y);
                    return Ok(result);
                }
            }

            for _ in 0..result.get_width() {
                if o >= data.len() {
                    result.set_height(result.layers[0].get_height());
                    return Ok(result);
                }

                if o + 1 >= data.len() {
                    // last byte is not important enough to throw an error
                    // there seem to be some invalid files out there.
                    log::error!("Invalid Bin. Read char block beyond EOF.");
                    result.set_height(result.layers[0].get_height());
                    return Ok(result);
                }

                result.layers[0].set_height(pos.y + 1);
                let mut attribute = TextAttribute::from_u8(data[o + 1], result.ice_mode);
                if attribute.is_bold() {
                    attribute.set_foreground(attribute.foreground_color + 8);
                    attribute.set_is_bold(false);
                }

                result.layers[0].set_char(pos, AttributedChar::new(data[o] as char, attribute));
                pos.x += 1;
                o += 2;
            }
            pos.x = 0;
            pos.y += 1;
        }
    }
}

pub fn get_save_sauce_default_binary(buf: &TextBuffer) -> (bool, String) {
    if buf.get_width() != 160 {
        return (true, "width != 160".to_string());
    }

    (false, String::new())
}
