//! Brush modes for drawing operations

use std::sync::Arc;

/// The brush mode determines how pixels/characters are drawn
#[derive(Clone, Debug, PartialEq)]
pub enum BrushMode {
    /// Draw full block characters (█)
    Block,
    /// Draw half-block characters for higher resolution (▀▄█ etc.)
    HalfBlock,
    /// Draw outline characters using TheDraw font outlines
    Outline,
    /// Draw a specific character
    Char(char),

    /// Replace only the character, keeping existing attributes
    Replace(char),
    /// Shade mode - increases shade level on each stroke
    Shade,
    /// Shade down - decreases shade level on each stroke
    ShadeDown,
    /// Colorize mode - only changes colors, keeps existing characters
    Colorize,

    /// Set blinking attribute (true = on, false = off)
    Blink(bool),
}

impl Default for BrushMode {
    fn default() -> Self {
        Self::HalfBlock
    }
}

/// Role of a point in a shape (for outline drawing)
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PointRole {
    /// Northwest corner (top-left)
    NWCorner,
    /// Northeast corner (top-right)
    NECorner,
    /// Southwest corner (bottom-left)
    SWCorner,
    /// Southeast corner (bottom-right)
    SECorner,
    /// Left side of a shape
    LeftSide,
    /// Right side of a shape
    RightSide,
    /// Top side of a shape
    TopSide,
    /// Bottom side of a shape
    BottomSide,
    /// Interior fill point
    Fill,
    /// Line segment point
    Line,
}

impl PointRole {
    /// Get the TheDraw outline character for this role
    /// Uses retrofont's outline table as the canonical source
    pub fn outline_char(&self, outline_style: u8) -> char {
        // Map PointRole to TheDraw character index (A=0, B=1, C=2, ...)
        let ch = match self {
            PointRole::TopSide => b'A',    // top outer
            PointRole::BottomSide => b'A', // (no separate bottom, reuse top)
            PointRole::LeftSide => b'C',   // left outer
            PointRole::RightSide => b'C',  // (no separate right, reuse left as vertical)
            PointRole::NWCorner => b'E',   // NW outer/outer
            PointRole::NECorner => b'F',   // NW outer/inner (used for NE)
            PointRole::SWCorner => b'I',   // SW outer/outer
            PointRole::SECorner => b'J',   // SW outer/inner (used for SE)
            PointRole::Fill | PointRole::Line => return ' ',
        };

        retrofont::transform_outline(outline_style as usize, ch)
    }
}
