use crate::{
    EditableScreen, EngineResult,
    ansi::mouse_event::{KeyModifiers, MouseButton, MouseEventType},
};

use self::ansi::sound::AnsiMusic;

use super::Position;

mod parser_errors;
pub use parser_errors::*;

pub mod ansi;
pub mod ascii;
pub mod atascii;
pub mod avatar;
pub mod ctrla;
pub mod igs;
pub mod mode7;
pub mod pcboard;
pub mod petscii;
pub mod renegade;
pub mod rip;
pub mod skypix;
pub mod viewdata;

pub const BEL: char = '\x07';
pub const LF: char = '\n';
pub const CR: char = '\r';
pub const BS: char = '\x08';
pub const VT: char = '\x0B';
pub const FF: char = '\x0C';
pub const TAB: char = '\t';

#[derive(Debug, PartialEq)]
pub enum CallbackAction {
    None,

    Update,
    Beep,
    RunSkypixSequence(Vec<i32>),
    SendString(String),
    PlayMusic(AnsiMusic),
    ChangeBaudEmulation(ansi::BaudEmulation),
    ResizeTerminal(i32, i32),
    XModemTransfer(String),
    /// Pause for milliseconds
    Pause(u32),
    ScrollDown(i32),
    PlayGISTSound(Vec<i16>),
    SendMouseEvent(MouseEventType, Position, MouseButton, KeyModifiers),
}

pub trait BufferParser: Send {
    fn get_next_action(&mut self, _buffer: &mut dyn EditableScreen) -> Option<CallbackAction> {
        None
    }

    /// Prints a character to the buffer. Gives back an optional string returned to the sender (in case for terminals).
    ///
    /// # Errors
    ///
    /// This function will return an error if .
    fn print_char(&mut self, buffer: &mut dyn EditableScreen, c: char) -> EngineResult<CallbackAction>;
}
