//! Pure calculation functions for terminal rendering
//!
//! This module contains pure, testable functions for geometric calculations
//! used in terminal rendering. These functions have no side effects and
//! no dependencies on GPU, UI frameworks, or global state.

use crate::ScalingMode;

/// Result of terminal rect calculation for shader consumption.
///
/// The terminal rect defines where the terminal content is rendered
/// within the viewport, enabling proper centering and scaling.
#[derive(Debug, Clone, PartialEq)]
pub struct TerminalRect {
    /// X offset where terminal starts (normalized 0-1)
    pub start_x: f32,
    /// Y offset where terminal starts (normalized 0-1)
    pub start_y: f32,
    /// Width of terminal area (normalized 0-1)
    pub width_n: f32,
    /// Height of terminal area (normalized 0-1)
    pub height_n: f32,
    /// X offset in pixels (for RenderInfo)
    pub offset_x: f32,
    /// Y offset in pixels (for RenderInfo)
    pub offset_y: f32,
    /// Scaled width in pixels
    pub scaled_w: f32,
    /// Scaled height in pixels
    pub scaled_h: f32,
    /// The computed zoom/scale factor
    pub scale: f32,
}

/// Computes where the terminal content should be positioned within the viewport.
///
/// This calculation determines centering and scaling of the terminal content.
/// The result is used by the WGSL shader to properly position the rendered content.
///
/// # Arguments
/// * `visible_width` - Width of visible terminal content in pixels
/// * `visible_height` - Height of visible terminal content in pixels
/// * `avail_w` - Available viewport width in pixels
/// * `avail_h` - Available viewport height in pixels
/// * `scaling_mode` - The scaling mode (Auto, Manual zoom level, etc.)
/// * `use_integer_scaling` - Whether to use integer scaling for pixel-perfect rendering
///
/// # Returns
/// A `TerminalRect` with both normalized (0-1) and pixel coordinates.
pub fn compute_terminal_rect(
    visible_width: f32,
    visible_height: f32,
    avail_w: f32,
    avail_h: f32,
    scaling_mode: &ScalingMode,
    use_integer_scaling: bool,
) -> TerminalRect {
    let term_w = visible_width.max(1.0);
    let term_h = visible_height.max(1.0);
    let avail_w = avail_w.max(1.0);
    let avail_h = avail_h.max(1.0);

    let scale = scaling_mode.compute_zoom(term_w, term_h, avail_w, avail_h, use_integer_scaling);
    let scaled_w = (term_w * scale).min(avail_w);
    let scaled_h = (term_h * scale).min(avail_h);

    let offset_x = ((avail_w - scaled_w) / 2.0).max(0.0);
    let offset_y = ((avail_h - scaled_h) / 2.0).max(0.0);
    let start_x = offset_x / avail_w;
    let start_y = offset_y / avail_h;
    let width_n = scaled_w / avail_w;
    let height_n = scaled_h / avail_h;

    TerminalRect {
        start_x,
        start_y,
        width_n,
        height_n,
        offset_x,
        offset_y,
        scaled_w,
        scaled_h,
        scale,
    }
}

/// Parameters for viewport calculation.
#[derive(Debug, Clone, PartialEq)]
pub struct ViewportParams {
    /// Width of visible content in pixels
    pub visible_width: f32,
    /// Height of visible content in pixels
    pub visible_height: f32,
    /// Scroll offset Y in pixels (clamped to valid range)
    pub scroll_offset_y: f32,
    /// Scroll offset X in pixels (clamped to valid range)
    pub scroll_offset_x: f32,
    /// The computed zoom factor
    pub zoom: f32,
}

