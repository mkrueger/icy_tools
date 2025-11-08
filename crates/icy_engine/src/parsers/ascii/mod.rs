use codepages::tables::{CP437_TO_UNICODE, UNICODE_TO_CP437};

use super::{BufferParser, TAB};
use crate::{BEL, BS, CR, CallbackAction, EditableScreen, EngineResult, FF, LF, UnicodeConverter};
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
    fn print_char(&mut self, buf: &mut dyn EditableScreen, ch: char) -> EngineResult<CallbackAction> {
        match ch {
            '\x00' | '\u{00FF}' => {
                buf.caret_mut().reset_color_attribute();
            }
            BEL => {
                return Ok(CallbackAction::Beep);
            }
            LF => return Ok(buf.lf()),
            FF => buf.ff(),
            CR => buf.cr(),
            BS => buf.bs(),
            TAB => buf.tab_forward(),
            '\x7F' => buf.del(),
            _ => buf.print_value(ch as u16),
        }
        Ok(CallbackAction::NoUpdate)
    }
}
