//! F-Key Toolbar Layout Model
//!
//! Centralized layout calculations for the F-Key toolbar.
//! Used by both hit-testing and GPU rendering to ensure consistency.
//!
//! This module provides a single source of truth for all layout-related
//! computations, making maintenance easier and preventing drift between
//! the interactive and visual representations.

// Allow unused items - this module provides a complete API for future use.
// Currently only a subset is used by the GPU renderer.
#![allow(dead_code)]

use icy_ui::{Point, Rectangle};

use crate::ui::editor::ansi::constants::{TOP_CONTROL_HEIGHT, TOP_CONTROL_SHADOW_PADDING};

// ═══════════════════════════════════════════════════════════════════════════
// Layout Constants
// ═══════════════════════════════════════════════════════════════════════════

/// Target rendered height for slot characters (the F-key glyph) in logical pixels.
pub const SLOT_CHAR_HEIGHT: f32 = 32.0;

/// Target rendered height for label characters (set number digits) in logical pixels.
pub const LABEL_HEIGHT: f32 = 16.0 * 1.2;

/// Label width (2 digits: "01", "02", etc.).
pub const LABEL_WIDTH: f32 = 8.0 * 1.2;

/// Width per F-key slot (label + char area).
pub const SLOT_WIDTH: f32 = 40.0;

/// Spacing between slots.
pub const SLOT_SPACING: f32 = 5.0;

/// Navigation button size (clickable area).
pub const NAV_SIZE: f32 = 26.0;

/// Arrow icon size (visual triangle).
pub const ARROW_SIZE: f32 = 14.0;

/// Desired visual gap between arrow icon edge and set number digits.
pub const SET_NUM_ICON_GAP: f32 = 6.0;

/// Fine-tuning: shift the set number digits left.
pub const NAV_NUM_SHIFT_X: f32 = -4.0;

/// Fine-tuning: shift the NEXT nav button left.
pub const NAV_NEXT_SHIFT_X: f32 = -8.0;

/// Default space for nav label (fallback if not computed dynamically).
pub const NAV_LABEL_SPACE: f32 = 16.0;

/// Gap before nav section.
pub const NAV_GAP: f32 = 0.0;

/// Corner radius for rounded rectangles.
pub const CORNER_RADIUS: f32 = 6.0;

/// Border width.
pub const BORDER_WIDTH: f32 = 1.0;

/// Extra padding around the control for drop shadow.
pub const SHADOW_PADDING: f32 = TOP_CONTROL_SHADOW_PADDING;

/// Toolbar height (control area, excluding shadow).
pub const TOOLBAR_HEIGHT: f32 = TOP_CONTROL_HEIGHT;

/// Left padding before content.
pub const LEFT_PADDING: f32 = 9.0;

/// Right padding after the nav section.
pub const RIGHT_PADDING: f32 = 4.0;

/// Number of F-key slots.
pub const NUM_SLOTS: usize = 12;

/// Marker value for "no hover".
pub const NO_HOVER: u32 = 0xFFFF_FFFF;

// ═══════════════════════════════════════════════════════════════════════════
// Hover State
// ═══════════════════════════════════════════════════════════════════════════

/// Hover state: which element is currently hovered.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum HoverState {
    #[default]
    None,
    /// Slot hover (slot_index 0..11, is_on_char: true=char area, false=label area).
    Slot(usize, bool),
    /// Hover over previous-set navigation arrow.
    NavPrev,
    /// Hover over next-set navigation arrow.
    NavNext,
}

impl HoverState {
    /// Convert to GPU uniform representation (slot, hover_type).
    pub fn to_uniforms(&self) -> (u32, u32) {
        match self {
            HoverState::None => (NO_HOVER, 0),
            HoverState::Slot(idx, is_char) => (*idx as u32, if *is_char { 1 } else { 0 }),
            HoverState::NavPrev => (NO_HOVER, 2),
            HoverState::NavNext => (NO_HOVER, 3),
        }
    }

    /// Reconstruct from GPU uniform representation.
    pub fn from_uniforms(slot: u32, hover_type: u32) -> Self {
        if hover_type == 2 {
            HoverState::NavPrev
        } else if hover_type == 3 {
            HoverState::NavNext
        } else if slot != NO_HOVER {
            HoverState::Slot(slot as usize, hover_type == 1)
        } else {
            HoverState::None
        }
    }
}

