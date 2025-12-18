//! Tests for caret UV calculation in scrolled viewports.
//!
//! Regression tests for bug where caret was rendered at wrong position
//! when the document was scrolled and visible_width != texture_width.

use icy_engine_gui::compute_caret_position;

/// Reproduces the bug scenario from the issue:
/// - 80x25 terminal (640x400 content)
/// - Zoomed to 3.5x
/// - Scrolled to (291.619, 188.343)
/// - Visible area: 348.381 x 211.657 pixels
/// - Caret at cell (65, 13)
///
/// Expected: Caret should be visible at relative position (228.381, 19.657)
/// Bug: Caret was normalized using texture_width (640) instead of visible_width (348.381)
#[test]
fn caret_position_with_fractional_scroll() {
    let caret_col = 65;
    let caret_row = 13;
    let font_width = 8.0;
    let font_height = 16.0;
    let scroll_offset_x = 291.619;
    let scroll_offset_y = 188.343;
    let visible_width = 348.381;
    let visible_height = 211.657;
    let scan_lines = false;

    let pos = compute_caret_position(
        caret_col,
        caret_row,
        font_width,
        font_height,
        scroll_offset_x,
        scroll_offset_y,
        visible_width,
        visible_height,
        scan_lines,
    );

    // Absolute position: 65*8 = 520, 13*16 = 208
    // Relative position: 520 - 291.619 = 228.381, 208 - 188.343 = 19.657
    let expected_x = 520.0 - 291.619; // 228.381
    let expected_y = 208.0 - 188.343; // 19.657

    assert!((pos.x - expected_x).abs() < 0.01, "x: expected {}, got {}", expected_x, pos.x);
    assert!((pos.y - expected_y).abs() < 0.01, "y: expected {}, got {}", expected_y, pos.y);
    assert!(pos.is_visible, "caret should be visible");

    // Verify UV calculation (what the shader needs)
    // UV should be relative position / visible size
    let uv_x = pos.x / visible_width;
    let uv_y = pos.y / visible_height;

    // UV should be 228.381 / 348.381 ≈ 0.656
    // NOT 229 / 640 ≈ 0.358 (the buggy value)
    let expected_uv_x = expected_x / visible_width; // ~0.656
    let expected_uv_y = expected_y / visible_height; // ~0.093

    assert!((uv_x - expected_uv_x).abs() < 0.01, "uv_x: expected {}, got {}", expected_uv_x, uv_x);
    assert!((uv_y - expected_uv_y).abs() < 0.01, "uv_y: expected {}, got {}", expected_uv_y, uv_y);

    // The buggy calculation would give:
    let buggy_uv_x = 229.0 / 640.0; // ~0.358 (wrong!)
    assert!((uv_x - buggy_uv_x).abs() > 0.1, "uv_x should NOT equal buggy value {}", buggy_uv_x);
}

/// Tests that scroll truncation to i32 causes position drift
#[test]
fn scroll_truncation_causes_drift() {
    let caret_col = 10;
    let font_width = 8.0;
    let font_height = 16.0;
    let scroll_offset_x = 8.9; // Almost a full character scrolled
    let scroll_offset_y = 0.0;
    let visible_width = 640.0;
    let visible_height = 400.0;

    let pos = compute_caret_position(
        caret_col,
        0,
        font_width,
        font_height,
        scroll_offset_x,
        scroll_offset_y,
        visible_width,
        visible_height,
        false,
    );

    // Correct: 10*8 - 8.9 = 71.1
    let expected_x = 80.0 - 8.9; // 71.1

    // Bug (with i32 truncation): 10*8 - 8 = 72
    let buggy_x = 80.0 - 8.0; // 72.0

    assert!((pos.x - expected_x).abs() < 0.001, "x: expected {}, got {}", expected_x, pos.x);
    assert!((pos.x - buggy_x).abs() > 0.5, "position should differ from buggy calculation");
}

/// Tests caret visibility bounds checking with fractional scroll
#[test]
fn caret_visibility_with_fractional_scroll() {
    let font_width = 8.0;
    let font_height = 16.0;
    let visible_width = 100.0;
    let visible_height = 100.0;

    // Caret at position that would be just barely visible with correct calculation,
    // but invisible with truncated scroll
    let pos = compute_caret_position(
        12,
        0,
        font_width,
        font_height,
        95.5,
        0.0, // scroll_x = 95.5
        visible_width,
        visible_height,
        false,
    );

    // 12*8 - 95.5 = 96 - 95.5 = 0.5 (barely visible on left edge)
    assert!((pos.x - 0.5).abs() < 0.001);
    assert!(pos.is_visible);

    // With truncated scroll (95): 96 - 95 = 1 (different position!)
    let buggy_x = 96.0 - 95.0;
    assert!((pos.x - buggy_x).abs() > 0.4);
}
