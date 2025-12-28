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
        matches!(
            self.block_type,
            HalfBlockType::Upper | HalfBlockType::Lower | HalfBlockType::Full | HalfBlockType::Empty
        )
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
        } else if self.is_top {
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

#[cfg(test)]
mod tests {
    use super::*;

    // ==================== HalfBlock Parsing Tests ====================

    #[test]
    fn test_from_char_empty_block() {
        let ch = AttributedChar::new(' ', TextAttribute::from_colors(AttributeColor::Palette(7), AttributeColor::Palette(1)));
        let pos = Position::new(0, 0); // top half
        let block = HalfBlock::from_char(ch, pos);

        assert_eq!(block.block_type, HalfBlockType::Empty);
        assert_eq!(block.upper_block_color, AttributeColor::Palette(1)); // background
        assert_eq!(block.lower_block_color, AttributeColor::Palette(1)); // background
        assert!(block.is_top);
        assert!(block.is_blocky());
    }

    #[test]
    fn test_from_char_full_block() {
        let ch = AttributedChar::new(FULL_BLOCK, TextAttribute::from_colors(AttributeColor::Palette(4), AttributeColor::Palette(0)));
        let pos = Position::new(0, 0);
        let block = HalfBlock::from_char(ch, pos);

        assert_eq!(block.block_type, HalfBlockType::Full);
        assert_eq!(block.upper_block_color, AttributeColor::Palette(4)); // foreground
        assert_eq!(block.lower_block_color, AttributeColor::Palette(4)); // foreground
        assert!(block.is_blocky());
    }

    #[test]
    fn test_from_char_half_block_top() {
        let ch = AttributedChar::new(
            HALF_BLOCK_TOP,
            TextAttribute::from_colors(AttributeColor::Palette(2), AttributeColor::Palette(5)),
        );
        let pos_top = Position::new(0, 0);
        let block_top = HalfBlock::from_char(ch, pos_top);

        assert_eq!(block_top.block_type, HalfBlockType::Upper);
        assert_eq!(block_top.upper_block_color, AttributeColor::Palette(2)); // foreground
        assert_eq!(block_top.lower_block_color, AttributeColor::Palette(5)); // background
        assert!(block_top.is_top);

        let pos_bottom = Position::new(0, 1);
        let block_bottom = HalfBlock::from_char(ch, pos_bottom);
        assert!(!block_bottom.is_top);
    }

    #[test]
    fn test_from_char_half_block_bottom() {
        let ch = AttributedChar::new(
            HALF_BLOCK_BOTTOM,
            TextAttribute::from_colors(AttributeColor::Palette(3), AttributeColor::Palette(6)),
        );
        let pos = Position::new(0, 0);
        let block = HalfBlock::from_char(ch, pos);

        assert_eq!(block.block_type, HalfBlockType::Lower);
        assert_eq!(block.upper_block_color, AttributeColor::Palette(6)); // background
        assert_eq!(block.lower_block_color, AttributeColor::Palette(3)); // foreground
    }

    #[test]
    fn test_from_char_left_block() {
        let ch = AttributedChar::new(LEFT_BLOCK, TextAttribute::from_colors(AttributeColor::Palette(1), AttributeColor::Palette(2)));
        let pos = Position::new(0, 0);
        let block = HalfBlock::from_char(ch, pos);

        assert_eq!(block.block_type, HalfBlockType::Left);
        assert_eq!(block.left_block_color, AttributeColor::Palette(1)); // foreground
        assert_eq!(block.right_block_color, AttributeColor::Palette(2)); // background
        assert!(block.is_vertically_blocky());
        assert!(!block.is_blocky());
    }

    #[test]
    fn test_from_char_right_block() {
        let ch = AttributedChar::new(RIGHT_BLOCK, TextAttribute::from_colors(AttributeColor::Palette(3), AttributeColor::Palette(4)));
        let pos = Position::new(0, 0);
        let block = HalfBlock::from_char(ch, pos);

        assert_eq!(block.block_type, HalfBlockType::Right);
        assert_eq!(block.left_block_color, AttributeColor::Palette(4)); // background
        assert_eq!(block.right_block_color, AttributeColor::Palette(3)); // foreground
        assert!(block.is_vertically_blocky());
    }

    // ==================== Transparent Block Tests ====================

    #[test]
    fn test_from_char_transparent_invisible() {
        let ch = AttributedChar::invisible();
        let pos = Position::new(0, 0);
        let block = HalfBlock::from_char(ch, pos);

        // Invisible char has transparent colors
        assert_eq!(block.upper_block_color, AttributeColor::Transparent);
        assert_eq!(block.lower_block_color, AttributeColor::Transparent);
        assert_eq!(block.block_type, HalfBlockType::Empty);
        assert!(block.is_blocky());
    }

    #[test]
    fn test_transparent_vs_colored_detection() {
        // A transparent cell
        let transparent_ch = AttributedChar::invisible();
        let transparent_block = HalfBlock::from_char(transparent_ch, Position::new(0, 0));

        // A colored cell (red foreground on black background)
        let colored_ch = AttributedChar::new(FULL_BLOCK, TextAttribute::from_colors(AttributeColor::Palette(4), AttributeColor::Palette(0)));
        let colored_block = HalfBlock::from_char(colored_ch, Position::new(0, 0));

        // They should have different colors - this is key for fill boundaries
        assert_eq!(transparent_block.upper_block_color, AttributeColor::Transparent);
        assert_eq!(colored_block.upper_block_color, AttributeColor::Palette(4));
        assert_ne!(transparent_block.upper_block_color, colored_block.upper_block_color);
    }

    #[test]
    fn test_transparent_boundary_detection() {
        // Simulate fill boundary detection:
        // When filling transparent areas, a non-transparent cell should be a boundary
        let target_color = AttributeColor::Transparent;

        // Transparent cell - should match target (fill continues)
        let transparent_ch = AttributedChar::invisible();
        let transparent_block = HalfBlock::from_char(transparent_ch, Position::new(0, 0));
        assert_eq!(transparent_block.upper_block_color, target_color);

        // Colored cell - should NOT match target (fill stops)
        let barrier_ch = AttributedChar::new(' ', TextAttribute::from_colors(AttributeColor::Palette(7), AttributeColor::Palette(1)));
        let barrier_block = HalfBlock::from_char(barrier_ch, Position::new(0, 0));
        assert_ne!(barrier_block.upper_block_color, target_color);

        // Any visible block character - should be a barrier
        let full_block_ch = AttributedChar::new(FULL_BLOCK, TextAttribute::from_colors(AttributeColor::Palette(4), AttributeColor::Palette(0)));
        let full_block = HalfBlock::from_char(full_block_ch, Position::new(0, 0));
        assert_ne!(full_block.upper_block_color, target_color);
    }

    // ==================== Block Type Detection Tests ====================

    #[test]
    fn test_is_blocky_variants() {
        // Horizontal half blocks should be blocky
        let empty = HalfBlock::from_char(AttributedChar::new(' ', TextAttribute::default()), Position::new(0, 0));
        assert!(empty.is_blocky());

        let full = HalfBlock::from_char(AttributedChar::new(FULL_BLOCK, TextAttribute::default()), Position::new(0, 0));
        assert!(full.is_blocky());

        let top = HalfBlock::from_char(AttributedChar::new(HALF_BLOCK_TOP, TextAttribute::default()), Position::new(0, 0));
        assert!(top.is_blocky());

        let bottom = HalfBlock::from_char(AttributedChar::new(HALF_BLOCK_BOTTOM, TextAttribute::default()), Position::new(0, 0));
        assert!(bottom.is_blocky());
    }

    #[test]
    fn test_is_vertically_blocky_variants() {
        // Vertical half blocks should be vertically blocky (but not blocky)
        let left = HalfBlock::from_char(AttributedChar::new(LEFT_BLOCK, TextAttribute::default()), Position::new(0, 0));
        assert!(left.is_vertically_blocky());
        assert!(!left.is_blocky());

        let right = HalfBlock::from_char(AttributedChar::new(RIGHT_BLOCK, TextAttribute::default()), Position::new(0, 0));
        assert!(right.is_vertically_blocky());
        assert!(!right.is_blocky());
    }

    #[test]
    fn test_non_block_character() {
        // Regular ASCII character with different fg/bg should not be blocky
        let letter = HalfBlock::from_char(
            AttributedChar::new('A', TextAttribute::from_colors(AttributeColor::Palette(7), AttributeColor::Palette(0))),
            Position::new(0, 0),
        );
        assert_eq!(letter.block_type, HalfBlockType::None);
        assert!(!letter.is_blocky());
        assert!(!letter.is_vertically_blocky());
    }

    #[test]
    fn test_same_fg_bg_becomes_full() {
        // Character with same fg and bg is treated as full block
        let same_colors = HalfBlock::from_char(
            AttributedChar::new('X', TextAttribute::from_colors(AttributeColor::Palette(5), AttributeColor::Palette(5))),
            Position::new(0, 0),
        );
        assert_eq!(same_colors.block_type, HalfBlockType::Full);
        assert!(same_colors.is_blocky());
    }

    // ==================== Position-based Tests ====================

    #[test]
    fn test_is_top_based_on_position() {
        let ch = AttributedChar::new(HALF_BLOCK_TOP, TextAttribute::default());

        // Even y positions (0, 2, 4, ...) are top half
        assert!(HalfBlock::from_char(ch, Position::new(0, 0)).is_top);
        assert!(HalfBlock::from_char(ch, Position::new(0, 2)).is_top);
        assert!(HalfBlock::from_char(ch, Position::new(0, 4)).is_top);

        // Odd y positions (1, 3, 5, ...) are bottom half
        assert!(!HalfBlock::from_char(ch, Position::new(0, 1)).is_top);
        assert!(!HalfBlock::from_char(ch, Position::new(0, 3)).is_top);
        assert!(!HalfBlock::from_char(ch, Position::new(0, 5)).is_top);
    }

    // ==================== get_half_block_char Tests ====================

    #[test]
    fn test_get_half_block_char_fill_top() {
        // Empty cell, fill top half with red
        let ch = AttributedChar::new(' ', TextAttribute::from_colors(AttributeColor::Palette(0), AttributeColor::Palette(0)));
        let pos = Position::new(0, 0); // top half
        let block = HalfBlock::from_char(ch, pos);

        let result = block.get_half_block_char(AttributeColor::Palette(4), false);
        // Should create a half block with red top
        assert_eq!(result.ch, HALF_BLOCK_TOP);
    }

    #[test]
    fn test_get_half_block_char_fill_bottom() {
        // Empty cell, fill bottom half with blue
        let ch = AttributedChar::new(' ', TextAttribute::from_colors(AttributeColor::Palette(0), AttributeColor::Palette(0)));
        let pos = Position::new(0, 1); // bottom half
        let block = HalfBlock::from_char(ch, pos);

        let result = block.get_half_block_char(AttributeColor::Palette(1), false);
        // Should create a half block with blue bottom
        assert_eq!(result.ch, HALF_BLOCK_BOTTOM);
    }

    #[test]
    fn test_get_half_block_char_merge_to_full() {
        // Half block with red top, fill bottom with red -> should become full block
        let ch = AttributedChar::new(
            HALF_BLOCK_TOP,
            TextAttribute::from_colors(AttributeColor::Palette(4), AttributeColor::Palette(0)),
        );
        let pos = Position::new(0, 1); // bottom half
        let block = HalfBlock::from_char(ch, pos);

        let result = block.get_half_block_char(AttributeColor::Palette(4), false);
        assert_eq!(result.ch, FULL_BLOCK);
    }
}