/// Computes viewport parameters for Auto scaling mode.
///
/// In Auto mode, the content fills the viewport completely.
///
/// # Arguments
/// * `res_w` - Resolution width (terminal width × font width)
/// * `res_h` - Resolution height (terminal height × font height)
/// * `content_width` - Total content width including scrollback
/// * `content_height` - Total content height including scrollback
/// * `scroll_x` - Requested horizontal scroll position
/// * `scroll_y` - Requested vertical scroll position
pub fn compute_viewport_auto(res_w: f32, res_h: f32, content_width: f32, content_height: f32, scroll_x: f32, scroll_y: f32) -> ViewportParams {
    let visible_width = res_w;
    let visible_height = res_h;

    let max_scroll_y = (content_height - visible_height).max(0.0);
    let scroll_offset_y = scroll_y.clamp(0.0, max_scroll_y);

    let max_scroll_x = (content_width - visible_width).max(0.0);
    let scroll_offset_x = scroll_x.clamp(0.0, max_scroll_x);

    ViewportParams {
        visible_width,
        visible_height,
        scroll_offset_y,
        scroll_offset_x,
        zoom: 1.0, // Auto mode doesn't have explicit zoom
    }
}

/// Computes viewport parameters for Manual scaling mode.
///
/// In Manual mode, a specific zoom level is used and visible area is calculated.
///
/// # Arguments
/// * `res_w` - Resolution width (terminal width × font width)
/// * `original_res_h` - Original resolution height (before fit_terminal_height)
/// * `bounds_width` - Widget bounds width
/// * `bounds_height` - Widget bounds height
/// * `content_width` - Total content width
/// * `content_height` - Total content height
/// * `scroll_x` - Requested horizontal scroll position
/// * `scroll_y` - Requested vertical scroll position
/// * `scaling_mode` - The scaling mode with zoom level
/// * `use_integer_scaling` - Whether to use integer scaling
pub fn compute_viewport_manual(
    res_w: f32,
    original_res_h: f32,
    bounds_width: f32,
    bounds_height: f32,
    content_width: f32,
    content_height: f32,
    scroll_x: f32,
    scroll_y: f32,
    scaling_mode: &ScalingMode,
    use_integer_scaling: bool,
) -> ViewportParams {
    let zoom = scaling_mode
        .compute_zoom(res_w, original_res_h, bounds_width, bounds_height, use_integer_scaling)
        .max(0.001);

    let visible_width = (bounds_width / zoom).min(res_w);
    let visible_height = (bounds_height / zoom).min(original_res_h);

    let max_scroll_y = (content_height - visible_height).max(0.0);
    let scroll_offset_y = scroll_y.clamp(0.0, max_scroll_y);

    let max_scroll_x = (content_width - visible_width).max(0.0);
    let scroll_offset_x = scroll_x.clamp(0.0, max_scroll_x);

    ViewportParams {
        visible_width,
        visible_height,
        scroll_offset_y,
        scroll_offset_x,
        zoom,
    }
}

/// Result of tile index calculation.
#[derive(Debug, Clone, PartialEq)]
pub struct TileIndices {
    /// Indices of tiles to render (in order)
    pub indices: Vec<i32>,
    /// Y position where the first tile starts (in pixels)
    pub first_slice_start_y: f32,
}

/// Computes which tiles need to be rendered based on scroll position.
///
/// This implements a sliding window approach where we render:
/// - 1 tile above the visible area (for smooth scrolling)
/// - All visible tiles
/// - 1 tile below the visible area
///
/// # Arguments
/// * `scroll_offset_y` - Current vertical scroll position in pixels
/// * `visible_height` - Height of visible area in pixels
/// * `full_content_height` - Total content height in pixels
/// * `tile_height` - Height of each tile in pixels
/// * `max_tiles` - Maximum number of tiles to render (hardware limit)
///
/// # Returns
/// A `TileIndices` struct with the tile indices and first tile Y position.
pub fn compute_tile_indices(scroll_offset_y: f32, visible_height: f32, full_content_height: f32, tile_height: f32, max_tiles: usize) -> TileIndices {
    let tile_height = tile_height.max(1.0);

    // Current tile index based on scroll position
    let current_tile_idx = (scroll_offset_y / tile_height).floor() as i32;
    let max_tile_idx = ((full_content_height / tile_height).ceil() as i32 - 1).max(0);

    // Dynamic slice count: visible tiles + 1 above + 1 below
    let visible_tiles = (visible_height / tile_height).ceil().max(1.0) as i32;
    let mut desired_count = (visible_tiles + 2).clamp(1, max_tiles as i32);
    desired_count = desired_count.min(max_tile_idx + 1);

    // Start one tile above current, but clamp so we can still fit desired_count tiles
    let max_first_tile_idx = (max_tile_idx - (desired_count - 1)).max(0);
    let first_tile_idx = (current_tile_idx - 1).clamp(0, max_first_tile_idx);

    // Calculate tile indices to render
    let mut indices: Vec<i32> = Vec::with_capacity(desired_count as usize);
    for i in 0..desired_count {
        let idx = first_tile_idx + i;
        if idx <= max_tile_idx {
            indices.push(idx);
        }
    }

    let first_slice_start_y = first_tile_idx as f32 * tile_height;

    TileIndices { indices, first_slice_start_y }
}

