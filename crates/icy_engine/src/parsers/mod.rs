use crate::{EngineResult, Line, Size, TextPane};
use std::cmp::{max, min};

use self::{ansi::sound::AnsiMusic, rip::bgi::MouseField};

use super::{AttributedChar, Buffer, Caret, Position};

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
}

pub trait UnicodeConverter: Send + Sync {
    fn convert_from_unicode(&self, ch: char, font_page: usize) -> char;
    fn convert_to_unicode(&self, attributed_char: AttributedChar) -> char;
}

const EMPTY_MOUSE_FIELD: Vec<MouseField> = Vec::new();

pub trait BufferParser: Send {
    fn get_next_action(&mut self, _buffer: &mut Buffer, _caret: &mut Caret, _current_layer: usize) -> Option<CallbackAction> {
        None
    }

    /// Prints a character to the buffer. Gives back an optional string returned to the sender (in case for terminals).
    ///
    /// # Errors
    ///
    /// This function will return an error if .
    fn print_char(&mut self, buffer: &mut Buffer, current_layer: usize, caret: &mut Caret, c: char) -> EngineResult<CallbackAction>;

    fn get_mouse_fields(&self) -> Vec<MouseField> {
        EMPTY_MOUSE_FIELD
    }

    fn get_picture_data(&mut self) -> Option<(Size, Vec<u8>)> {
        None
    }
}

impl Caret {
    /// (line feed, LF, \n, ^J), moves the print head down one line, or to the left edge and down. Used as the end of line marker in most UNIX systems and variants.
    pub fn lf(&mut self, buf: &mut Buffer, current_layer: usize) -> CallbackAction {
        let was_ooe = self.pos.y > buf.get_last_editable_line();
        let mut line_inserted = 0;
        self.pos.x = 0;
        self.pos.y += 1;
        while self.pos.y >= buf.layers[current_layer].lines.len() as i32 {
            if buf.terminal_state.fixed_size && self.pos.y >= buf.terminal_state.get_height() {
                line_inserted += 1;
                if !buf.layers[current_layer].lines.is_empty() {
                    buf.layers[current_layer].lines.remove(0);
                }
                self.pos.y -= 1;
                continue;
            }
            let len = buf.layers[current_layer].lines.len();
            let buffer_width = buf.terminal_state.get_width();
            buf.layers[current_layer].lines.insert(len, Line::with_capacity(buffer_width));
        }

        if !buf.is_terminal_buffer {
            if line_inserted > 0 {
                return CallbackAction::ScrollDown(line_inserted);
            }
            return CallbackAction::Update;
        }
        if self.pos.y + 1 > buf.get_height() {
            buf.set_height(self.pos.y + 1);
        }

        if was_ooe {
            buf.terminal_state.limit_caret_pos(buf, self);
        } else {
            self.check_scrolling_on_caret_down(buf, current_layer, false);
        }
        if line_inserted > 0 {
            return CallbackAction::ScrollDown(line_inserted);
        }
        CallbackAction::Update
    }

    /// (form feed, FF, \f, ^L), to cause a printer to eject paper to the top of the next page, or a video terminal to clear the screen.
    pub fn ff(&mut self, buf: &mut Buffer, current_layer: usize) {
        buf.reset_terminal();
        buf.layers[current_layer].clear();
        buf.stop_sixel_threads();
        self.pos = Position::default();
        self.set_is_visible(true);
        self.reset_color_attribute();
    }

    /// (carriage return, CR, \r, ^M), moves the printing position to the start of the line.
    pub fn cr(&mut self, _buf: &Buffer) {
        self.pos.x = 0;
    }

    pub fn eol(&mut self, buf: &Buffer) {
        self.pos.x = buf.terminal_state.get_width() - 1;
    }

    pub fn home(&mut self, buf: &Buffer) {
        self.pos = buf.upper_left_position();
    }

    /// (backspace, BS, \b, ^H), may overprint the previous character
    pub fn bs(&mut self, buf: &mut Buffer, current_layer: usize) {
        self.pos.x = max(0, self.pos.x - 1);
        buf.layers[current_layer].set_char(self.pos, AttributedChar::new(' ', self.attribute));
    }

