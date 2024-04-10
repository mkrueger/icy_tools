use super::BufferParser;
use crate::{AttributedChar, Buffer, CallbackAction, Caret, EngineResult, UnicodeConverter};

#[derive(Default)]
pub struct Parser {
    got_escape: bool,
}

#[derive(Default)]
pub struct CharConverter {}

impl UnicodeConverter for CharConverter {
    fn convert_from_unicode(&self, ch: char, _font_page: usize) -> char {
        match UNICODE_TO_ATARI.get(&ch) {
            Some(out_ch) => *out_ch,
            _ => ch,
        }
    }

    fn convert_to_unicode(&self, attributed_char: AttributedChar) -> char {
        match ATARI_TO_UNICODE.get(attributed_char.ch as usize) {
            Some(out_ch) => *out_ch,
            _ => attributed_char.ch,
        }
    }
}

impl BufferParser for Parser {
    fn print_char(&mut self, buf: &mut Buffer, current_layer: usize, caret: &mut Caret, ch: char) -> EngineResult<CallbackAction> {
        if self.got_escape {
            self.got_escape = false;
            buf.print_value(current_layer, caret, ch as u16);
            return Ok(CallbackAction::Update);
        }

        match ch {
            '\x1B' => self.got_escape = true,
            '\x1C' => caret.up(buf, current_layer, 1),
            '\x1D' => caret.down(buf, current_layer, 1),
            '\x1E' => caret.left(buf, 1),
            '\x1F' => caret.right(buf, 1),
            '\x7D' => buf.clear_screen(current_layer, caret),
            '\x7E' => caret.bs(buf, current_layer),
            '\x7F' | '\u{009E}' | '\u{009F}' => { /* TAB TODO */ }
            '\u{009B}' => caret.lf(buf, current_layer),
            '\u{009C}' => buf.remove_terminal_line(current_layer, caret.pos.y),
            '\u{009D}' => buf.insert_terminal_line(current_layer, caret.pos.y),
            //   '\u{009E}' => { /* clear TAB stops TODO */ }
            //   '\u{009F}' => { /* set TAB stops TODO */ }
            '\u{00FD}' => return Ok(CallbackAction::Beep),
            '\u{00FE}' => caret.del(buf, current_layer),
            '\u{00FF}' => caret.ins(buf, current_layer),
            _ => {
                let mut ch = ch as u16;
                if ch > 0x7F {
                    ch -= 0x80;
                    caret.attribute.set_foreground(0);
                    caret.attribute.set_background(7);
                } else {
                    caret.attribute.set_foreground(7);
                    caret.attribute.set_background(0);
                }
                buf.print_value(current_layer, caret, ch);
            }
        }
        Ok(CallbackAction::Update)
    }
}

lazy_static::lazy_static! {
    static ref UNICODE_TO_ATARI: std::collections::HashMap<char, char> = {
        let mut res = std::collections::HashMap::new();
        (0..128).for_each(|a: u8| {
            res.insert(ATARI_TO_UNICODE[a as usize], a as char);
        });
        res
    };
}

pub const ATARI_TO_UNICODE: [char; 256] = [
    '♥', '├', '🮇', '┘', '┤', '┐', '╱', '╲', '◢', '▗', '◣', '▝', '▘', '🮂', '▂', '▖', '♣', '┌', '─', '┼', '•', '▄', '▎', '┬', '┴', '▌', '└', '␛', '↑', '↓', '←',
    '→', ' ', '!', '"', '#', '$', '%', '&', '\'', '(', ')', '*', '+', ',', '-', '.', '/', '0', '1', '2', '3', '4', '5', '6', '7', '8', '9', ':', ';', '<', '=',
    '>', '?', '@', 'A', 'B', 'C', 'D', 'E', 'F', 'G', 'H', 'I', 'J', 'K', 'L', 'M', 'N', 'O', 'P', 'Q', 'R', 'S', 'T', 'U', 'V', 'W', 'X', 'Y', 'Z', '[', '\\',
    ']', '^', '_', '♦', 'a', 'b', 'c', 'd', 'e', 'f', 'g', 'h', 'i', 'j', 'k', 'l', 'm', 'n', 'o', 'p', 'q', 'r', 's', 't', 'u', 'v', 'w', 'x', 'y', 'z', '♠',
    '|', '🢰', '◀', '▶', '♥', '├', '▊', '┘', '┤', '┐', '╱', '╲', '◤', '▛', '◥', '▙', '▟', '▆', '▂', '▜', '♣', '┌', '─', '┼', '•', '▀', '▎', '┬', '┴', '▐', '└',
    '\x08', '↑', '↓', '←', '→', '█', '!', '"', '#', '$', '%', '&', '\'', '(', ')', '*', '+', ',', '-', '.', '/', '0', '1', '2', '3', '4', '5', '6', '7', '8',
    '9', ':', ';', '<', '=', '>', '?', '@', 'A', 'B', 'C', 'D', 'E', 'F', 'G', 'H', 'I', 'J', 'K', 'L', 'M', 'N', 'O', 'P', 'Q', 'R', 'S', 'T', 'U', 'V', 'W',
    'X', 'Y', 'Z', '[', '\\', ']', '^', '_', '♦', 'a', 'b', 'c', 'd', 'e', 'f', 'g', 'h', 'i', 'j', 'k', 'l', 'm', 'n', 'o', 'p', 'q', 'r', 's', 't', 'u', 'v',
    'w', 'x', 'y', 'z', '♠', '-', '🢰', '◀', '▶',
];
