use super::{ansi, BufferParser};
use crate::{Buffer, CallbackAction, Caret, EngineResult};

#[derive(Default, Clone, Copy, PartialEq)]
enum State {
    #[default]
    Normal,
    ParseFirstColor,
    ParseSecondColor(u8),
}

#[derive(Default)]
pub struct Parser {
    ansi_parser: ansi::Parser,
    state: State,
}

impl BufferParser for Parser {
    fn print_char(&mut self, buf: &mut Buffer, current_layer: usize, caret: &mut Caret, ch: char) -> EngineResult<CallbackAction> {
        match self.state {
            State::Normal => match ch {
                '|' => {
                    self.state = State::ParseFirstColor;
                    Ok(CallbackAction::NoUpdate)
                }
                _ => self.ansi_parser.print_char(buf, current_layer, caret, ch),
            },
            State::ParseFirstColor => {
                let code = ch as u8;
                if !(b'0'..=b'3').contains(&code) {
                    self.state = State::Normal;
                    return Err(anyhow::anyhow!("Invalid color code: {}", ch));
                }
                self.state = State::ParseSecondColor((code - b'0') * 10);
                Ok(CallbackAction::NoUpdate)
            }
            State::ParseSecondColor(first) => {
                self.state = State::Normal;

                let code = ch as u8;
                if !code.is_ascii_digit() {
                    return Err(anyhow::anyhow!("Invalid color code: {}", ch));
                }
                let color = first + (code - b'0');
                if color < 16 {
                    caret.attribute.set_foreground(color as u32);
                } else {
                    caret.attribute.set_background((color - 16) as u32);
                }
                Ok(CallbackAction::NoUpdate)
            }
        }
    }
}
