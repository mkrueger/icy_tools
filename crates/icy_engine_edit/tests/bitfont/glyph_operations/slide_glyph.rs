//! Slide glyph tests (rotate with wrap)
//!
//! Tests the slide operation which rotates pixels within the selection.
//! Unlike move, slide uses Rust's rotate_left/rotate_right for efficient rotation.

use icy_engine_edit::bitfont::{BitFontEditState, BitFontUndoState};

/// Helper to create a state with a test pattern
fn create_state_with_pattern(ch: char, pattern: Vec<Vec<bool>>) -> BitFontEditState {
    let mut state = BitFontEditState::new();
    state.set_glyph_pixels(ch, pattern).unwrap();
    state
}

/// Create a horizontal line at given row
fn horizontal_line_pattern(width: usize, height: usize, row: usize) -> Vec<Vec<bool>> {
    let mut pattern = vec![vec![false; width]; height];
    if row < height {
        for col in 0..width {
            pattern[row][col] = true;
        }
    }
    pattern
}

/// Create a vertical line at given column
fn vertical_line_pattern(width: usize, height: usize, col: usize) -> Vec<Vec<bool>> {
    let mut pattern = vec![vec![false; width]; height];
    if col < width {
        for row in 0..height {
            pattern[row][col] = true;
        }
    }
    pattern
}

#[test]
fn test_slide_right() {
    // Vertical line at column 0, slide right should rotate to column 1
    let pattern = vertical_line_pattern(8, 16, 0);
    let mut state = create_state_with_pattern('A', pattern);
    state.set_selected_char('A');

    state.slide_glyph(1, 0).unwrap();

    let result = state.get_glyph_pixels('A');
    // Column 0 should now be empty, column 1 should have the line
    assert!(!result[0][0]);
    assert!(result[0][1]);
}

#[test]
fn test_slide_left() {
    let pattern = vertical_line_pattern(8, 16, 3);
    let mut state = create_state_with_pattern('A', pattern);
    state.set_selected_char('A');

    state.slide_glyph(-1, 0).unwrap();

    let result = state.get_glyph_pixels('A');
    assert!(!result[0][3]);
    assert!(result[0][2]);
}

#[test]
fn test_slide_down() {
    // Horizontal line at row 0, slide down should rotate to row 1
    let pattern = horizontal_line_pattern(8, 16, 0);
    let mut state = create_state_with_pattern('A', pattern);
    state.set_selected_char('A');

    state.slide_glyph(0, 1).unwrap();

    let result = state.get_glyph_pixels('A');
    assert!(!result[0][0]);
    assert!(result[1][0]);
}

#[test]
fn test_slide_up() {
    let pattern = horizontal_line_pattern(8, 16, 5);
    let mut state = create_state_with_pattern('A', pattern);
    state.set_selected_char('A');

    state.slide_glyph(0, -1).unwrap();

    let result = state.get_glyph_pixels('A');
    assert!(!result[5][0]);
    assert!(result[4][0]);
}

#[test]
fn test_slide_right_wraps() {
    // Line at rightmost column, slide right wraps to column 0
    let pattern = vertical_line_pattern(8, 16, 7);
    let mut state = create_state_with_pattern('A', pattern);
    state.set_selected_char('A');

    state.slide_glyph(1, 0).unwrap();

    let result = state.get_glyph_pixels('A');
    assert!(!result[0][7], "column 7 should be empty");
    assert!(result[0][0], "should wrap to column 0");
}

#[test]
fn test_slide_down_wraps() {
    // Line at bottom row, slide down wraps to row 0
    let pattern = horizontal_line_pattern(8, 16, 15);
    let mut state = create_state_with_pattern('A', pattern);
    state.set_selected_char('A');

    state.slide_glyph(0, 1).unwrap();

    let result = state.get_glyph_pixels('A');
    assert!(!result[15][0], "row 15 should be empty");
    assert!(result[0][0], "should wrap to row 0");
}

#[test]
fn test_slide_with_selection() {
    // Create a pattern and select only part of it
    let mut pattern = vec![vec![false; 8]; 16];
    // Fill a 4x4 block at (2, 2) to (5, 5)
    for row in 2..6 {
        for col in 2..6 {
            pattern[row][col] = true;
        }
    }

    let mut state = create_state_with_pattern('A', pattern);
    state.set_selected_char('A');

    // Select a smaller region (3, 3) to (4, 4)
    state.set_selection(Some((3, 3, 4, 4)));

    state.slide_glyph(1, 0).unwrap();

    // Only the selected region should have rotated
    let result = state.get_glyph_pixels('A');

    // Pixels outside selection should be unchanged
    assert!(result[2][2], "pixels outside selection unchanged");
    assert!(result[5][5], "pixels outside selection unchanged");
}

#[test]
fn test_slide_undo() {
    let pattern = vertical_line_pattern(8, 16, 3);
    let mut state = create_state_with_pattern('A', pattern.clone());
    state.set_selected_char('A');

    state.slide_glyph(2, 0).unwrap();

    // Verify slide happened
    assert!(!state.get_glyph_pixels('A')[0][3]);
    assert!(state.get_glyph_pixels('A')[0][5]);

    // Undo
    state.undo().unwrap();

    // Should be back to original
    assert!(state.get_glyph_pixels('A')[0][3], "line should be restored after undo");
}

#[test]
fn test_slide_full_rotation_returns_to_original() {
    let pattern = vertical_line_pattern(8, 16, 3);
    let original = pattern.clone();
    let mut state = create_state_with_pattern('A', pattern);
    state.set_selected_char('A');

    // Slide right by full width should return to original
    state.slide_glyph(8, 0).unwrap();

    let result = state.get_glyph_pixels('A');
    assert_eq!(result, &original, "full rotation should return to original");
}
