use super::BufferParser;
use crate::{CallbackAction, EditableScreen, EngineResult};

#[derive(Default)]
pub struct Parser {
    got_escape: bool,
}

impl BufferParser for Parser {
    fn print_char(&mut self, buf: &mut dyn EditableScreen, ch: char) -> EngineResult<CallbackAction> {
        if self.got_escape {
            self.got_escape = false;
            buf.print_value(ch as u16);
            return Ok(CallbackAction::Update);
        }

        match ch {
            '\x1B' => self.got_escape = true,
            '\x1C' => buf.up(1),
            '\x1D' => buf.down(1),
            '\x1E' => buf.left(1),
            '\x1F' => buf.right(1),
            '\x7D' => buf.clear_screen(),
            '\x7E' => buf.bs(),
            '\x7F' | '\u{009E}' | '\u{009F}' => { /* TAB TODO */ }
            '\u{009B}' => return Ok(buf.lf()),
            '\u{009C}' => buf.remove_terminal_line(buf.caret().y),
            '\u{009D}' => buf.insert_terminal_line(buf.caret().y),
            //   '\u{009E}' => { /* clear TAB stops TODO */ }
            //   '\u{009F}' => { /* set TAB stops TODO */ }
            '\u{00FD}' => return Ok(CallbackAction::Beep),
            '\u{00FE}' => buf.del(),
            '\u{00FF}' => buf.ins(),
            _ => {
                let mut ch = ch as u16;
                if ch > 0x7F {
                    ch -= 0x80;
                    buf.caret_mut().attribute.set_foreground(0);
                    buf.caret_mut().attribute.set_background(7);
                } else {
                    buf.caret_mut().attribute.set_foreground(7);
                    buf.caret_mut().attribute.set_background(0);
                }
                buf.print_value(ch);
            }
        }
        Ok(CallbackAction::Update)
    }
}

lazy_static::lazy_static! {
    pub(crate) static ref UNICODE_TO_ATARI: std::collections::HashMap<char, char> = {
        let mut res = std::collections::HashMap::new();
        (0..128).for_each(|a: u8| {
            res.insert(ATARI_TO_UNICODE[a as usize], a as char);
        });
        res
    };
}

pub(crate) const ATARI_TO_UNICODE: [char; 256] = [
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
