//! Segmented Control Layout Model
//!
//! Centralized layout calculations for the segmented control widget.
//! Used by both hit-testing (Canvas overlay) and GPU rendering to ensure consistency.
//!
//! This module provides a single source of truth for all layout-related
//! computations, making maintenance easier and preventing drift between
//! the interactive and visual representations.

// Allow unused items - this module provides a complete API for future use.
#![allow(dead_code)]

use iced::{Point, Rectangle};
use icy_engine::BitFont;

use super::constants::{TOP_CONTROL_HEIGHT, TOP_CONTROL_SHADOW_PADDING};

// ═══════════════════════════════════════════════════════════════════════════
// Layout Constants
// ═══════════════════════════════════════════════════════════════════════════

/// Horizontal padding inside each segment.
pub const SEGMENT_PADDING_H: f32 = 12.0;

/// Segment height (control area, excluding shadow).
pub const SEGMENT_HEIGHT: f32 = TOP_CONTROL_HEIGHT;

/// Corner radius for rounded rectangles.
pub const CORNER_RADIUS: f32 = 6.0;

/// Border width.
pub const BORDER_WIDTH: f32 = 1.0;

/// Extra padding around the control for drop shadow.
pub const SHADOW_PADDING: f32 = TOP_CONTROL_SHADOW_PADDING;

/// Preview glyph height for Char segments (rendered with caret colors).
pub const PREVIEW_GLYPH_HEIGHT: f32 = 32.0;

/// Maximum number of segments supported.
pub const MAX_SEGMENTS: usize = 8;

/// Marker value for "no hover".
pub const NO_HOVER: u32 = 0xFFFF_FFFF;

/// Magnification factor for Char segments.
pub const CHAR_MAGNIFICATION: f32 = 2.0;

// ═══════════════════════════════════════════════════════════════════════════
// Segment Content
// ═══════════════════════════════════════════════════════════════════════════

/// Segment content type for width calculation.
#[derive(Clone, Debug)]
pub enum SegmentContentType {
    /// Text label with character count.
    Text(usize),
    /// Single character (rendered larger).
    Char,
}

// ═══════════════════════════════════════════════════════════════════════════
// Layout Calculator
// ═══════════════════════════════════════════════════════════════════════════

/// Pre-computed layout for a segmented control.
///
/// All values are in logical pixels.
#[derive(Debug, Clone)]
pub struct SegmentedLayout {
    /// Width of each segment.
    pub segment_widths: Vec<f32>,
    /// Total widget width (including shadow padding).
    pub total_width: f32,
    /// Total widget height (including shadow padding).
    pub total_height: f32,
    /// Content width (sum of segment widths).
    pub content_width: f32,
    /// Font glyph width.
    pub font_width: f32,
    /// Font glyph height.
    pub font_height: f32,
    /// Preview glyph magnification factor.
    pub preview_magnify: f32,
}

impl SegmentedLayout {
    /// Create a new layout from segment content types and font.
    ///
    /// # Arguments
    /// * `segments` - Content types for each segment.
    /// * `font` - Optional BitFont for dimension calculations.
    pub fn new(segments: &[SegmentContentType], font: Option<&BitFont>) -> Self {
        let font_width = font.map(|f| f.size().width as f32).unwrap_or(8.0);
        let font_height = font.map(|f| f.size().height as f32).unwrap_or(16.0);
        
        let preview_magnify = (PREVIEW_GLYPH_HEIGHT / font_height).floor().max(1.0);

        let segment_widths: Vec<f32> = segments
            .iter()
            .map(|seg| Self::calculate_segment_width(seg, font_width))
            .collect();

        let content_width = segment_widths.iter().sum::<f32>();
        let total_width = content_width + BORDER_WIDTH * 2.0 + SHADOW_PADDING * 2.0;
        let total_height = SEGMENT_HEIGHT + SHADOW_PADDING * 2.0;

        Self {
            segment_widths,
            total_width,
            total_height,
            content_width,
            font_width,
            font_height,
            preview_magnify,
        }
    }

    /// Calculate width for a single segment.
    fn calculate_segment_width(content: &SegmentContentType, font_width: f32) -> f32 {
        let content_width = match content {
            SegmentContentType::Text(char_count) => *char_count as f32 * font_width,
            SegmentContentType::Char => font_width * CHAR_MAGNIFICATION,
        };
        content_width + SEGMENT_PADDING_H * 2.0
    }

    /// Create layout with default 8x16 font.
    pub fn default_font(segments: &[SegmentContentType]) -> Self {
        Self::new(segments, None)
    }