// ═══════════════════════════════════════════════════════════════════════════
// Layout Calculator
// ═══════════════════════════════════════════════════════════════════════════

/// Pre-computed layout for the F-Key toolbar.
///
/// All values are in logical pixels unless otherwise noted.
/// For physical pixels, multiply by `scale`.
#[derive(Debug, Clone)]
pub struct FKeyLayout {
    /// Total widget width (including shadow padding).
    pub total_width: f32,
    /// Total widget height (including shadow padding).
    pub total_height: f32,
    /// Control height (excluding shadow).
    pub control_height: f32,
    /// X offset where content starts.
    pub content_start_x: f32,
    /// Width of all slots combined (including spacing).
    pub slots_width: f32,
    /// X offset where nav section starts.
    pub nav_x: f32,
    /// Space reserved for the set number between arrows.
    pub nav_label_space: f32,
    /// X offset for the "next" nav button.
    pub next_nav_x: f32,
    /// Magnification factor for slot characters.
    pub slot_char_magnify: f32,
    /// Magnification factor for label characters.
    pub label_magnify: f32,
    /// Font glyph width.
    pub font_width: f32,
    /// Font glyph height.
    pub font_height: f32,
}

impl FKeyLayout {
    /// Create a new layout based on font dimensions.
    ///
    /// # Arguments
    /// * `font_width` - Font glyph width in pixels.
    /// * `font_height` - Font glyph height in pixels.
    pub fn new(font_width: f32, font_height: f32) -> Self {
        let font_w = font_width.max(1.0);
        let font_h = font_height.max(1.0);

        let slot_char_magnify = (SLOT_CHAR_HEIGHT / font_h).floor().max(1.0);
        let label_magnify = (LABEL_HEIGHT / font_h).floor().max(1.0);
        let label_char_w = font_w * label_magnify;

        // Compute padding so the perceived gap between the arrow icon and the number is ~SET_NUM_ICON_GAP.
        let icon_side_gap = (NAV_SIZE - ARROW_SIZE) / 2.0;
        let num_padding = (SET_NUM_ICON_GAP - icon_side_gap).max(0.0);

        let set_num_field_width = 2.0 * label_char_w;
        let nav_label_space = set_num_field_width + 2.0 * num_padding;

        let content_start_x = SHADOW_PADDING + BORDER_WIDTH + LEFT_PADDING;
        let slots_width = (NUM_SLOTS as f32) * SLOT_WIDTH + ((NUM_SLOTS - 1) as f32) * SLOT_SPACING;
        let nav_x = content_start_x + slots_width + NAV_GAP;
        let next_nav_x = nav_x + NAV_SIZE + nav_label_space + NAV_NEXT_SHIFT_X;

        let content_width = slots_width + NAV_GAP + (2.0 * NAV_SIZE) + nav_label_space + NAV_NEXT_SHIFT_X + RIGHT_PADDING;
        let total_width = content_width + SHADOW_PADDING * 2.0 + BORDER_WIDTH * 2.0 + LEFT_PADDING;
        let total_height = TOOLBAR_HEIGHT + SHADOW_PADDING * 2.0;
        let control_height = TOOLBAR_HEIGHT;

        Self {
            total_width,
            total_height,
            control_height,
            content_start_x,
            slots_width,
            nav_x,
            nav_label_space,
            next_nav_x,
            slot_char_magnify,
            label_magnify,
            font_width: font_w,
            font_height: font_h,
        }
    }

    /// Create layout with default 8x16 font.
    pub fn default_font() -> Self {
        Self::new(8.0, 16.0)
    }

    // ───────────────────────────────────────────────────────────────────────
    // Slot Geometry
    // ───────────────────────────────────────────────────────────────────────

    /// Get the X offset for a slot (0..11).
    #[inline]
    pub fn slot_x(&self, slot: usize) -> f32 {
        self.content_start_x + (slot as f32) * (SLOT_WIDTH + SLOT_SPACING)
    }

    /// Get the X offset for the character area within a slot.
    #[inline]
    pub fn slot_char_x(&self, slot: usize) -> f32 {
        self.slot_x(slot) + LABEL_WIDTH
    }

