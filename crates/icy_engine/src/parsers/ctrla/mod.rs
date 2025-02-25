use super::BufferParser;
use crate::{Buffer, CallbackAction, Caret, EngineResult, Position, ansi};

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
    fn print_char(&mut self, buf: &mut Buffer, current_layer: usize, caret: &mut Caret, ch: char) -> EngineResult<CallbackAction> {
        if self.ctrl_a {
            self.ctrl_a = false;
            match ch {
                'L' => buf.clear_screen(0, caret),
                '\'' => caret.set_position(Position::default()),
                'J' => buf.clear_buffer_down(current_layer, caret),
                '>' => buf.clear_line_end(current_layer, caret),
                '<' => caret.left(buf, 1),
                '|' => caret.cr(buf),
                ']' => caret.down(buf, current_layer, 1),
                'A' => {
                    let _ = self.ascii_parser.print_char(buf, current_layer, caret, CTRL_A);
                }
                'H' => {
                    self.is_bold = true;
                    let fg = caret.attribute.get_foreground();
                    if fg < 8 {
                        caret.set_foreground(fg + 8);
                    }
                }
                'I' => caret.attribute.set_is_blinking(true),
                'E' => {
                    self.high_bg = true;
                    let bg = caret.attribute.get_background();
                    if bg < 8 {
                        caret.set_background(bg + 8);
                    }
                }
                'N' => {
                    self.high_bg = false;
                    self.is_bold = false;
                    caret.reset_color_attribute();
                    let fg = caret.attribute.get_foreground();
                    if fg > 7 {
                        caret.set_foreground(fg - 8);
                    }
                    let bg = caret.attribute.get_background();
                    if bg > 7 {
                        caret.set_background(bg - 8);
                    }
                }
                'Z' => { /* End of File */ }
                _ => {
                    if let Some(fg) = FG.iter().position(|c| *c == ch as u8) {
                        caret.set_foreground(fg as u32 + if self.is_bold { 8 } else { 0 });
                        return Ok(CallbackAction::NoUpdate);
                    }
                    if let Some(bg) = BG.iter().position(|c| *c == ch as u8) {
                        caret.set_background(bg as u32 + if self.high_bg { 8 } else { 0 });
                        return Ok(CallbackAction::NoUpdate);
                    }

                    let c = ch as i32;
                    if (128..=255).contains(&c) {
                        caret.right(buf, c - 127);
                        return Ok(CallbackAction::NoUpdate);
                    }
                    log::error!("Unsupported CtrlA sequence :'{ch}'.");
                }
            }
            return Ok(CallbackAction::NoUpdate);
        }
        match ch {
            CTRL_A => {
                self.ctrl_a = true;
                Ok(CallbackAction::NoUpdate)
            }
            _ => self.ascii_parser.print_char(buf, current_layer, caret, ch),
        }
    }
}