    // ───────────────────────────────────────────────────────────────────────
    // Segment Geometry
    // ───────────────────────────────────────────────────────────────────────

    /// Get the X offset where content starts (after shadow and border).
    #[inline]
    pub fn content_start_x(&self) -> f32 {
        SHADOW_PADDING + BORDER_WIDTH
    }

    /// Get the Y offset where content starts.
    #[inline]
    pub fn content_start_y(&self) -> f32 {
        SHADOW_PADDING + BORDER_WIDTH
    }

    /// Get the content height (excluding shadow and border).
    #[inline]
    pub fn content_height(&self) -> f32 {
        self.total_height - SHADOW_PADDING * 2.0 - BORDER_WIDTH * 2.0
    }

    /// Get the X offset for a segment.
    pub fn segment_x(&self, idx: usize) -> f32 {
        let mut x = self.content_start_x();
        for i in 0..idx {
            x += self.segment_widths.get(i).copied().unwrap_or(0.0);
        }
        x
    }

    /// Get the width of a segment.
    pub fn segment_width(&self, idx: usize) -> f32 {
        self.segment_widths.get(idx).copied().unwrap_or(0.0)
    }

    /// Get the rectangle for a segment (in widget-local coordinates).
    pub fn segment_rect(&self, idx: usize) -> Rectangle {
        Rectangle {
            x: self.segment_x(idx),
            y: self.content_start_y(),
            width: self.segment_width(idx),
            height: self.content_height(),
        }
    }

    // ───────────────────────────────────────────────────────────────────────
    // Text/Glyph Geometry
    // ───────────────────────────────────────────────────────────────────────

    /// Get the Y offset for centering text in the content area.
    pub fn text_y(&self) -> f32 {
        let content_y = self.content_start_y();
        let content_h = self.content_height();
        (content_y + (content_h - self.font_height) / 2.0).floor()
    }

    /// Get the Y offset for centering a preview glyph in the content area.
    pub fn preview_glyph_y(&self) -> f32 {
        let content_y = self.content_start_y();
        let content_h = self.content_height();
        let glyph_h = self.font_height * self.preview_magnify;
        (content_y + (content_h - glyph_h) / 2.0).floor()
    }

    /// Get preview glyph dimensions.
    pub fn preview_glyph_size(&self) -> (f32, f32) {
        let w = self.font_width * self.preview_magnify;
        let h = self.font_height * self.preview_magnify;
        (w, h)
    }

    /// Get the centered X position for text within a segment.
    pub fn text_x_centered(&self, segment_idx: usize, text_width: f32) -> f32 {
        let seg_x = self.segment_x(segment_idx);
        let seg_w = self.segment_width(segment_idx);
        seg_x + (seg_w - text_width) / 2.0
    }

    /// Get the centered X position for a preview glyph within a segment.
    pub fn preview_glyph_x_centered(&self, segment_idx: usize) -> f32 {
        let seg_x = self.segment_x(segment_idx);
        let seg_w = self.segment_width(segment_idx);
        let (glyph_w, _) = self.preview_glyph_size();
        (seg_x + (seg_w - glyph_w) / 2.0).floor()
    }

    // ───────────────────────────────────────────────────────────────────────
    // Hit Testing
    // ───────────────────────────────────────────────────────────────────────

    /// Find which segment contains the given point (widget-local coordinates).
    ///
    /// # Arguments
    /// * `pos` - Point in widget-local coordinates.
    ///
    /// # Returns
    /// `Some(segment_index)` if the point is within a segment, `None` otherwise.
    pub fn hit_test(&self, pos: Point) -> Option<usize> {
        let content_x = self.content_start_x();
        let local_x = pos.x - content_x;
        
        if local_x < 0.0 {
            return None;
        }

        let mut seg_x = 0.0;
        for (idx, &width) in self.segment_widths.iter().enumerate() {
            if local_x >= seg_x && local_x < seg_x + width {
                return Some(idx);
            }
            seg_x += width;
        }

        None
    }

    /// Hit test and return as GPU uniform representation.
    pub fn hit_test_uniform(&self, pos: Point) -> u32 {
        self.hit_test(pos).map(|i| i as u32).unwrap_or(NO_HOVER)
    }
}

// ═══════════════════════════════════════════════════════════════════════════
// Scaled Layout (for GPU rendering in physical pixels)
// ═══════════════════════════════════════════════════════════════════════════