    /// Get the full slot rectangle (label + char area).
    pub fn slot_rect(&self, slot: usize) -> Rectangle {
        Rectangle {
            x: self.slot_x(slot),
            y: SHADOW_PADDING,
            width: SLOT_WIDTH,
            height: self.control_height,
        }
    }

    /// Get the label area rectangle within a slot.
    pub fn slot_label_rect(&self, slot: usize) -> Rectangle {
        Rectangle {
            x: self.slot_x(slot),
            y: SHADOW_PADDING,
            width: LABEL_WIDTH,
            height: self.control_height,
        }
    }

    /// Get the character area rectangle within a slot.
    pub fn slot_char_rect(&self, slot: usize) -> Rectangle {
        Rectangle {
            x: self.slot_char_x(slot),
            y: SHADOW_PADDING,
            width: SLOT_WIDTH - LABEL_WIDTH,
            height: self.control_height,
        }
    }

    // ───────────────────────────────────────────────────────────────────────
    // Rendered Glyph Geometry (for GPU)
    // ───────────────────────────────────────────────────────────────────────

    /// Rendered slot char dimensions.
    pub fn slot_char_size(&self) -> (f32, f32) {
        let w = self.font_width * self.slot_char_magnify;
        let h = self.font_height * self.slot_char_magnify;
        (w, h)
    }

    /// Y offset for centering slot char in control area.
    pub fn slot_char_y(&self) -> f32 {
        let (_, h) = self.slot_char_size();
        SHADOW_PADDING + (self.control_height - h) / 2.0
    }

    /// Rendered label char dimensions.
    pub fn label_char_size(&self) -> (f32, f32) {
        let w = self.font_width * self.label_magnify;
        let h = self.font_height * self.label_magnify;
        (w, h)
    }

    /// Y offset for centering label char in control area.
    pub fn label_y(&self) -> f32 {
        let (_, h) = self.label_char_size();
        SHADOW_PADDING + (self.control_height - h) / 2.0
    }

    // ───────────────────────────────────────────────────────────────────────
    // Navigation Geometry
    // ───────────────────────────────────────────────────────────────────────

    /// Get the "previous" nav button rectangle.
    pub fn nav_prev_rect(&self) -> Rectangle {
        let nav_y = SHADOW_PADDING + (self.control_height - NAV_SIZE) / 2.0;
        Rectangle {
            x: self.nav_x,
            y: nav_y,
            width: NAV_SIZE,
            height: NAV_SIZE,
        }
    }

    /// Get the "next" nav button rectangle.
    pub fn nav_next_rect(&self) -> Rectangle {
        let nav_y = SHADOW_PADDING + (self.control_height - NAV_SIZE) / 2.0;
        Rectangle {
            x: self.next_nav_x,
            y: nav_y,
            width: NAV_SIZE,
            height: NAV_SIZE,
        }
    }

    /// Get the center point for the prev arrow icon.
    pub fn nav_prev_arrow_center(&self) -> Point {
        let rect = self.nav_prev_rect();
        Point::new(rect.x + rect.width / 2.0, rect.y + rect.height / 2.0)
    }

    /// Get the center point for the next arrow icon.
    pub fn nav_next_arrow_center(&self) -> Point {
        let rect = self.nav_next_rect();
        Point::new(rect.x + rect.width / 2.0, rect.y + rect.height / 2.0)
    }

    /// Get the X offset for the set number display.
    pub fn set_num_x(&self, num_width: f32) -> f32 {
        let icon_side_gap = (NAV_SIZE - ARROW_SIZE) / 2.0;
        let num_padding = (SET_NUM_ICON_GAP - icon_side_gap).max(0.0);
        let num_field_width = 2.0 * self.font_width * self.label_magnify;
        self.nav_x + NAV_SIZE + num_padding + (num_field_width - num_width) / 2.0 + NAV_NUM_SHIFT_X
    }

    // ───────────────────────────────────────────────────────────────────────
    // Hit Testing
    // ───────────────────────────────────────────────────────────────────────

