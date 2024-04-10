use crate::{Position, AttributedChar};

pub trait Layer {
    fn get_char(&self, pos: Position) -> AttributedChar;
    fn get_width(&self) -> i32;
    fn get_line_count(&self) -> i32;
    fn get_line_length(&self, line: i32) -> i32;

    fn get_char_xy(&self, x: i32, y: i32) -> AttributedChar {
        self.get_char(Position::new(x, y))
    }

    fn set_char(&mut self, pos: Position, attributed_char: AttributedChar);

    fn set_char_xy(&mut self, x: i32, y: i32, attributed_char: AttributedChar) {
        self.set_char( Position::new(x, y), attributed_char);
    }
}

