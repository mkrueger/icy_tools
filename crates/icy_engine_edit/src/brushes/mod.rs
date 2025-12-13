//! Brush library for ANSI art drawing operations
//!
//! This module provides GUI-independent drawing primitives for ANSI art editors.
//! The algorithms can be tested independently of any UI framework.
//!
//! # Features
//! - Line drawing (Bresenham algorithm)
//! - Rectangle outline and fill
//! - Ellipse outline and fill (midpoint algorithm)
//! - Various brush modes (block, half-block, shade, colorize, etc.)
//! - Outline character support for TheDraw fonts
//!
//! # Example
//! ```ignore
//! use icy_engine_edit::brushes::{DrawContext, BrushMode, ColorMode, line::draw_line};
//! use icy_engine::Position;
//!
//! let ctx = DrawContext::default();
//! let mut target = MyDrawTarget::new();
//! draw_line(&mut target, &ctx, Position::new(0, 0), Position::new(10, 5));
//! ```

mod brush_mode;
mod color_mode;
pub mod ellipse;
pub mod line;
pub mod rectangle;

pub use brush_mode::{BrushMode, CustomBrush, PointRole};
pub use color_mode::ColorMode;

use icy_engine::{AttributedChar, Position, TextAttribute};

/// Standard CP437 shade gradient characters (from light to dark)
///
/// Note: These are stored as CP437 codepoints (0..255) in `char`, matching how
/// ANSI buffers are represented throughout the editor.
pub const SHADE_GRADIENT: [char; 4] = ['\u{00B0}', '\u{00B1}', '\u{00B2}', 219 as char];

/// Half-block characters for high-resolution drawing
pub const HALF_BLOCKS: HalfBlocks = HalfBlocks {
    upper: 223 as char,
    lower: 220 as char,
    full: 219 as char,
    left: 221 as char,
    right: 222 as char,
};

/// Collection of half-block characters
pub struct HalfBlocks {
    pub upper: char,
    pub lower: char,
    pub full: char,
    pub left: char,
    pub right: char,
}

/// A trait for targets that can be drawn on
///
/// This abstraction allows the brush algorithms to work with
/// any buffer-like structure without depending on specific types.
pub trait DrawTarget {
    /// Get the width of the drawable area
    fn width(&self) -> i32;

    /// Get the height of the drawable area
    fn height(&self) -> i32;

    /// Get the character at a position
    fn char_at(&self, pos: Position) -> Option<AttributedChar>;

    /// Set the character at a position
    fn set_char(&mut self, pos: Position, ch: AttributedChar);

    /// Check if a position is within bounds
    fn is_valid(&self, pos: Position) -> bool {
        pos.x >= 0 && pos.y >= 0 && pos.x < self.width() && pos.y < self.height()
    }
}

/// Context for drawing operations
#[derive(Clone)]
pub struct DrawContext {
    /// The brush mode to use
    pub brush_mode: BrushMode,

    /// The color mode (which colors to affect)
    pub color_mode: ColorMode,

    /// The foreground color to use
    pub foreground: u32,

    /// The background color to use
    pub background: u32,

    /// Base attribute template (font page, blink, etc.) for drawing new chars.
    /// Colors are still taken from `foreground`/`background` according to `color_mode`.
    pub template_attribute: TextAttribute,

    /// Outline style for TheDraw fonts (0-15)
    pub outline_style: u8,

    /// Whether to use character mirroring
    pub mirror_mode: MirrorMode,

    /// For half-block mode: whether the point targets the upper half (true) or lower half (false)
    pub half_block_is_top: bool,
}

impl Default for DrawContext {
    fn default() -> Self {
        let template_attribute = TextAttribute::new(7, 0);
        Self {
            brush_mode: BrushMode::HalfBlock,
            color_mode: ColorMode::Both,
            foreground: 7,
            background: 0,
            template_attribute,
            outline_style: 0,
            mirror_mode: MirrorMode::None,
            half_block_is_top: true,
        }
    }
}

/// Mirror mode for symmetrical drawing
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum MirrorMode {
    #[default]
    None,
    Horizontal,
    Vertical,
    Both,
}

impl DrawContext {
    /// Create a new draw context with default settings
    pub fn new() -> Self {
        Self::default()
    }

    /// Set the brush mode
    pub fn with_brush_mode(mut self, mode: BrushMode) -> Self {
        self.brush_mode = mode;
        self
    }

    /// Set the color mode
    pub fn with_color_mode(mut self, mode: ColorMode) -> Self {
        self.color_mode = mode;
        self
    }

    /// Set foreground color
    pub fn with_foreground(mut self, fg: u32) -> Self {
        self.foreground = fg;
        self.template_attribute.set_foreground(fg);
        self
    }

