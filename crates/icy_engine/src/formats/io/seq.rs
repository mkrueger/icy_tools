use super::super::{LoadData, SaveOptions};
use crate::{AttributedChar, BitFont, C64_DEFAULT_PALETTE, C64_SHIFTED, C64_UNSHIFTED, EditableScreen, Palette, Result, TextBuffer, TextScreen};

#[allow(unused)]
pub(crate) fn save_seq(buf: &TextBuffer, _options: &SaveOptions) -> Result<Vec<u8>> {
    if buf.buffer_type != crate::BufferType::Petscii {
        return Err(crate::EngineError::BufferTypeMismatch {
            expected: "Petscii".to_string(),
        });
    }

    Err(crate::EngineError::not_implemented("Seq export"))
}

pub(crate) fn load_seq(data: &[u8], load_data_opt: Option<LoadData>) -> Result<TextScreen> {
    let mut result = TextScreen::new((40, 25));

    result.buffer.clear_font_table();
    result.buffer.set_font(0, BitFont::from_bytes("", C64_UNSHIFTED).unwrap());
    result.buffer.set_font(1, BitFont::from_bytes("", C64_SHIFTED).unwrap());

    result.buffer.palette = Palette::from_slice(&C64_DEFAULT_PALETTE);
    result.buffer.buffer_type = crate::BufferType::Petscii;
    result.buffer.terminal_state.is_terminal_buffer = false;
    let load_data = load_data_opt.unwrap_or_default();
    if let Some(sauce) = load_data.sauce_opt {
        result.apply_sauce(&sauce);
    }

    seq_prepare(&mut result);
    crate::load_with_parser(&mut result, &mut icy_parser_core::PetsciiParser::default(), data, true, 25)?;
    Ok(result)
}

pub fn seq_prepare(result: &mut dyn EditableScreen) {
    for y in 0..result.height() {
        for x in 0..result.width() {
            let mut ch = AttributedChar::default();
            ch.attribute.set_foreground(7);
            ch.attribute.set_background(0);
            result.set_char((x, y).into(), ch);
        }
    }
    result.caret_mut().set_foreground(7);
    result.caret_mut().set_background(0);
    result.caret_mut().set_font_page(1);
}
