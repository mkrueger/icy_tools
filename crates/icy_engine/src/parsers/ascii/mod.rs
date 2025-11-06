use codepages::tables::{CP437_TO_UNICODE, UNICODE_TO_CP437};

use super::{BufferParser, TAB};
use crate::{BEL, BS, Buffer, CR, CallbackAction, Caret, EngineResult, FF, LF, UnicodeConverter};
#[derive(Default)]
pub struct Parser {}

#[cfg(test)]
mod tests;

#[derive(Default)]
pub struct CP437Converter {}

impl UnicodeConverter for CP437Converter {
    fn convert_from_unicode(&self, ch: char, _font_page: usize) -> char {
        if let Some(tch) = UNICODE_TO_CP437.get(&ch) { *tch as char } else { ch }
    }

    fn convert_to_unicode(&self, ch: char) -> char {
        match CP437_TO_UNICODE.get(ch as usize) {
            Some(out_ch) => *out_ch,
            _ => ch,
        }
    }
}

#[derive(Default)]
pub struct IdentityConverter {}

impl UnicodeConverter for IdentityConverter {
    fn convert_from_unicode(&self, ch: char, _font_page: usize) -> char {
        ch
    }

    fn convert_to_unicode(&self, ch: char) -> char {
        ch
    }
}

impl BufferParser for Parser {
    fn print_char(&mut self, buf: &mut Buffer, current_layer: usize, caret: &mut Caret, ch: char) -> EngineResult<CallbackAction> {
        match ch {
            '\x00' | '\u{00FF}' => {
                caret.reset_color_attribute();
            }
            BEL => {
                return Ok(CallbackAction::Beep);
            }
            LF => return Ok(caret.lf(buf, current_layer)),
            FF => caret.ff(buf, current_layer),
            CR => caret.cr(buf),
            BS => caret.bs(buf, current_layer),
            TAB => caret.tab_forward(buf),
            '\x7F' => caret.del(buf, current_layer),
            _ => buf.print_value(current_layer, caret, ch as u16),
        }
        Ok(CallbackAction::NoUpdate)
    }
}
