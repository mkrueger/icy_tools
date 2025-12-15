//! Common test target for brush tests

use icy_engine::{AttributedChar, Position, TextAttribute};
use icy_engine_edit::brushes::DrawTarget;

/// A simple test target that records all drawing operations
pub struct TestTarget {
    width: i32,
    height: i32,
    chars: Vec<AttributedChar>,
}

impl TestTarget {
    pub fn new(width: i32, height: i32) -> Self {
        Self {
            width,
            height,
            chars: vec![AttributedChar::new(' ', TextAttribute::default()); (width * height) as usize],
        }
    }

    pub fn get_at(&self, x: i32, y: i32) -> AttributedChar {
        if x >= 0 && x < self.width && y >= 0 && y < self.height {
            self.chars[(y * self.width + x) as usize]
        } else {
            AttributedChar::default()
        }
    }
}

impl DrawTarget for TestTarget {
    fn width(&self) -> i32 {
        self.width
    }

    fn height(&self) -> i32 {
        self.height
    }

    fn char_at(&self, pos: Position) -> Option<AttributedChar> {
        if self.is_valid(pos) {
            Some(self.chars[(pos.y * self.width + pos.x) as usize])
        } else {
            None
        }
    }

    fn set_char(&mut self, pos: Position, ch: AttributedChar) {
        if self.is_valid(pos) {
            self.chars[(pos.y * self.width + pos.x) as usize] = ch;
        }
    }
}