    /// Compute hover state from cursor position (relative to widget bounds).
    ///
    /// # Arguments
    /// * `pos` - Cursor position relative to widget origin.
    ///
    /// # Returns
    /// The `HoverState` for the given position.
    pub fn hit_test(&self, pos: Point) -> HoverState {
        // Check F-key slots
        for slot in 0..NUM_SLOTS {
            let slot_rect = self.slot_rect(slot);
            if pos.x >= slot_rect.x && pos.x < slot_rect.x + slot_rect.width && pos.y >= slot_rect.y && pos.y < slot_rect.y + slot_rect.height {
                let char_x = self.slot_char_x(slot);
                let is_on_char = pos.x >= char_x;
                return HoverState::Slot(slot, is_on_char);
            }
        }

        // Check nav buttons
        let prev_rect = self.nav_prev_rect();
        if pos.x >= prev_rect.x && pos.x < prev_rect.x + prev_rect.width && pos.y >= prev_rect.y && pos.y < prev_rect.y + prev_rect.height {
            return HoverState::NavPrev;
        }

        let next_rect = self.nav_next_rect();
        if pos.x >= next_rect.x && pos.x < next_rect.x + next_rect.width && pos.y >= next_rect.y && pos.y < next_rect.y + next_rect.height {
            return HoverState::NavNext;
        }

        HoverState::None
    }

    /// Compute hover state and return as GPU uniform representation.
    pub fn hit_test_uniforms(&self, pos: Point) -> (u32, u32) {
        self.hit_test(pos).to_uniforms()
    }
}

// ═══════════════════════════════════════════════════════════════════════════
// Scaled Layout (for GPU rendering in physical pixels)
// ═══════════════════════════════════════════════════════════════════════════

/// Layout scaled to physical pixels for GPU rendering.
#[derive(Debug, Clone)]
pub struct FKeyLayoutScaled {
    /// Base layout (logical pixels).
    pub base: FKeyLayout,
    /// Scale factor (DPI scaling).
    pub scale: f32,
}

impl FKeyLayoutScaled {
    /// Create a scaled layout.
    pub fn new(base: FKeyLayout, scale: f32) -> Self {
        Self { base, scale }
    }

    /// Scale a logical pixel value to physical pixels.
    #[inline]
    pub fn px(&self, logical: f32) -> f32 {
        (logical * self.scale).round()
    }

    /// Content start X in physical pixels.
    pub fn content_start_x_px(&self) -> f32 {
        self.px(self.base.content_start_x)
    }

    /// Control height in physical pixels.
    pub fn control_height_px(&self) -> f32 {
        self.px(self.base.control_height)
    }

    /// Slot width in physical pixels.
    pub fn slot_width_px(&self) -> f32 {
        self.px(SLOT_WIDTH)
    }

    /// Slot spacing in physical pixels.
    pub fn slot_spacing_px(&self) -> f32 {
        self.px(SLOT_SPACING)
    }

    /// Nav size in physical pixels.
    pub fn nav_size_px(&self) -> f32 {
        self.px(NAV_SIZE)
    }

    /// Arrow size in physical pixels.
    pub fn arrow_size_px(&self) -> f32 {
        self.px(ARROW_SIZE)
    }

    /// Slot char magnification (HiDPI-safe integer scaling).
    pub fn slot_char_magnify_px(&self) -> f32 {
        let target_h = self.px(SLOT_CHAR_HEIGHT);
        (target_h / self.base.font_height).floor().max(1.0)
    }

    /// Label magnification (HiDPI-safe integer scaling).
    pub fn label_magnify_px(&self) -> f32 {
        let target_h = self.px(LABEL_HEIGHT);
        (target_h / self.base.font_height).floor().max(1.0)
    }

    /// Slot char size in physical pixels.
    pub fn slot_char_size_px(&self) -> (f32, f32) {
        let mag = self.slot_char_magnify_px();
        let w = (self.base.font_width * mag).round().max(1.0);
        let h = (self.base.font_height * mag).round().max(1.0);
        (w, h)
    }

    /// Slot char Y in physical pixels (centered).
    pub fn slot_char_y_px(&self) -> f32 {
        let (_, h) = self.slot_char_size_px();
        let shadow_px = self.px(SHADOW_PADDING);
        let control_h = self.control_height_px();
        shadow_px + ((control_h - h) / 2.0).floor()
    }

    /// Label char size in physical pixels.
    pub fn label_char_size_px(&self) -> (f32, f32) {
        let mag = self.label_magnify_px();
        let w = (self.base.font_width * mag).round().max(1.0);
        let h = (self.base.font_height * mag).round().max(1.0);
        (w, h)
    }

