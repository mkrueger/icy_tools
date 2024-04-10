use super::{ansi, BufferParser};
use crate::{Buffer, CallbackAction, Caret, EngineResult, TextAttribute};

#[derive(Default)]
pub struct Parser {
    ansi_parser: ansi::Parser,

    // PCB
    pub pcb_code: bool,
    pub pcb_color: bool,
    pub pcb_value: u8,
    pub pcb_pos: i32,
}

impl BufferParser for Parser {
    fn print_char(&mut self, buf: &mut Buffer, current_layer: usize, caret: &mut Caret, ch: char) -> EngineResult<CallbackAction> {
        if self.pcb_color {
            self.pcb_pos += 1;
            if self.pcb_pos < 3 {
                match self.pcb_pos {
                    1 => {
                        self.pcb_value = conv_ch(ch);
                        return Ok(CallbackAction::NoUpdate);
                    }
                    2 => {
                        self.pcb_value = (self.pcb_value << 4) + conv_ch(ch);
                        caret.attribute = TextAttribute::from_u8(self.pcb_value, buf.ice_mode);
                    }
                    _ => {}
                }
            }
            self.pcb_color = false;
            self.pcb_code = false;
            return Ok(CallbackAction::NoUpdate);
        }

        if self.pcb_code {
            match ch {
                '@' => {
                    self.pcb_code = false;
                }
                'X' => {
                    self.pcb_color = true;
                    self.pcb_pos = 0;
                }
                _ => {}
            }
            return Ok(CallbackAction::NoUpdate);
        }
        match ch {
            '@' => {
                self.pcb_code = true;
                Ok(CallbackAction::NoUpdate)
            }
            _ => self.ansi_parser.print_char(buf, current_layer, caret, ch),
        }
    }
}

fn conv_ch(ch: char) -> u8 {
    if ch.is_ascii_digit() {
        return ch as u8 - b'0';
    }
    if ('a'..='f').contains(&ch) {
        return 10 + ch as u8 - b'a';
    }
    if ('A'..='F').contains(&ch) {
        return 10 + ch as u8 - b'A';
    }
    0
}
