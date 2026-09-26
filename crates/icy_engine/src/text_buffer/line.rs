use serde::{Deserialize, Serialize};

use super::AttributedChar;
use crate::AttributeColor;

#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
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

    /// Returns true if no cell of this line would show up when rendered.
    ///
    /// Unlike `AttributedChar::is_transparent()` (which treats every space as
    /// transparent), a space on a coloured background counts as content here,
    /// so rows like a closing colour bar survive `line_count()` and export.
    pub(crate) fn is_effective_empty(&self) -> bool {
        !self.chars.iter().any(|ch| is_visible_content(*ch))
    }
}

/// Returns true if the cell renders visibly on the default (black) screen.
fn is_visible_content(ch: AttributedChar) -> bool {
    if !ch.is_visible() {
        return false;
    }
    let attr = ch.attribute;
    let bg = default_to_black(attr.background_color());
    // A coloured background is visible even behind a blank glyph.
    if bg != AttributeColor::Palette(0) {
        return true;
    }
    // In iCE mode blink selects the bright background (DOS colour 8 for black).
    if attr.is_blinking() {
        return true;
    }
    let is_blank = ch.ch == '\0' || ch.ch == ' ';
    !is_blank && default_to_black(attr.foreground_color()) != bg
}

fn default_to_black(color: AttributeColor) -> AttributeColor {
    if color.is_transparent() {
        AttributeColor::Palette(0)
    } else {
        color
    }
}
