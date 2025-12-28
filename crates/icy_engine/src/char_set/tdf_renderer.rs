use crate::{AttributedChar, Position, TextAttribute, TextBuffer, TextPane};
use retrofont::{Cell, FontTarget};

/// A renderer that writes to a `TextBuffer`
pub struct TdfBufferRenderer<'a> {
    buffer: &'a mut TextBuffer,
    cur_x: i32,
    cur_y: i32,
    start_x: i32,
    start_y: i32,
}

impl<'a> TdfBufferRenderer<'a> {
    pub fn new(buffer: &'a mut TextBuffer, start_x: i32, start_y: i32) -> Self {
        Self {
            buffer,
            cur_x: start_x,
            cur_y: start_y,
            start_x,
            start_y,
        }
    }

    /// Reset to the next character position (advances X, resets Y to start)
    pub fn next_char(&mut self) {
        // Find the maximum X used so far
        self.start_x = self.cur_x;
        self.cur_y = self.start_y;
    }
}

impl FontTarget for TdfBufferRenderer<'_> {
    type Error = std::fmt::Error;

    fn draw(&mut self, cell: Cell) -> std::result::Result<(), Self::Error> {
        if self.cur_x >= 0 && self.cur_x < self.buffer.width() && self.cur_y >= 0 && self.cur_y < self.buffer.height() {
            let fg = cell.fg.unwrap_or(15);
            let bg = cell.bg.unwrap_or(0);
            let attr = TextAttribute::from_color(fg, bg);

            self.buffer.layers[0].set_char(
                Position::new(self.cur_x, self.cur_y),
                AttributedChar::new(self.buffer.buffer_type.convert_from_unicode(cell.ch), attr),
            );
        }
        self.cur_x += 1;
        Ok(())
    }

    fn skip(&mut self) -> std::result::Result<(), Self::Error> {
        self.cur_x += 1;
        Ok(())
    }

    fn next_line(&mut self) -> std::result::Result<(), Self::Error> {
        self.cur_y += 1;
        self.cur_x = self.start_x;
        Ok(())
    }
}
