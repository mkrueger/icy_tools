use super::AttributedChar;
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Line {
    pub chars: Vec<AttributedChar>,
}

impl Line {
    pub fn new() -> Self {
        Line::with_capacity(80)
    }

    pub fn with_capacity(capacity: i32) -> Self {
        Line {
            chars: Vec::with_capacity(capacity as usize),
        }
    }

    pub fn create(width: i32) -> Self {
        let mut chars = Vec::new();
        chars.resize(width as usize, AttributedChar::invisible());
        Line { chars }
    }

    pub fn line_length(&self) -> i32 {
        for idx in (0..self.chars.len()).rev() {
            if !self.chars[idx].is_transparent() {
                return idx as i32 + 1;
            }
        }
        0
    }

    pub fn insert_char(&mut self, index: i32, char_opt: AttributedChar) {
        if index > self.chars.len() as i32 {
            self.chars.resize(index as usize, AttributedChar::invisible());
        }
        self.chars.insert(index as usize, char_opt);
    }

    pub fn set_char(&mut self, index: i32, char: AttributedChar) {
        if index >= self.chars.len() as i32 {
            self.chars.resize(index as usize + 1, AttributedChar::invisible());
        }
        self.chars[index as usize] = char;
    }

    pub(crate) fn is_effective_empty(&self) -> bool {
        if self.chars.is_empty() {
            return true;
        }
        for ch in &self.chars {
            if !ch.is_transparent() {
                return false;
            }
        }
        true
    }
}
