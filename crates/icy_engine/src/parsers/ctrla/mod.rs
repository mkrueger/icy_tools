use super::BufferParser;
use crate::{CallbackAction, EditableScreen, EngineResult, Position, ansi};

const CTRL_A: char = 1 as char;
pub const FG: &[u8] = b"KBGCRMYW";
pub const BG: &[u8] = b"04261537";

#[derive(Default)]
pub struct Parser {
    ascii_parser: ansi::Parser,
    ctrl_a: bool,
    is_bold: bool,
    high_bg: bool,
}

impl BufferParser for Parser {
    fn print_char(&mut self, buf: &mut dyn EditableScreen, ch: char) -> EngineResult<CallbackAction> {
        if self.ctrl_a {
            self.ctrl_a = false;
            match ch {
                'L' => buf.clear_screen(),
                '\'' => buf.caret_mut().set_position(Position::default()),
                'J' => buf.clear_buffer_down(),
                '>' => buf.clear_line_end(),
                '<' => buf.left(1),
                '|' => buf.cr(),
                ']' => buf.down(1),
                'A' => {
                    let _ = self.ascii_parser.print_char(buf, CTRL_A);
                }
                'H' => {
                    self.is_bold = true;
                    let fg = buf.caret().attribute.get_foreground();
                    if fg < 8 {
                        buf.caret_mut().set_foreground(fg + 8);
                    }
                }
                'I' => buf.caret_mut().attribute.set_is_blinking(true),
                'E' => {
                    self.high_bg = true;
                    let bg = buf.caret().attribute.get_background();
                    if bg < 8 {
                        buf.caret_mut().set_background(bg + 8);
                    }
                }
                'N' => {
                    self.high_bg = false;
                    self.is_bold = false;
                    buf.caret_default_colors();
                    let fg = buf.caret().attribute.get_foreground();
                    if fg > 7 {
                        buf.caret_mut().set_foreground(fg - 8);
                    }
                    let bg = buf.caret().attribute.get_background();
                    if bg > 7 {
                        buf.caret_mut().set_background(bg - 8);
                    }
                }
                'Z' => { /* End of File */ }
                _ => {
                    if let Some(fg) = FG.iter().position(|c| *c == ch as u8) {
                        buf.caret_mut().set_foreground(fg as u32 + if self.is_bold { 8 } else { 0 });
                        return Ok(CallbackAction::None);
                    }
                    if let Some(bg) = BG.iter().position(|c| *c == ch as u8) {
                        buf.caret_mut().set_background(bg as u32 + if self.high_bg { 8 } else { 0 });
                        return Ok(CallbackAction::None);
                    }

                    let c = ch as i32;
                    if (128..=255).contains(&c) {
                        buf.right(c - 127);
                        return Ok(CallbackAction::None);
                    }
                    log::error!("Unsupported CtrlA sequence :'{ch}'.");
                }
            }
            return Ok(CallbackAction::None);
        }
        match ch {
            CTRL_A => {
                self.ctrl_a = true;
                Ok(CallbackAction::None)
            }
            _ => self.ascii_parser.print_char(buf, ch),
        }
    }
}