    pub fn del(&mut self, buf: &mut Buffer, current_layer: usize) {
        if let Some(line) = buf.layers[current_layer].lines.get_mut(self.pos.y as usize) {
            let i = self.pos.x as usize;
            if i < line.chars.len() {
                line.chars.remove(i);
            }
        }
    }

    pub fn ins(&mut self, buf: &mut Buffer, current_layer: usize) {
        if let Some(line) = buf.layers[current_layer].lines.get_mut(self.pos.y as usize) {
            let i = self.pos.x as usize;
            if i < line.chars.len() {
                line.chars.insert(i, AttributedChar::new(' ', self.attribute));
            }
        }
    }

    pub fn erase_charcter(&mut self, buf: &mut Buffer, current_layer: usize, number: i32) {
        let mut i = self.pos.x;
        let number = min(buf.terminal_state.get_width() - i, number);
        if number <= 0 {
            return;
        }
        if let Some(line) = buf.layers[current_layer].lines.get_mut(self.pos.y as usize) {
            for _ in 0..number {
                line.set_char(i, AttributedChar::new(' ', self.attribute));
                i += 1;
            }
        }
    }

    pub fn left(&mut self, buf: &Buffer, num: i32) {
        self.pos.x = self.pos.x.saturating_sub(num);
        buf.terminal_state.limit_caret_pos(buf, self);
    }

    pub fn right(&mut self, buf: &Buffer, num: i32) {
        self.pos.x = self.pos.x.saturating_add(num);
        buf.terminal_state.limit_caret_pos(buf, self);
    }

    pub fn up(&mut self, buf: &mut Buffer, current_layer: usize, num: i32) {
        self.pos.y = self.pos.y.saturating_sub(num);
        self.check_scrolling_on_caret_up(buf, current_layer, false);
        buf.terminal_state.limit_caret_pos(buf, self);
    }

    pub fn down(&mut self, buf: &mut Buffer, current_layer: usize, num: i32) {
        self.pos.y += num;
        self.check_scrolling_on_caret_down(buf, current_layer, false);
        buf.terminal_state.limit_caret_pos(buf, self);
    }

    /// Moves the cursor down one line in the same column. If the cursor is at the bottom margin, the page scrolls up.
    pub fn index(&mut self, buf: &mut Buffer, current_layer: usize) {
        self.pos.y += 1;
        self.check_scrolling_on_caret_down(buf, current_layer, true);
        buf.terminal_state.limit_caret_pos(buf, self);
    }

    /// Moves the cursor up one line in the same column. If the cursor is at the top margin, the page scrolls down.
    pub fn reverse_index(&mut self, buf: &mut Buffer, current_layer: usize) {
        self.pos.y -= 1;
        self.check_scrolling_on_caret_up(buf, current_layer, true);
        buf.terminal_state.limit_caret_pos(buf, self);
    }

    pub fn next_line(&mut self, buf: &mut Buffer, current_layer: usize) {
        self.pos.y += 1;
        self.pos.x = 0;
        self.check_scrolling_on_caret_down(buf, current_layer, true);
        buf.terminal_state.limit_caret_pos(buf, self);
    }

    fn check_scrolling_on_caret_up(&mut self, buf: &mut Buffer, current_layer: usize, force: bool) {
        if buf.needs_scrolling() || force {
            let last = buf.get_first_editable_line();
            while self.pos.y < last {
                buf.scroll_down(current_layer);
                self.pos.y += 1;
            }
        }
    }

    fn check_scrolling_on_caret_down(&mut self, buf: &mut Buffer, current_layer: usize, force: bool) {
        if (buf.needs_scrolling() || force) && self.pos.y > buf.get_last_editable_line() {
            buf.scroll_up(current_layer);
            self.pos.y -= 1;
        }
    }
}

impl Buffer {
    fn print_value(&mut self, layer: usize, caret: &mut Caret, ch: u16) {
        if let Some(ch) = char::from_u32(ch as u32) {
            let ch = AttributedChar::new(ch, caret.attribute);
            self.print_char(layer, caret, ch);
        }
    }