    /// Set background color
    pub fn with_background(mut self, bg: u32) -> Self {
        self.background = bg;
        self.template_attribute.set_background(bg);
        self
    }

    /// Set the base attribute template used when drawing new characters.
    pub fn with_template_attribute(mut self, attr: TextAttribute) -> Self {
        self.foreground = attr.foreground();
        self.background = attr.background();
        self.template_attribute = attr;
        self
    }

    /// Set outline style
    pub fn with_outline_style(mut self, style: u8) -> Self {
        self.outline_style = style;
        self
    }

    /// Select which half is targeted for `BrushMode::HalfBlock`.
    pub fn with_half_block_is_top(mut self, is_top: bool) -> Self {
        self.half_block_is_top = is_top;
        self
    }

    /// Plot a single point using the current brush settings
    pub fn plot_point<T: DrawTarget>(&self, target: &mut T, pos: Position, role: PointRole) {
        if !target.is_valid(pos) {
            return;
        }

        match &self.brush_mode {
            BrushMode::Block => {
                self.draw_block(target, pos);
            }
            BrushMode::HalfBlock => {
                self.draw_half_block(target, pos);
            }
            BrushMode::Outline => {
                self.draw_outline(target, pos, role);
            }
            BrushMode::Char(ch) => {
                self.draw_char(target, pos, *ch);
            }
            BrushMode::Replace(ch) => {
                self.replace_char(target, pos, *ch);
            }
            BrushMode::Shade => {
                self.draw_shade(target, pos, true);
            }
            BrushMode::ShadeDown => {
                self.draw_shade(target, pos, false);
            }
            BrushMode::Colorize => {
                self.colorize(target, pos);
            }
            BrushMode::Blink(on) => {
                self.set_blink(target, pos, *on);
            }
            BrushMode::Custom(brush) => {
                self.draw_custom_brush(target, pos, brush);
            }
        }

        // Handle mirroring
        if self.mirror_mode != MirrorMode::None {
            self.plot_mirrored_points(target, pos, role);
        }
    }

    fn draw_block<T: DrawTarget>(&self, target: &mut T, pos: Position) {
        let attr = self.make_attribute();
        target.set_char(pos, AttributedChar::new(HALF_BLOCKS.full, attr));
    }

    fn draw_half_block<T: DrawTarget>(&self, target: &mut T, pos: Position) {
        // CP437 half-block drawing: update only upper/lower pixel using existing cell state.
        let Some(current) = target.char_at(pos) else {
            return;
        };
        let hb_pos = Position::new(pos.x, if self.half_block_is_top { 0 } else { 1 });
        let hb = icy_engine::paint::HalfBlock::from_char(current, hb_pos);
        let col = self.foreground;
        let block = hb.get_half_block_char(col, true);
        target.set_char(pos, block);
    }

    fn draw_outline<T: DrawTarget>(&self, target: &mut T, pos: Position, role: PointRole) {
        let ch = role.outline_char(self.outline_style);
        if ch == ' ' {
            return;
        }
        let attr = self.make_attribute();
        target.set_char(pos, AttributedChar::new(ch, attr));
    }

    fn draw_char<T: DrawTarget>(&self, target: &mut T, pos: Position, ch: char) {
        let attr = self.make_attribute();
        target.set_char(pos, AttributedChar::new(ch, attr));
    }

    fn replace_char<T: DrawTarget>(&self, target: &mut T, pos: Position, ch: char) {
        if let Some(mut cur) = target.char_at(pos) {
            cur.ch = ch;
            cur.attribute.attr &= !icy_engine::attribute::INVISIBLE;
            target.set_char(pos, cur);
        }
    }

    fn draw_shade<T: DrawTarget>(&self, target: &mut T, pos: Position, up: bool) {
        let current = target.char_at(pos);
        let current_ch = current.map(|c| c.ch).unwrap_or(' ');

        // Find current shade level
        let mut level = SHADE_GRADIENT
            .iter()
            .position(|&c| c == current_ch)
            .map(|i| i as i32)
            .unwrap_or(if up { -1 } else { SHADE_GRADIENT.len() as i32 });

        // Adjust shade level
        if up {
            level = (level + 1).min(SHADE_GRADIENT.len() as i32 - 1);
        } else {
            level = (level - 1).max(0);
        }

        let new_ch = SHADE_GRADIENT[level as usize];
        let attr = self.make_attribute();
        target.set_char(pos, AttributedChar::new(new_ch, attr));
    }

