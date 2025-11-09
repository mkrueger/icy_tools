#[cfg(test)]
use crate::TextScreen;
use crate::{
    EditableScreen, EngineResult, Size,
    ansi::mouse_event::{KeyModifiers, MouseButton, MouseEventType},
};

use self::{ansi::sound::AnsiMusic, rip::bgi::MouseField};

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
    Update,
    NoUpdate,
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

const EMPTY_MOUSE_FIELD: Vec<MouseField> = Vec::new();

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

    fn get_mouse_fields(&self) -> Vec<MouseField> {
        EMPTY_MOUSE_FIELD
    }

    fn has_renederer(&self) -> bool {
        false
    }

    fn picture_is_empty(&self) -> bool {
        true
    }

    fn get_picture_data(&mut self) -> Option<(Size, Vec<u8>)> {
        None
    }
}

#[cfg(test)]
fn create_buffer<T: BufferParser>(parser: &mut T, input: &[u8]) -> TextScreen {
    use crate::{Caret, SelectionMask, TextBuffer, TextScreen};

    let mut buf = TextScreen {
        buffer: TextBuffer::create((80, 25)),
        caret: Caret::default(),
        current_layer: 0,
        selection_opt: None,
        selection_mask: SelectionMask::default(),
    };

    buf.terminal_state_mut().is_terminal_buffer = true;
    buf.buffer.layers.first_mut().unwrap().lines.clear();

    update_buffer(&mut buf, parser, input);
    while parser.get_next_action(&mut buf).is_some() {}

    buf
}

#[cfg(test)]
fn update_buffer<T: BufferParser>(buf: &mut dyn EditableScreen, parser: &mut T, input: &[u8]) {
    for b in input {
        parser.print_char(buf, *b as char).unwrap(); // test code
    }
}
/*
#[cfg(test)]
fn update_buffer_force<T: BufferParser>(buf: &mut dyn EditableScreen, parser: &mut T, input: &[u8]) {
    for b in input {
        let _ = parser.print_char(buf, *b as char); // test code
    }
}

#[cfg(test)]
fn get_simple_action<T: BufferParser>(parser: &mut T, input: &[u8]) -> CallbackAction {
    use crate::{Buffer, Caret};

    let mut buf = TextScreen {
        buffer: Buffer::create((80, 25)),
        caret: Caret::default(),
        current_layer: 0,
        selection_opt: None,
    };
    buf.buffer.terminal_state.is_terminal_buffer = true;

    get_action(&mut buf, parser, input)
}

#[cfg(test)]
fn get_action<T: BufferParser>(buf: &mut dyn EditableScreen, parser: &mut T, input: &[u8]) -> CallbackAction {
    let mut action = CallbackAction::NoUpdate;
    for b in input {
        action = parser.print_char(buf, *b as char).unwrap(); // test code
    }

    action
}
*/