    /// Label Y in physical pixels (centered).
    pub fn label_y_px(&self) -> f32 {
        let (_, h) = self.label_char_size_px();
        let shadow_px = self.px(SHADOW_PADDING);
        let control_h = self.control_height_px();
        shadow_px + ((control_h - h) / 2.0).floor()
    }

    /// Slot X in physical pixels.
    pub fn slot_x_px(&self, slot: usize) -> f32 {
        let content_start = self.content_start_x_px();
        let slot_stride = self.slot_width_px() + self.slot_spacing_px();
        (content_start + (slot as f32) * slot_stride).floor()
    }

    /// Slot char X in physical pixels.
    pub fn slot_char_x_px(&self, slot: usize) -> f32 {
        (self.slot_x_px(slot) + self.px(LABEL_WIDTH)).floor()
    }

    /// Nav X in physical pixels.
    pub fn nav_x_px(&self) -> f32 {
        let slots_width = (NUM_SLOTS as f32) * self.slot_width_px() + ((NUM_SLOTS - 1) as f32) * self.slot_spacing_px();
        (self.content_start_x_px() + slots_width + self.px(NAV_GAP)).floor()
    }

    /// Nav label space in physical pixels.
    pub fn nav_label_space_px(&self) -> f32 {
        let (label_w, _) = self.label_char_size_px();
        let icon_side_gap = (self.nav_size_px() - self.arrow_size_px()) / 2.0;
        let num_padding = (self.px(SET_NUM_ICON_GAP) - icon_side_gap).max(0.0);
        2.0 * label_w + 2.0 * num_padding
    }

    /// Next nav X in physical pixels.
    pub fn next_nav_x_px(&self) -> f32 {
        self.nav_x_px() + self.nav_size_px() + self.nav_label_space_px() + self.px(NAV_NEXT_SHIFT_X)
    }

    /// Arrow Y in physical pixels (centered).
    pub fn arrow_y_px(&self) -> f32 {
        let shadow_px = self.px(SHADOW_PADDING);
        let control_h = self.control_height_px();
        shadow_px + ((control_h - self.arrow_size_px()) / 2.0).floor()
    }

    /// Set number X in physical pixels.
    pub fn set_num_x_px(&self, num_width_px: f32) -> f32 {
        let (label_w, _) = self.label_char_size_px();
        let icon_side_gap = (self.nav_size_px() - self.arrow_size_px()) / 2.0;
        let num_padding = (self.px(SET_NUM_ICON_GAP) - icon_side_gap).max(0.0);
        let num_field_width = 2.0 * label_w;
        self.nav_x_px() + self.nav_size_px() + num_padding + (num_field_width - num_width_px) / 2.0 + self.px(NAV_NUM_SHIFT_X)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_layout_default() {
        let layout = FKeyLayout::default_font();
        assert!(layout.total_width > 0.0);
        assert!(layout.total_height > 0.0);
        assert_eq!(layout.slot_char_magnify, 2.0); // 32 / 16 = 2
        assert_eq!(layout.label_magnify, 1.0); // 19 / 16 = 1.1875 → floor = 1
    }

    #[test]
    fn test_hit_test_slot() {
        let layout = FKeyLayout::default_font();
        // Hit first slot label area
        let pos = Point::new(layout.slot_x(0) + 5.0, SHADOW_PADDING + 10.0);
        assert_eq!(layout.hit_test(pos), HoverState::Slot(0, false));

        // Hit first slot char area
        let pos = Point::new(layout.slot_char_x(0) + 5.0, SHADOW_PADDING + 10.0);
        assert_eq!(layout.hit_test(pos), HoverState::Slot(0, true));
    }

    #[test]
    fn test_hit_test_nav() {
        let layout = FKeyLayout::default_font();
        let prev_rect = layout.nav_prev_rect();
        let pos = Point::new(prev_rect.x + 5.0, prev_rect.y + 5.0);
        assert_eq!(layout.hit_test(pos), HoverState::NavPrev);

        let next_rect = layout.nav_next_rect();
        let pos = Point::new(next_rect.x + 5.0, next_rect.y + 5.0);
        assert_eq!(layout.hit_test(pos), HoverState::NavNext);
    }
}
