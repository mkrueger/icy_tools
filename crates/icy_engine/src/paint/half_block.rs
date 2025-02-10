use crate::{AttributedChar, Buffer, Position, TextAttribute, TextPane};

pub(crate) const FULL_BLOCK: char = 219 as char;
pub(crate) const HALF_BLOCK_TOP: char = 223 as char;
pub(crate) const HALF_BLOCK_BOTTOM: char = 220 as char;
pub(crate) const EMPTY_BLOCK1: char = 0 as char;
pub(crate) const EMPTY_BLOCK2: char = 255 as char;
pub(crate) const LEFT_BLOCK: char = 221 as char;
pub(crate) const RIGHT_BLOCK: char = 222 as char;

pub(crate) struct HalfBlock {
    pub upper_block_color: u32,
    pub lower_block_color: u32,
    pub is_top: bool,
}

impl HalfBlock {
    pub fn from(buf: &Buffer, block: AttributedChar, pos: Position) -> Self {
        let is_top = pos.y % 2 == 0;

        let Some(font) = buf.get_font(block.get_font_page()) else {
            return Self {
                upper_block_color: block.attribute.get_background(),
                lower_block_color: block.attribute.get_background(),
                is_top,
            };
        };

        let Some(glyph) = font.get_glyph(block.ch) else {
            return Self {
                upper_block_color: block.attribute.get_background(),
                lower_block_color: block.attribute.get_background(),
                is_top,
            };
        };

        let mut upper = 0;
        let mut lower = 0;
        for i in 0..(glyph.data.len() / 2) {
            upper += glyph.data[i].count_ones() as i32;
            lower += glyph.data[glyph.data.len() / 2 + i].count_ones() as i32;
        }
        let upper_block_color = if upper > font.size.width * font.size.height / 4 {
            block.attribute.get_foreground()
        } else {
            block.attribute.get_background()
        };
        let lower_block_color = if lower > font.size.width * font.size.height / 4 {
            block.attribute.get_foreground()
        } else {
            block.attribute.get_background()
        };

        Self {
            upper_block_color,
            lower_block_color,
            is_top,
        }
    }
}

pub fn get_halfblock(buf: &Buffer, cur_char: AttributedChar, pos: Position, color: u32, transparent_color: bool) -> AttributedChar {
    let half_block = HalfBlock::from(buf, cur_char, pos);
    let transparent_color = cur_char.is_transparent() && transparent_color;

    let ch = if (half_block.is_top && half_block.lower_block_color == color) || (!half_block.is_top && half_block.upper_block_color == color) {
        AttributedChar::new(FULL_BLOCK, TextAttribute::new(color, 0))
    } else if half_block.is_top {
        AttributedChar::new(
            HALF_BLOCK_TOP,
            TextAttribute::new(
                color,
                if transparent_color {
                    TextAttribute::TRANSPARENT_COLOR
                } else {
                    half_block.lower_block_color
                },
            ),
        )
    } else {
        AttributedChar::new(
            HALF_BLOCK_BOTTOM,
            TextAttribute::new(
                color,
                if transparent_color {
                    TextAttribute::TRANSPARENT_COLOR
                } else {
                    half_block.upper_block_color
                },
            ),
        )
    };

    optimize_block(ch)
}

fn flip_colors(attribute: TextAttribute) -> TextAttribute {
    let mut result = attribute;
    result.set_foreground(attribute.get_background());
    result.set_background(attribute.get_foreground());
    result
}