/// Layout scaled to physical pixels for GPU rendering.
#[derive(Debug, Clone)]
pub struct SegmentedLayoutScaled {
    /// Base layout (logical pixels).
    pub base: SegmentedLayout,
    /// Scale factor (DPI scaling).
    pub scale: f32,
}

impl SegmentedLayoutScaled {
    /// Create a scaled layout.
    pub fn new(base: SegmentedLayout, scale: f32) -> Self {
        Self { base, scale }
    }

    /// Scale a logical pixel value to physical pixels.
    #[inline]
    pub fn px(&self, logical: f32) -> f32 {
        (logical * self.scale).round()
    }

    /// Content start X in physical pixels.
    pub fn content_start_x_px(&self) -> f32 {
        self.px(self.base.content_start_x())
    }

    /// Content height in physical pixels.
    pub fn content_height_px(&self) -> f32 {
        self.px(self.base.content_height())
    }

    /// Segment X in physical pixels.
    pub fn segment_x_px(&self, idx: usize) -> f32 {
        let mut x = self.content_start_x_px();
        for i in 0..idx {
            x += self.px(self.base.segment_width(i));
        }
        x.floor()
    }

    /// Segment width in physical pixels.
    pub fn segment_width_px(&self, idx: usize) -> f32 {
        self.px(self.base.segment_width(idx))
    }

    /// Preview glyph magnification (HiDPI-safe integer scaling).
    pub fn preview_magnify_px(&self) -> f32 {
        let target_h = self.px(PREVIEW_GLYPH_HEIGHT);
        (target_h / self.base.font_height).floor().max(1.0)
    }

    /// Preview glyph size in physical pixels.
    pub fn preview_glyph_size_px(&self) -> (f32, f32) {
        let mag = self.preview_magnify_px();
        let w = (self.base.font_width * mag).round().max(1.0);
        let h = (self.base.font_height * mag).round().max(1.0);
        (w, h)
    }

    /// Preview glyph Y in physical pixels (centered).
    pub fn preview_glyph_y_px(&self) -> f32 {
        let (_, h) = self.preview_glyph_size_px();
        let shadow_px = self.px(SHADOW_PADDING);
        let border_px = self.px(BORDER_WIDTH);
        let control_h = self.content_height_px();
        (shadow_px + border_px + ((control_h - h) / 2.0)).floor()
    }

    /// Preview glyph X centered in segment (physical pixels).
    pub fn preview_glyph_x_centered_px(&self, segment_idx: usize) -> f32 {
        let seg_x = self.segment_x_px(segment_idx);
        let seg_w = self.segment_width_px(segment_idx);
        let (glyph_w, _) = self.preview_glyph_size_px();
        (seg_x + ((seg_w - glyph_w) / 2.0)).floor()
    }
}

// ═══════════════════════════════════════════════════════════════════════════
// Helper Functions
// ═══════════════════════════════════════════════════════════════════════════

/// Calculate segment width for a text segment.
pub fn text_segment_width(text: &str, font: Option<&BitFont>) -> f32 {
    let font_width = font.map(|f| f.size().width as f32).unwrap_or(8.0);
    let char_count = text.chars().count();
    char_count as f32 * font_width + SEGMENT_PADDING_H * 2.0
}

/// Calculate segment width for a char segment.
pub fn char_segment_width(font: Option<&BitFont>) -> f32 {
    let font_width = font.map(|f| f.size().width as f32).unwrap_or(8.0);
    font_width * CHAR_MAGNIFICATION + SEGMENT_PADDING_H * 2.0
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_layout_basic() {
        let segments = vec![
            SegmentContentType::Text(4), // "Test"
            SegmentContentType::Char,
        ];
        let layout = SegmentedLayout::default_font(&segments);
        
        assert_eq!(layout.segment_widths.len(), 2);
        assert!(layout.total_width > 0.0);
        assert!(layout.total_height > 0.0);
    }

    #[test]
    fn test_hit_test() {
        let segments = vec![
            SegmentContentType::Text(4),
            SegmentContentType::Text(4),
        ];
        let layout = SegmentedLayout::default_font(&segments);
        
        // Hit first segment
        let pos = Point::new(layout.segment_x(0) + 5.0, layout.content_start_y() + 5.0);
        assert_eq!(layout.hit_test(pos), Some(0));
        
        // Hit second segment
        let pos = Point::new(layout.segment_x(1) + 5.0, layout.content_start_y() + 5.0);
        assert_eq!(layout.hit_test(pos), Some(1));
        
        // Miss (before first segment)
        let pos = Point::new(0.0, layout.content_start_y() + 5.0);
        assert_eq!(layout.hit_test(pos), None);
    }
}
