use super::BufferParser;
use crate::{CallbackAction, EditableScreen, EngineResult, ParserError, Position, TextAttribute};
use std::cmp::{max, min};

#[derive(Debug)]
enum AvtReadState {
    Chars,
    RepeatChars,
    ReadCommand,
    MoveCursor,
    ReadColor,
}

pub const AVT_MOVE_CLREOL: u8 = 7;
pub const AVT_MOVE_CURSOR: u8 = 8;

/// Starts Avatar command
const AVT_CMD: char = '\x16';
/// clear the current window and set current attribute to default.
const AVT_CLR: char = '\x0C';
///  Read two bytes from the modem. Send the first one to the screen as many times as the binary value
///  of the second one. This is the exception where the two bytes may have their high bit set. Do not reset it here!
const AVT_REP: char = '\x19';

pub struct Parser {
    ansi_parser: super::ansi::Parser,

    avt_state: AvtReadState,
    avatar_state: i32,
    avt_repeat_char: char,
}

impl Default for Parser {
    fn default() -> Self {
        let mut p = super::ansi::Parser::default();
        p.bs_is_ctrl_char = true;
        Self {
            ansi_parser: p,
            avatar_state: 0,
            avt_state: AvtReadState::Chars,
            avt_repeat_char: ' ',
        }
    }
}

impl Parser {
    fn print_fallback(&mut self, buf: &mut dyn EditableScreen, ch: char) -> EngineResult<CallbackAction> {
        self.ansi_parser.print_char(buf, ch)
    }
}

impl BufferParser for Parser {
    fn print_char(&mut self, buf: &mut dyn EditableScreen, ch: char) -> EngineResult<CallbackAction> {
        match self.avt_state {
            AvtReadState::Chars => {
                match ch {
                    AVT_CLR => buf.ff(), // clear & reset attributes
                    AVT_REP => {
                        self.avt_state = AvtReadState::RepeatChars;
                        self.avatar_state = 1;
                    }
                    AVT_CMD => {
                        self.avt_state = AvtReadState::ReadCommand;
                    }
                    _ => return self.print_fallback(buf, ch),
                }
                Ok(CallbackAction::None)
            }
            AvtReadState::ReadCommand => {
                match ch as u8 {
                    1 => {
                        self.avt_state = AvtReadState::ReadColor;
                        return Ok(CallbackAction::None);
                    }
                    2 => {
                        buf.caret_mut().attribute.set_is_blinking(true);
                    }
                    3 => {
                        let y = max(0, buf.caret_mut().y - 1);
                        buf.caret_mut().y = y;
                    }
                    4 => {
                        let y = buf.caret().y;
                        buf.caret_mut().y = y + 1;
                    }

                    5 => {
                        let x = max(0, buf.caret_mut().x - 1);
                        buf.caret_mut().x = x;
                    }
                    6 => {
                        let x = min(79, buf.caret_mut().x + 1);
                        buf.caret_mut().x = x;
                    }
                    AVT_MOVE_CLREOL => {
                        return Err(ParserError::Description("todo: avt cleareol").into());
                    }
                    AVT_MOVE_CURSOR => {
                        self.avt_state = AvtReadState::MoveCursor;
                        self.avatar_state = 1;
                        return Ok(CallbackAction::None);
                    }
                    // TODO implement commands from FSC0025.txt & FSC0037.txt
                    _ => {
                        self.avt_state = AvtReadState::Chars;
                        return Err(ParserError::Description("unsupported avatar command").into());
                    }
                }
                self.avt_state = AvtReadState::Chars;
                Ok(CallbackAction::None)
            }
            AvtReadState::RepeatChars => match self.avatar_state {
                1 => {
                    self.avt_repeat_char = ch;
                    self.avatar_state = 2;
                    Ok(CallbackAction::None)
                }
                2 => {
                    self.avatar_state = 3;
                    let repeat_count = ch as usize;
                    for _ in 0..repeat_count {
                        self.ansi_parser.print_char(buf, self.avt_repeat_char)?;
                    }
                    self.avt_state = AvtReadState::Chars;
                    Ok(CallbackAction::None)
                }
                _ => {
                    self.avt_state = AvtReadState::Chars;
                    Err(ParserError::Description("error in reading avt state").into())
                }
            },
            AvtReadState::ReadColor => {
                let ice = buf.ice_mode();
                buf.caret_mut().attribute = TextAttribute::from_u8(ch as u8, ice);
                self.avt_state = AvtReadState::Chars;
                Ok(CallbackAction::None)
            }
            AvtReadState::MoveCursor => match self.avatar_state {
                1 => {
                    self.avt_repeat_char = ch;
                    self.avatar_state = 2;
                    Ok(CallbackAction::None)
                }
                2 => {
                    buf.caret_mut().set_position(Position::new(self.avt_repeat_char as i32, ch as i32));

                    self.avt_state = AvtReadState::Chars;
                    Ok(CallbackAction::None)
                }
                _ => Err(ParserError::Description("error in reading avt avt_gotoxy").into()),
            },
        }
    }
}