    pub fn print_char(&mut self, layer: usize, caret: &mut Caret, ch: AttributedChar) {
        let buffer_width = self.layers[layer].get_width();
        if caret.insert_mode {
            let layer = &mut self.layers[layer];
            if layer.lines.len() < caret.pos.y as usize + 1 {
                layer.lines.resize(caret.pos.y as usize + 1, Line::with_capacity(buffer_width));
            }
            layer.lines[caret.pos.y as usize].insert_char(caret.pos.x, AttributedChar::default());
        }
        if caret.pos.y + 1 > self.layers[layer].get_height() {
            self.layers[layer].set_height(caret.pos.y + 1);
        }
        if self.is_terminal_buffer && caret.pos.y + 1 > self.get_height() {
            self.set_height(caret.pos.y + 1);
        }

        self.layers[layer].set_char(caret.pos, ch);
        caret.pos.x += 1;
        if caret.pos.x
            >= if self.is_terminal_buffer {
                self.terminal_state.get_width()
            } else {
                buffer_width
            }
        {
            if let crate::AutoWrapMode::AutoWrap = self.terminal_state.auto_wrap_mode {
                caret.lf(self, layer);
            } else {
                caret.pos.x -= 1;
            }
        }
    }

    fn scroll_up(&mut self, layer: usize) {
        let start_line: i32 = self.get_first_editable_line();
        let end_line = self.get_last_editable_line();

        let start_column = self.get_first_editable_column();
        let end_column = self.get_last_editable_column();

        let layer = &mut self.layers[layer];
        for x in start_column..=end_column {
            (start_line..end_line).for_each(|y| {
                let ch = layer.get_char((x, y + 1));
                layer.set_char((x, y), ch);
            });
            layer.set_char((x, end_line), AttributedChar::default());
        }
    }

    fn scroll_down(&mut self, layer: usize) {
        let start_line: i32 = self.get_first_editable_line();
        let end_line = self.get_last_editable_line();

        let start_column = self.get_first_editable_column();
        let end_column = self.get_last_editable_column();

        let layer = &mut self.layers[layer];
        for x in start_column..=end_column {
            ((start_line + 1)..=end_line).rev().for_each(|y| {
                let ch = layer.get_char((x, y - 1));
                layer.set_char((x, y), ch);
            });
            layer.set_char((x, start_line), AttributedChar::default());
        }
    }

    fn scroll_left(&mut self, layer: usize) {
        let start_line: i32 = self.get_first_editable_line();
        let end_line = self.get_last_editable_line();

        let start_column = self.get_first_editable_column() as usize;
        let end_column = self.get_last_editable_column() + 1;

        let layer = &mut self.layers[layer];
        for i in start_line..=end_line {
            let line = &mut layer.lines[i as usize];
            if line.chars.len() > start_column {
                line.chars.insert(end_column as usize, AttributedChar::default());
                line.chars.remove(start_column);
            }
        }
    }

    fn scroll_right(&mut self, layer: usize) {
        let start_line = self.get_first_editable_line();
        let end_line = self.get_last_editable_line();

        let start_column = self.get_first_editable_column() as usize;
        let end_column = self.get_last_editable_column() as usize;

        let layer = &mut self.layers[layer];
        for i in start_line..=end_line {
            let line = &mut layer.lines[i as usize];
            if line.chars.len() > start_column {
                line.chars.insert(start_column, AttributedChar::default());
                line.chars.remove(end_column + 1);
            }
        }
    }

    pub fn clear_screen(&mut self, layer: usize, caret: &mut Caret) {
        caret.pos = Position::default();
        let layer = &mut self.layers[layer];
        layer.clear();
        self.stop_sixel_threads();
        self.terminal_state.cleared_screen = true;
        if self.is_terminal_buffer {
            self.set_size(self.terminal_state.get_size());
        }
    }

    fn clear_buffer_down(&mut self, layer: usize, caret: &Caret) {
        let pos = caret.get_position();
        let ch: AttributedChar = AttributedChar {
            attribute: caret.attribute,
            ..Default::default()
        };

        for y in pos.y..self.get_last_visible_line() {
            for x in 0..self.get_width() {
                self.layers[layer].set_char((x, y), ch);
            }
        }
    }

