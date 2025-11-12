use super::{BufferParser, TAB};
use crate::{BEL, BS, CR, CallbackAction, EditableScreen, EngineResult, FF, LF};

#[derive(Default)]
pub struct Parser {}

impl BufferParser for Parser {
    fn print_char(&mut self, buf: &mut dyn EditableScreen, ch: char) -> EngineResult<CallbackAction> {
        match ch {
            '\x00' | '\u{00FF}' => {
                buf.caret_default_colors();
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
        Ok(CallbackAction::None)
    }
}
