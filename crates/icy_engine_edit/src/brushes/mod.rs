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
//! - Brush size expansion utilities
//!
//! # Shape Functions
//! Each shape has `get_*_points()` functions that return point lists without drawing,
//! and `draw_*()` / `fill_*()` functions that draw directly to a target.
//!
//! - `get_line_points(p0, p1)` - Get all points on a line
//! - `get_rectangle_points(p0, p1)` - Get outline points of a rectangle
//! - `get_filled_rectangle_points(p0, p1)` - Get all points of a filled rectangle
//! - `get_ellipse_points(center, rx, ry)` - Get outline points of an ellipse
//! - `get_ellipse_points_from_rect(p0, p1)` - Get outline points from bounding box
//! - `get_filled_ellipse_points(center, rx, ry)` - Get all points of a filled ellipse
//! - `get_filled_ellipse_points_from_rect(p0, p1)` - Get filled points from bounding box
//!
//! # Example
//! ```ignore
//! use icy_engine_edit::brushes::{DrawContext, BrushMode, ColorMode, draw_line, get_line_points};
//! use icy_engine::Position;
//!
//! // Get points without drawing
//! let points = get_line_points(Position::new(0, 0), Position::new(10, 5));
//!
//! // Or draw directly to a target
//! let ctx = DrawContext::default();
//! let mut target = MyDrawTarget::new();
//! draw_line(&mut target, &ctx, Position::new(0, 0), Position::new(10, 5));
//! ```

mod brush_mode;
mod color_mode;
pub mod ellipse;
pub mod line;
pub mod rectangle;

pub use brush_mode::{BrushMode, PointRole};
pub use color_mode::ColorMode;
pub use ellipse::{
    draw_ellipse, draw_ellipse_from_rect, fill_ellipse, fill_ellipse_from_rect, get_ellipse_points, get_ellipse_points_from_rect, get_filled_ellipse_points,
    get_filled_ellipse_points_from_rect,
};
pub use line::{draw_line, get_line_points};
pub use rectangle::{draw_rectangle, fill_rectangle, get_filled_rectangle_points, get_rectangle_points};

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

        // Find current shade level (-1 = empty/space, 0-3 = shade gradient)
        let mut level = SHADE_GRADIENT.iter().position(|&c| c == current_ch).map(|i| i as i32).unwrap_or(-1); // -1 represents empty/space

        // Adjust shade level (Moebius behavior)
        if up {
            // Shade up: -1 (empty) -> 0 -> 1 -> 2 -> 3 (full block)
            level = (level + 1).min(SHADE_GRADIENT.len() as i32 - 1);
        } else {
            // Shade down: 3 -> 2 -> 1 -> 0 -> -1 (empty/space)
            level -= 1;
        }

        // Apply the new character
        if level < 0 {
            // Below shade gradient = empty space
            let attr = self.make_attribute();
            target.set_char(pos, AttributedChar::new(' ', attr));
        } else {
            let new_ch = SHADE_GRADIENT[level as usize];
            let attr = self.make_attribute();
            target.set_char(pos, AttributedChar::new(new_ch, attr));
        }
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

/// Expand a list of points by brush_size, centering the brush on each original point.
///
/// This creates a square expansion around each point. When `brush_size` is 1,
/// the original points are returned unchanged.
///
/// # Arguments
/// * `points` - The original points to expand
/// * `brush_size` - The size of the brush (width/height of the square)
///
/// # Returns
/// A new vector with all expanded points (may contain duplicates).
pub fn expand_points_by_brush_size(points: &[Position], brush_size: i32) -> Vec<Position> {
    if brush_size <= 1 {
        return points.to_vec();
    }

    let half = brush_size / 2;
    let mut expanded = Vec::with_capacity(points.len() * (brush_size * brush_size) as usize);

    for &p in points {
        for dy in 0..brush_size {
            for dx in 0..brush_size {
                expanded.push(p + Position::new(dx - half, dy - half));
            }
        }
    }

    expanded
}

/// Expand a list of points with PointRole by brush_size, centering the brush on each original point.
///
/// This creates a square expansion around each point. When `brush_size` is 1,
/// the original points are returned unchanged. The PointRole is preserved for all expanded points.
///
/// # Arguments
/// * `points` - The original points with their roles to expand
/// * `brush_size` - The size of the brush (width/height of the square)
///
/// # Returns
/// A new vector with all expanded points and their roles (may contain duplicates).
pub fn expand_points_with_role_by_brush_size(points: &[(Position, PointRole)], brush_size: i32) -> Vec<(Position, PointRole)> {
    if brush_size <= 1 {
        return points.to_vec();
    }

    let half = brush_size / 2;
    let mut expanded = Vec::with_capacity(points.len() * (brush_size * brush_size) as usize);

    for &(p, role) in points {
        for dy in 0..brush_size {
            for dx in 0..brush_size {
                expanded.push((p + Position::new(dx - half, dy - half), role));
            }
        }
    }

    expanded
}
