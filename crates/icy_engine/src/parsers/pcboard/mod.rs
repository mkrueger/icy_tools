use super::{BufferParser, ansi};
use crate::{CallbackAction, EditableScreen, EngineResult, TextAttribute};

pub struct Parser {
    ansi_parser: ansi::Parser,

    // PCB
    pub pcb_code: bool,
    pub pcb_color: bool,
    pub pcb_value: u8,
    pub pcb_pos: i32,

    pub pcb_string: String,
}

impl Default for Parser {
    fn default() -> Self {
        let mut p = super::ansi::Parser::default();
        p.bs_is_ctrl_char = true;

        Self {
            ansi_parser: p,
            pcb_code: Default::default(),
            pcb_color: Default::default(),
            pcb_value: Default::default(),
            pcb_pos: Default::default(),
            pcb_string: String::new(),
        }
    }
}

impl BufferParser for Parser {
    fn print_char(&mut self, buf: &mut dyn EditableScreen, ch: char) -> EngineResult<CallbackAction> {
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
                        buf.caret_mut().attribute = TextAttribute::from_u8(self.pcb_value, buf.ice_mode());
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
                    if !self.pcb_string.is_empty() {
                        self.ansi_parser.print_char(buf, '@')?;
                        for c in self.pcb_string.chars() {
                            self.ansi_parser.print_char(buf, c)?;
                        }
                        self.ansi_parser.print_char(buf, '@')?;
                    }
                }
                'X' => {
                    self.pcb_color = true;
                    self.pcb_pos = 0;
                }
                _ => {
                    self.pcb_string.push(ch);
                }
            }
            return Ok(CallbackAction::NoUpdate);
        }
        match ch {
            '@' => {
                self.pcb_code = true;
                self.pcb_string.clear();
                Ok(CallbackAction::NoUpdate)
            }
            _ => self.ansi_parser.print_char(buf, ch),
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