fn optimize_block(mut block: AttributedChar) -> AttributedChar {
    if block.attribute.get_foreground() == 0 {
        if block.attribute.get_background() == 0 || block.ch == FULL_BLOCK {
            block.ch = ' ';
            return block;
        }
        match block.ch {
            HALF_BLOCK_BOTTOM => {
                return AttributedChar::new(HALF_BLOCK_TOP, flip_colors(block.attribute));
            }
            HALF_BLOCK_TOP => {
                return AttributedChar::new(HALF_BLOCK_BOTTOM, flip_colors(block.attribute));
            }
            _ => {}
        }
    } else if block.attribute.get_foreground() < 8 && block.attribute.get_background() >= 8 {
        match block.ch {
            HALF_BLOCK_BOTTOM => {
                return AttributedChar::new(HALF_BLOCK_TOP, flip_colors(block.attribute));
            }
            HALF_BLOCK_TOP => {
                return AttributedChar::new(HALF_BLOCK_BOTTOM, flip_colors(block.attribute));
            }
            _ => {}
        }
    }
    block
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum HalfBlockType {
    None,
    Upper,
    Lower,
    Left,
    Right,
    Empty,
    Full,
}

#[derive(Debug)]
pub struct HalfBlockInfo {
    pub ch: AttributedChar,
    pub upper_block_color: u32,
    pub lower_block_color: u32,
    pub left_block_color: u32,
    pub right_block_color: u32,
    pub is_top: bool,
    pub block_type: HalfBlockType,
}

impl HalfBlockInfo {
    pub fn from<T: TextPane>(buf: &T, pos: Position) -> Self {
        let is_top = pos.y % 2 == 0;
        let ch = buf.get_char(Position::new(pos.x, pos.y / 2));
        let mut upper_block_color = 0;
        let mut lower_block_color = 0;
        let mut left_block_color = 0;
        let mut right_block_color = 0;
        let block_type;
        match ch.ch {
            EMPTY_BLOCK1 | ' ' | EMPTY_BLOCK2 => {
                upper_block_color = ch.attribute.background_color;
                lower_block_color = ch.attribute.background_color;
                block_type = HalfBlockType::Empty;
            }
            HALF_BLOCK_BOTTOM => {
                upper_block_color = ch.attribute.background_color;
                lower_block_color = ch.attribute.foreground_color;
                block_type = HalfBlockType::Lower;
            }
            HALF_BLOCK_TOP => {
                upper_block_color = ch.attribute.foreground_color;
                lower_block_color = ch.attribute.background_color;
                block_type = HalfBlockType::Upper;
            }
            FULL_BLOCK => {
                upper_block_color = ch.attribute.foreground_color;
                lower_block_color = ch.attribute.foreground_color;
                block_type = HalfBlockType::Full;
            }
            LEFT_BLOCK => {
                left_block_color = ch.attribute.foreground_color;
                right_block_color = ch.attribute.background_color;
                block_type = HalfBlockType::Left;
            }
            RIGHT_BLOCK => {
                left_block_color = ch.attribute.background_color;
                right_block_color = ch.attribute.foreground_color;
                block_type = HalfBlockType::Right;
            }
            _ => {
                if ch.attribute.background_color == ch.attribute.foreground_color {
                    upper_block_color = ch.attribute.foreground_color;
                    lower_block_color = ch.attribute.foreground_color;
                    block_type = HalfBlockType::Full;
                } else {
                    block_type = HalfBlockType::None;
                }
            }
        }
        Self {
            ch,
            upper_block_color,
            lower_block_color,
            left_block_color,
            right_block_color,
            is_top,
            block_type,
        }
    }

    pub fn is_blocky(&self) -> bool {
        self.block_type != HalfBlockType::None
    }

    pub fn is_vertically_blocky(&self) -> bool {
        self.block_type == HalfBlockType::Left || self.block_type == HalfBlockType::Right
    }

    pub fn get_half_block_char(&self, col: u32) -> AttributedChar {
        let block = if self.is_blocky() {
            if self.is_top && self.lower_block_color == col || !self.is_blocky() && self.upper_block_color == col {
                AttributedChar::new(FULL_BLOCK, TextAttribute::new(col, 0))
            } else if self.is_top {
                AttributedChar::new(HALF_BLOCK_TOP, TextAttribute::new(col, self.lower_block_color))
            } else {
                AttributedChar::new(HALF_BLOCK_BOTTOM, TextAttribute::new(col, self.upper_block_color))
            }
        } else {
            if self.is_top {
                AttributedChar::new(HALF_BLOCK_TOP, TextAttribute::new(col, self.ch.attribute.background_color))
            } else {
                AttributedChar::new(HALF_BLOCK_BOTTOM, TextAttribute::new(col, self.ch.attribute.background_color))
            }
        };
        optimize_block(block)
    }
}
