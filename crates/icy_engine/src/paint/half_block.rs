use crate::{AttributeColor, AttributedChar, Position, TextAttribute, TextPane};

pub(crate) const FULL_BLOCK: char = 219 as char;
pub(crate) const HALF_BLOCK_TOP: char = 223 as char;
pub(crate) const HALF_BLOCK_BOTTOM: char = 220 as char;
pub(crate) const EMPTY_BLOCK1: char = 0 as char;
pub(crate) const EMPTY_BLOCK2: char = 255 as char;
pub(crate) const LEFT_BLOCK: char = 221 as char;
pub(crate) const RIGHT_BLOCK: char = 222 as char;

fn flip_colors(attribute: TextAttribute) -> TextAttribute {
    let mut result = attribute;
    result.set_foreground_color(attribute.background_color());
    result.set_background_color(attribute.foreground_color());
    result
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum HalfBlockType {
    None,
    Upper,
    Lower,
    Full,
    Empty,

    Left,
    Right,
}

#[derive(Debug)]
pub struct HalfBlock {
    pub ch: AttributedChar,
    pub upper_block_color: AttributeColor,
    pub lower_block_color: AttributeColor,
    pub left_block_color: AttributeColor,
    pub right_block_color: AttributeColor,
    pub is_top: bool,
    pub block_type: HalfBlockType,
}

impl HalfBlock {
    pub fn from<T: TextPane>(buf: &T, pos: Position) -> Self {
        let ch = buf.char_at(Position::new(pos.x, pos.y / 2));
        Self::from_char(ch, pos)
    }

    pub fn from_char(ch: AttributedChar, pos: Position) -> Self {
        let is_top = pos.y % 2 == 0;
        let mut upper_block_color = AttributeColor::Palette(0);
        let mut lower_block_color = AttributeColor::Palette(0);
        let mut left_block_color = AttributeColor::Palette(0);
        let mut right_block_color = AttributeColor::Palette(0);
        let block_type;
        match ch.ch {
            EMPTY_BLOCK1 | ' ' | EMPTY_BLOCK2 => {
                upper_block_color = ch.attribute.background_color();
                lower_block_color = ch.attribute.background_color();
                block_type = HalfBlockType::Empty;
            }
            HALF_BLOCK_BOTTOM => {
                upper_block_color = ch.attribute.background_color();
                lower_block_color = ch.attribute.foreground_color();
                block_type = HalfBlockType::Lower;
            }
            HALF_BLOCK_TOP => {
                upper_block_color = ch.attribute.foreground_color();
                lower_block_color = ch.attribute.background_color();
                block_type = HalfBlockType::Upper;
            }
            FULL_BLOCK => {
                upper_block_color = ch.attribute.foreground_color();
                lower_block_color = ch.attribute.foreground_color();
                block_type = HalfBlockType::Full;
            }
            LEFT_BLOCK => {
                left_block_color = ch.attribute.foreground_color();
                right_block_color = ch.attribute.background_color();
                block_type = HalfBlockType::Left;
            }
            RIGHT_BLOCK => {
                left_block_color = ch.attribute.background_color();
                right_block_color = ch.attribute.foreground_color();
                block_type = HalfBlockType::Right;
            }
            _ => {
                if ch.attribute.background_color() == ch.attribute.foreground_color() {
                    upper_block_color = ch.attribute.foreground_color();
                    lower_block_color = ch.attribute.foreground_color();
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
        match self.block_type {
            HalfBlockType::Upper | HalfBlockType::Lower | HalfBlockType::Full | HalfBlockType::Empty => true,
            _ => false,
        }
    }

    pub fn is_vertically_blocky(&self) -> bool {
        self.block_type == HalfBlockType::Left || self.block_type == HalfBlockType::Right
    }

    pub fn get_half_block_char(&self, col: AttributeColor, transparent_color: bool) -> AttributedChar {
        let transparent_color = self.ch.is_transparent() && transparent_color;

        let block = if self.is_blocky() {
            if self.is_top && self.lower_block_color == col || !self.is_top && self.upper_block_color == col {
                AttributedChar::new(FULL_BLOCK, TextAttribute::from_colors(col, AttributeColor::Palette(0)))
            } else if self.is_top {
                AttributedChar::new(
                    HALF_BLOCK_TOP,
                    TextAttribute::from_colors(
                        col,
                        if transparent_color {
                            AttributeColor::Transparent
                        } else {
                            self.lower_block_color
                        },
                    ),
                )
            } else {
                AttributedChar::new(
                    HALF_BLOCK_BOTTOM,
                    TextAttribute::from_colors(
                        col,
                        if transparent_color {
                            AttributeColor::Transparent
                        } else {
                            self.upper_block_color
                        },
                    ),
                )
            }
        } else {
            if self.is_top {
                AttributedChar::new(
                    HALF_BLOCK_TOP,
                    TextAttribute::from_colors(
                        col,
                        if transparent_color {
                            AttributeColor::Transparent
                        } else {
                            self.ch.attribute.background_color()
                        },
                    ),
                )
            } else {
                AttributedChar::new(
                    HALF_BLOCK_BOTTOM,
                    TextAttribute::from_colors(
                        col,
                        if transparent_color {
                            AttributeColor::Transparent
                        } else {
                            self.ch.attribute.background_color()
                        },
                    ),
                )
            }
        };
        self.optimize_block(block)
    }

    fn optimize_block(&self, mut block: AttributedChar) -> AttributedChar {
        if block.attribute.foreground() == 0 {
            if block.attribute.background() == 0 || block.ch == FULL_BLOCK {
                block.ch = ' ';
                block.attribute = TextAttribute::default();
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
        } else if block.attribute.foreground() < 8 && block.attribute.background() >= 8 {
            if self.is_blocky() {
                match block.ch {
                    HALF_BLOCK_BOTTOM => {
                        return AttributedChar::new(HALF_BLOCK_TOP, flip_colors(block.attribute));
                    }
                    HALF_BLOCK_TOP => {
                        return AttributedChar::new(HALF_BLOCK_BOTTOM, flip_colors(block.attribute));
                    }
                    _ => {}
                }
            } else if self.is_vertically_blocky() {
                match block.ch {
                    LEFT_BLOCK => {
                        return AttributedChar::new(RIGHT_BLOCK, flip_colors(block.attribute));
                    }
                    RIGHT_BLOCK => {
                        return AttributedChar::new(LEFT_BLOCK, flip_colors(block.attribute));
                    }
                    _ => {}
                }
            }
        }
        block
    }
}