/// Result of caret position calculation.
#[derive(Debug, Clone, PartialEq)]
pub struct CaretPosition {
    /// Caret X position relative to visible area (in pixels)
    pub x: f32,
    /// Caret Y position relative to visible area (in pixels)
    pub y: f32,
    /// Width of caret in pixels
    pub width: f32,
    /// Height of caret in pixels
    pub height: f32,
    /// Whether the caret is visible in current viewport
    pub is_visible: bool,
}

/// Computes the caret position for rendering.
///
/// # Arguments
/// * `caret_col` - Caret column position (character index)
/// * `caret_row` - Caret row position (line index)
/// * `font_width` - Width of a character cell in pixels
/// * `font_height` - Height of a character cell in pixels
/// * `scroll_offset_x` - Horizontal scroll offset in pixels
/// * `scroll_offset_y` - Vertical scroll offset in pixels
/// * `visible_width` - Width of visible area in pixels
/// * `visible_height` - Height of visible area in pixels
/// * `scan_lines` - Whether scanlines are enabled (doubles effective height)
///
/// # Returns
/// A `CaretPosition` with coordinates relative to visible area.
pub fn compute_caret_position(
    caret_col: i32,
    caret_row: i32,
    font_width: f32,
    font_height: f32,
    scroll_offset_x: f32,
    scroll_offset_y: f32,
    visible_width: f32,
    visible_height: f32,
    scan_lines: bool,
) -> CaretPosition {
    let scan_mult = if scan_lines { 2.0 } else { 1.0 };
    let effective_font_height = font_height * scan_mult;

    // Absolute position in document
    let abs_x = caret_col as f32 * font_width;
    let abs_y = caret_row as f32 * effective_font_height;

    // Position relative to visible area
    let x = abs_x - scroll_offset_x;
    let y = abs_y - scroll_offset_y;

    // Check if caret is visible
    let is_visible = x >= -font_width && x <= visible_width && y >= -effective_font_height && y <= visible_height;

    CaretPosition {
        x,
        y,
        width: font_width,
        height: effective_font_height,
        is_visible,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // ============================================================
    // Tests for compute_terminal_rect
    // ============================================================

    #[test]
    fn terminal_rect_centers_at_50_percent_zoom() {
        let content_w = 800.0;
        let content_h = 600.0;
        let viewport_w = 1000.0;
        let viewport_h = 800.0;
        let scaling_mode = ScalingMode::Manual(0.5);

        let rect = compute_terminal_rect(content_w, content_h, viewport_w, viewport_h, &scaling_mode, false);

        // At 50% zoom: scaled = 400x300, centered in 1000x800
        assert!((rect.scaled_w - 400.0).abs() < 0.001);
        assert!((rect.scaled_h - 300.0).abs() < 0.001);
        assert!((rect.offset_x - 300.0).abs() < 0.001); // (1000-400)/2
        assert!((rect.offset_y - 250.0).abs() < 0.001); // (800-300)/2
    }

    #[test]
    fn terminal_rect_at_100_percent_fills_matching_viewport() {
        let size = 800.0;
        let scaling_mode = ScalingMode::Manual(1.0);

        let rect = compute_terminal_rect(size, size, size, size, &scaling_mode, false);

        assert!((rect.start_x).abs() < 0.001);
        assert!((rect.start_y).abs() < 0.001);
        assert!((rect.width_n - 1.0).abs() < 0.001);
        assert!((rect.height_n - 1.0).abs() < 0.001);
    }

    #[test]
    fn terminal_rect_handles_small_content_in_large_viewport() {
        // 80x25 terminal (640x400 at 8x16 font) in 1920x1080 viewport
        let content_w = 640.0;
        let content_h = 400.0;
        let viewport_w = 1920.0;
        let viewport_h = 1080.0;
        let scaling_mode = ScalingMode::Manual(1.0);

        let rect = compute_terminal_rect(content_w, content_h, viewport_w, viewport_h, &scaling_mode, false);

        // Content should be centered
        assert!((rect.offset_x - (1920.0 - 640.0) / 2.0).abs() < 0.001);
        assert!((rect.offset_y - (1080.0 - 400.0) / 2.0).abs() < 0.001);
        assert!((rect.scaled_w - 640.0).abs() < 0.001);
        assert!((rect.scaled_h - 400.0).abs() < 0.001);
    }

    #[test]
    fn terminal_rect_clamps_content_to_viewport() {
        // Content larger than viewport at 200% zoom
        let content_w = 800.0;
        let content_h = 600.0;
        let viewport_w = 400.0;
        let viewport_h = 300.0;
        let scaling_mode = ScalingMode::Manual(2.0);

        let rect = compute_terminal_rect(content_w, content_h, viewport_w, viewport_h, &scaling_mode, false);

        // Scaled would be 1600x1200, but clamped to 400x300
        assert!((rect.scaled_w - 400.0).abs() < 0.001);
        assert!((rect.scaled_h - 300.0).abs() < 0.001);
        assert!((rect.offset_x).abs() < 0.001); // No centering when clamped
        assert!((rect.offset_y).abs() < 0.001);
    }

    #[test]
    fn terminal_rect_handles_zero_dimensions() {
        let scaling_mode = ScalingMode::Manual(1.0);

        // Should not panic, dimensions clamped to 1.0
        let rect = compute_terminal_rect(0.0, 0.0, 0.0, 0.0, &scaling_mode, false);

        assert!(rect.start_x.is_finite());
        assert!(rect.start_y.is_finite());
    }

    // ============================================================
    // Tests for compute_viewport_auto
    // ============================================================

    #[test]
    fn viewport_auto_uses_full_resolution() {
        let params = compute_viewport_auto(800.0, 600.0, 800.0, 1200.0, 0.0, 100.0);

        assert!((params.visible_width - 800.0).abs() < 0.001);
        assert!((params.visible_height - 600.0).abs() < 0.001);
    }

    #[test]
    fn viewport_auto_clamps_scroll() {
        let params = compute_viewport_auto(800.0, 600.0, 800.0, 1200.0, 0.0, 1000.0);

        // Max scroll = 1200 - 600 = 600
        assert!((params.scroll_offset_y - 600.0).abs() < 0.001);
    }

    #[test]
    fn viewport_auto_prevents_negative_scroll() {
        let params = compute_viewport_auto(800.0, 600.0, 800.0, 600.0, -100.0, -100.0);

        assert!((params.scroll_offset_x).abs() < 0.001);
        assert!((params.scroll_offset_y).abs() < 0.001);
    }

    // ============================================================
    // Tests for compute_viewport_manual
    // ============================================================

    #[test]
    fn viewport_manual_calculates_visible_area() {
        let scaling_mode = ScalingMode::Manual(0.5);
        let params = compute_viewport_manual(
            800.0,
            600.0, // resolution
            400.0,
            300.0, // bounds (at 50% zoom, shows 800x600)
            800.0,
            1200.0, // content
            0.0,
            0.0, // scroll
            &scaling_mode,
            false,
        );

        // At 50% zoom, visible = bounds / zoom = 400/0.5 = 800, 300/0.5 = 600
        // But clamped to resolution
        assert!((params.visible_width - 800.0).abs() < 0.001);
        assert!((params.visible_height - 600.0).abs() < 0.001);
        assert!((params.zoom - 0.5).abs() < 0.001);
    }

    #[test]
    fn viewport_manual_at_200_percent_shows_less() {
        let scaling_mode = ScalingMode::Manual(2.0);
        let params = compute_viewport_manual(
            800.0,
            600.0, // resolution
            800.0,
            600.0, // bounds
            800.0,
            600.0, // content
            0.0,
            0.0, // scroll
            &scaling_mode,
            false,
        );

        // At 200% zoom, visible = 800/2 = 400, 600/2 = 300
        assert!((params.visible_width - 400.0).abs() < 0.001);
        assert!((params.visible_height - 300.0).abs() < 0.001);
    }

    // ============================================================
    // Tests for compute_tile_indices
    // ============================================================

    #[test]
    fn tile_indices_at_top() {
        let result = compute_tile_indices(0.0, 400.0, 1600.0, 256.0, 10);

        // At scroll 0: visible tiles = ceil(400/256) = 2
        // Desired = 2 + 2 = 4, but can't go above 0, so starts at 0
        assert_eq!(result.indices[0], 0);
        assert!((result.first_slice_start_y).abs() < 0.001);
    }

    #[test]
    fn tile_indices_in_middle() {
        // Scroll to middle of content
        let result = compute_tile_indices(512.0, 400.0, 1600.0, 256.0, 10);

        // At scroll 512: current_tile = floor(512/256) = 2
        // first_tile = 2 - 1 = 1
        assert!(result.indices.contains(&1));
        assert!(result.indices.contains(&2));
        assert!((result.first_slice_start_y - 256.0).abs() < 0.001);
    }

    #[test]
    fn tile_indices_respects_max_tiles() {
        let result = compute_tile_indices(0.0, 10000.0, 10000.0, 256.0, 3);

        // Even with huge visible area, limited to max_tiles
        assert!(result.indices.len() <= 3);
    }

    #[test]
    fn tile_indices_handles_small_content() {
        // Content smaller than one tile
        let result = compute_tile_indices(0.0, 100.0, 100.0, 256.0, 10);

        assert_eq!(result.indices.len(), 1);
        assert_eq!(result.indices[0], 0);
    }

    // ============================================================
    // Tests for compute_caret_position
    // ============================================================

    #[test]
    fn caret_position_at_origin() {
        let pos = compute_caret_position(0, 0, 8.0, 16.0, 0.0, 0.0, 800.0, 600.0, false);

        assert!((pos.x).abs() < 0.001);
        assert!((pos.y).abs() < 0.001);
        assert!(pos.is_visible);
    }

    #[test]
    fn caret_position_with_scroll() {
        // Caret at col 10, row 5
        // Scroll offset 40, 32
        let pos = compute_caret_position(10, 5, 8.0, 16.0, 40.0, 32.0, 800.0, 600.0, false);

        // Absolute: x = 10*8 = 80, y = 5*16 = 80
        // Relative: x = 80-40 = 40, y = 80-32 = 48
        assert!((pos.x - 40.0).abs() < 0.001);
        assert!((pos.y - 48.0).abs() < 0.001);
        assert!(pos.is_visible);
    }

    #[test]
    fn caret_position_offscreen() {
        // Caret at row 100, scrolled to top
        let pos = compute_caret_position(0, 100, 8.0, 16.0, 0.0, 0.0, 800.0, 600.0, false);

        // y = 100 * 16 = 1600, way beyond visible_height 600
        assert!(!pos.is_visible);
    }

    #[test]
    fn caret_position_with_scanlines() {
        let pos = compute_caret_position(0, 5, 8.0, 16.0, 0.0, 0.0, 800.0, 600.0, true);

        // With scanlines, effective height is doubled
        // y = 5 * 32 = 160
        assert!((pos.y - 160.0).abs() < 0.001);
        assert!((pos.height - 32.0).abs() < 0.001);
    }

    #[test]
    fn caret_dimensions() {
        let pos = compute_caret_position(0, 0, 8.0, 16.0, 0.0, 0.0, 800.0, 600.0, false);

        assert!((pos.width - 8.0).abs() < 0.001);
        assert!((pos.height - 16.0).abs() < 0.001);
    }
}