    fn colorize<T: DrawTarget>(&self, target: &mut T, pos: Position) {
        if let Some(mut ch) = target.char_at(pos) {
            let mut attr = ch.attribute;

            if self.color_mode.affects_foreground() {
                attr.set_foreground(self.foreground);
            }
            if self.color_mode.affects_background() {
                attr.set_background(self.background);
            }

            ch.attribute = attr;
            target.set_char(pos, ch);
        }
    }

    fn set_blink<T: DrawTarget>(&self, target: &mut T, pos: Position, on: bool) {
        if let Some(mut ch) = target.char_at(pos) {
            let mut attr = ch.attribute;
            attr.set_is_blinking(on);
            ch.attribute = attr;
            target.set_char(pos, ch);
        }
    }

    fn draw_custom_brush<T: DrawTarget>(&self, target: &mut T, pos: Position, brush: &CustomBrush) {
        let offset_x = brush.width / 2;
        let offset_y = brush.height / 2;

        for by in 0..brush.height {
            for bx in 0..brush.width {
                if let Some(ch) = brush.char_at(bx, by) {
                    if ch != ' ' {
                        let target_pos = Position::new(pos.x + bx - offset_x, pos.y + by - offset_y);
                        if target.is_valid(target_pos) {
                            let attr = self.make_attribute();
                            target.set_char(target_pos, AttributedChar::new(ch, attr));
                        }
                    }
                }
            }
        }
    }

    fn plot_mirrored_points<T: DrawTarget>(&self, target: &mut T, pos: Position, role: PointRole) {
        let center_x = target.width() / 2;
        let center_y = target.height() / 2;

        match self.mirror_mode {
            MirrorMode::Horizontal => {
                let mirror_x = 2 * center_x - pos.x - 1;
                if mirror_x != pos.x {
                    self.plot_single_point(target, Position::new(mirror_x, pos.y), role.mirror_horizontal());
                }
            }
            MirrorMode::Vertical => {
                let mirror_y = 2 * center_y - pos.y - 1;
                if mirror_y != pos.y {
                    self.plot_single_point(target, Position::new(pos.x, mirror_y), role.mirror_vertical());
                }
            }
            MirrorMode::Both => {
                let mirror_x = 2 * center_x - pos.x - 1;
                let mirror_y = 2 * center_y - pos.y - 1;
                if mirror_x != pos.x {
                    self.plot_single_point(target, Position::new(mirror_x, pos.y), role.mirror_horizontal());
                }
                if mirror_y != pos.y {
                    self.plot_single_point(target, Position::new(pos.x, mirror_y), role.mirror_vertical());
                }
                if mirror_x != pos.x && mirror_y != pos.y {
                    self.plot_single_point(target, Position::new(mirror_x, mirror_y), role.mirror_both());
                }
            }
            MirrorMode::None => {}
        }
    }

    /// Plot a single point without triggering mirror recursion
    fn plot_single_point<T: DrawTarget>(&self, target: &mut T, pos: Position, role: PointRole) {
        if !target.is_valid(pos) {
            return;
        }

        match &self.brush_mode {
            BrushMode::Block => self.draw_block(target, pos),
            BrushMode::HalfBlock => self.draw_half_block(target, pos),
            BrushMode::Outline => self.draw_outline(target, pos, role),
            BrushMode::Char(ch) => self.draw_char(target, pos, *ch),
            BrushMode::Replace(ch) => self.replace_char(target, pos, *ch),
            BrushMode::Shade => self.draw_shade(target, pos, true),
            BrushMode::ShadeDown => self.draw_shade(target, pos, false),
            BrushMode::Colorize => self.colorize(target, pos),
            BrushMode::Blink(on) => self.set_blink(target, pos, *on),
            BrushMode::Custom(brush) => self.draw_custom_brush(target, pos, brush),
        }
    }

    fn make_attribute(&self) -> TextAttribute {
        let mut attr = self.template_attribute;

        if self.color_mode.affects_foreground() {
            attr.set_foreground(self.foreground);
        }
        if self.color_mode.affects_background() {
            attr.set_background(self.background);
        }

        attr
    }
}

impl PointRole {
    /// Mirror this role horizontally
    pub fn mirror_horizontal(&self) -> Self {
        match self {
            PointRole::NWCorner => PointRole::NECorner,
            PointRole::NECorner => PointRole::NWCorner,
            PointRole::SWCorner => PointRole::SECorner,
            PointRole::SECorner => PointRole::SWCorner,
            PointRole::LeftSide => PointRole::RightSide,
            PointRole::RightSide => PointRole::LeftSide,
            other => *other,
        }
    }