    fn clear_buffer_up(&mut self, layer: usize, caret: &Caret) {
        let pos = caret.get_position();
        let ch: AttributedChar = AttributedChar {
            attribute: caret.attribute,
            ..Default::default()
        };

        for y in self.get_first_visible_line()..pos.y {
            for x in 0..self.get_width() {
                self.layers[layer].set_char((x, y), ch);
            }
        }
    }

    fn clear_line(&mut self, layer: usize, caret: &Caret) {
        let mut pos = caret.get_position();
        let ch: AttributedChar = AttributedChar {
            attribute: caret.attribute,
            ..Default::default()
        };
        for x in 0..self.get_width() {
            pos.x = x;
            self.layers[layer].set_char(pos, ch);
        }
    }

    fn clear_line_end(&mut self, layer: usize, caret: &Caret) {
        let mut pos = caret.get_position();
        let ch: AttributedChar = AttributedChar {
            attribute: caret.attribute,
            ..Default::default()
        };
        for x in pos.x..self.get_width() {
            pos.x = x;
            self.layers[layer].set_char(pos, ch);
        }
    }

    fn clear_line_start(&mut self, layer: usize, caret: &Caret) {
        let mut pos = caret.get_position();
        let ch: AttributedChar = AttributedChar {
            attribute: caret.attribute,
            ..Default::default()
        };
        for x in 0..pos.x {
            pos.x = x;
            self.layers[layer].set_char(pos, ch);
        }
    }

    fn remove_terminal_line(&mut self, layer: usize, line: i32) {
        if line >= self.layers[layer].get_line_count() {
            return;
        }
        self.layers[layer].remove_line(line);
        if let Some((_, end)) = self.terminal_state.get_margins_top_bottom() {
            let buffer_width = self.layers[layer].get_width();
            self.layers[layer].insert_line(end, Line::with_capacity(buffer_width));
        }
    }

    fn insert_terminal_line(&mut self, layer: usize, line: i32) {
        if let Some((_, end)) = self.terminal_state.get_margins_top_bottom() {
            if end < self.layers[layer].get_line_count() {
                self.layers[layer].lines.remove(end as usize);
            }
        }
        let buffer_width = self.layers[layer].get_width();
        self.layers[layer].insert_line(line, Line::with_capacity(buffer_width));
    }
}

#[cfg(test)]
fn create_buffer<T: BufferParser>(parser: &mut T, input: &[u8]) -> (Buffer, Caret) {
    let mut buf: Buffer = Buffer::create((80, 25));
    buf.is_terminal_buffer = true;
    let mut caret = Caret::default();
    buf.layers.first_mut().unwrap().lines.clear();

    update_buffer(&mut buf, &mut caret, parser, input);

    (buf, caret)
}

#[cfg(test)]
fn update_buffer<T: BufferParser>(buf: &mut Buffer, caret: &mut Caret, parser: &mut T, input: &[u8]) {
    for b in input {
        parser.print_char(buf, 0, caret, *b as char).unwrap(); // test code
    }
}

#[cfg(test)]
fn update_buffer_force<T: BufferParser>(buf: &mut Buffer, caret: &mut Caret, parser: &mut T, input: &[u8]) {
    for b in input {
        let _ = parser.print_char(buf, 0, caret, *b as char); // test code
    }
}

#[cfg(test)]
fn get_simple_action<T: BufferParser>(parser: &mut T, input: &[u8]) -> CallbackAction {
    let mut buf = Buffer::create((80, 25));
    let mut caret = Caret::default();
    buf.is_terminal_buffer = true;

    get_action(&mut buf, &mut caret, parser, input)
}

#[cfg(test)]
fn get_action<T: BufferParser>(buf: &mut Buffer, caret: &mut Caret, parser: &mut T, input: &[u8]) -> CallbackAction {
    let mut action = CallbackAction::NoUpdate;
    for b in input {
        action = parser.print_char(buf, 0, caret, *b as char).unwrap(); // test code
    }

    action
}