    /// Mirror this role vertically
    pub fn mirror_vertical(&self) -> Self {
        match self {
            PointRole::NWCorner => PointRole::SWCorner,
            PointRole::NECorner => PointRole::SECorner,
            PointRole::SWCorner => PointRole::NWCorner,
            PointRole::SECorner => PointRole::NECorner,
            PointRole::TopSide => PointRole::BottomSide,
            PointRole::BottomSide => PointRole::TopSide,
            other => *other,
        }
    }

    /// Mirror this role both horizontally and vertically
    pub fn mirror_both(&self) -> Self {
        self.mirror_horizontal().mirror_vertical()
    }
}

/// A simple test target that records all drawing operations
#[cfg(test)]
pub struct TestTarget {
    width: i32,
    height: i32,
    chars: Vec<AttributedChar>,
}

#[cfg(test)]
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

#[cfg(test)]
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_draw_block() {
        let mut target = TestTarget::new(80, 25);
        let ctx = DrawContext::default().with_brush_mode(BrushMode::Block);

        ctx.plot_point(&mut target, Position::new(10, 5), PointRole::Fill);

        let ch = target.get_at(10, 5);
        assert_eq!(ch.ch, HALF_BLOCKS.full);
    }

    #[test]
    fn test_draw_char() {
        let mut target = TestTarget::new(80, 25);
        let ctx = DrawContext::default().with_brush_mode(BrushMode::Char('X'));

        ctx.plot_point(&mut target, Position::new(10, 5), PointRole::Fill);

        let ch = target.get_at(10, 5);
        assert_eq!(ch.ch, 'X');
    }

    #[test]
    fn test_shade_gradient() {
        let mut target = TestTarget::new(80, 25);
        let ctx = DrawContext::default().with_brush_mode(BrushMode::Shade);
        let pos = Position::new(10, 5);

        // First stroke
        ctx.plot_point(&mut target, pos, PointRole::Fill);
        assert_eq!(target.get_at(10, 5).ch, SHADE_GRADIENT[0]);

        // Second stroke
        ctx.plot_point(&mut target, pos, PointRole::Fill);
        assert_eq!(target.get_at(10, 5).ch, SHADE_GRADIENT[1]);

        // Third stroke
        ctx.plot_point(&mut target, pos, PointRole::Fill);
        assert_eq!(target.get_at(10, 5).ch, SHADE_GRADIENT[2]);

        // Fourth stroke - max
        ctx.plot_point(&mut target, pos, PointRole::Fill);
        assert_eq!(target.get_at(10, 5).ch, SHADE_GRADIENT[3]);
    }

    #[test]
    fn test_colorize() {
        let mut target = TestTarget::new(80, 25);
        let pos = Position::new(10, 5);

        // First, draw a character
        target.set_char(pos, AttributedChar::new('A', TextAttribute::default()));

        // Now colorize it
        let ctx = DrawContext::default()
            .with_brush_mode(BrushMode::Colorize)
            .with_foreground(4)
            .with_background(1);

        ctx.plot_point(&mut target, pos, PointRole::Fill);

        let ch = target.get_at(10, 5);
        assert_eq!(ch.ch, 'A'); // Character unchanged
        assert_eq!(ch.attribute.foreground(), 4);
        assert_eq!(ch.attribute.background(), 1);
    }

    #[test]
    fn test_bounds_check() {
        let mut target = TestTarget::new(80, 25);
        let ctx = DrawContext::default().with_brush_mode(BrushMode::Block);

        // Should not panic on out-of-bounds
        ctx.plot_point(&mut target, Position::new(-1, 5), PointRole::Fill);
        ctx.plot_point(&mut target, Position::new(100, 5), PointRole::Fill);
        ctx.plot_point(&mut target, Position::new(10, -1), PointRole::Fill);
        ctx.plot_point(&mut target, Position::new(10, 100), PointRole::Fill);
    }

    #[test]
    fn test_mirror_horizontal() {
        let mut target = TestTarget::new(80, 25);
        let ctx = DrawContext {
            brush_mode: BrushMode::Block,
            mirror_mode: MirrorMode::Horizontal,
            ..Default::default()
        };

        // Draw at x=10, should also draw at x=69 (mirror around center 40)
        ctx.plot_point(&mut target, Position::new(10, 5), PointRole::Fill);

        assert_eq!(target.get_at(10, 5).ch, HALF_BLOCKS.full);
        assert_eq!(target.get_at(69, 5).ch, HALF_BLOCKS.full);
    }

    #[test]
    fn test_point_role_mirror() {
        assert_eq!(PointRole::NWCorner.mirror_horizontal(), PointRole::NECorner);
        assert_eq!(PointRole::NWCorner.mirror_vertical(), PointRole::SWCorner);
        assert_eq!(PointRole::NWCorner.mirror_both(), PointRole::SECorner);
    }
}
