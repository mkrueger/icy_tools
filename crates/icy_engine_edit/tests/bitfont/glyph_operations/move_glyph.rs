//! Move glyph tests
//!
//! Tests moving glyph pixels with wrapping behavior.
//! Move operations wrap pixels around the glyph boundaries.

use icy_engine_edit::bitfont::{BitFontEditState, BitFontUndoState};

/// Helper to create a state with a test pattern at a specific character
fn create_state_with_pattern(ch: char, pattern: Vec<Vec<bool>>) -> BitFontEditState {
    let mut state = BitFontEditState::new();
    state.set_glyph_pixels(ch, pattern).unwrap();
    state
}

/// Create a single pixel pattern at given position
fn single_pixel_pattern(width: usize, height: usize, x: usize, y: usize) -> Vec<Vec<bool>> {
    let mut pattern = vec![vec![false; width]; height];
    if y < height && x < width {
        pattern[y][x] = true;
    }
    pattern
}

#[test]
fn test_move_right_basic() {
    // Single pixel at (0, 0), move right 1
    let pattern = single_pixel_pattern(8, 16, 0, 0);
    let mut state = create_state_with_pattern('A', pattern);

    state.move_glyph('A', 1, 0).unwrap();

    let result = state.get_glyph_pixels('A');
    assert!(!result[0][0], "original position should be empty");
    assert!(result[0][1], "pixel should have moved right");
}

#[test]
fn test_move_left_basic() {
    let pattern = single_pixel_pattern(8, 16, 3, 0);
    let mut state = create_state_with_pattern('A', pattern);

    state.move_glyph('A', -1, 0).unwrap();

    let result = state.get_glyph_pixels('A');
    assert!(result[0][2], "pixel should have moved left");
    assert!(!result[0][3], "original position should be empty");
}

#[test]
fn test_move_down_basic() {
    let pattern = single_pixel_pattern(8, 16, 0, 0);
    let mut state = create_state_with_pattern('A', pattern);

    state.move_glyph('A', 0, 1).unwrap();

    let result = state.get_glyph_pixels('A');
    assert!(!result[0][0], "original position should be empty");
    assert!(result[1][0], "pixel should have moved down");
}

#[test]
fn test_move_up_basic() {
    let pattern = single_pixel_pattern(8, 16, 0, 5);
    let mut state = create_state_with_pattern('A', pattern);

    state.move_glyph('A', 0, -1).unwrap();

    let result = state.get_glyph_pixels('A');
    assert!(result[4][0], "pixel should have moved up");
    assert!(!result[5][0], "original position should be empty");
}

#[test]
fn test_move_right_wraps_at_boundary() {
    // Pixel at rightmost column (7), move right should wrap to column 0
    let pattern = single_pixel_pattern(8, 16, 7, 5);
    let mut state = create_state_with_pattern('A', pattern);

    state.move_glyph('A', 1, 0).unwrap();

    let result = state.get_glyph_pixels('A');
    assert!(!result[5][7], "original position should be empty");
    assert!(result[5][0], "pixel should have wrapped to left edge");
}

#[test]
fn test_move_left_wraps_at_boundary() {
    // Pixel at leftmost column (0), move left should wrap to column 7
    let pattern = single_pixel_pattern(8, 16, 0, 5);
    let mut state = create_state_with_pattern('A', pattern);

    state.move_glyph('A', -1, 0).unwrap();

    let result = state.get_glyph_pixels('A');
    assert!(!result[5][0], "original position should be empty");
    assert!(result[5][7], "pixel should have wrapped to right edge");
}

#[test]
fn test_move_down_wraps_at_boundary() {
    // Pixel at bottom row (15), move down should wrap to row 0
    let pattern = single_pixel_pattern(8, 16, 3, 15);
    let mut state = create_state_with_pattern('A', pattern);

    state.move_glyph('A', 0, 1).unwrap();

    let result = state.get_glyph_pixels('A');
    assert!(!result[15][3], "original position should be empty");
    assert!(result[0][3], "pixel should have wrapped to top edge");
}

#[test]
fn test_move_up_wraps_at_boundary() {
    // Pixel at top row (0), move up should wrap to row 15
    let pattern = single_pixel_pattern(8, 16, 3, 0);
    let mut state = create_state_with_pattern('A', pattern);

    state.move_glyph('A', 0, -1).unwrap();

    let result = state.get_glyph_pixels('A');
    assert!(!result[0][3], "original position should be empty");
    assert!(result[15][3], "pixel should have wrapped to bottom edge");
}

#[test]
fn test_move_diagonal_with_wrap() {
    // Pixel at corner (7, 15), move (+1, +1) should wrap to (0, 0)
    let pattern = single_pixel_pattern(8, 16, 7, 15);
    let mut state = create_state_with_pattern('A', pattern);

    state.move_glyph('A', 1, 1).unwrap();

    let result = state.get_glyph_pixels('A');
    assert!(!result[15][7], "original position should be empty");
    assert!(result[0][0], "pixel should have wrapped to opposite corner");
}

#[test]
fn test_move_preserves_pixel_count() {
    // Create a pattern with multiple pixels
    let mut pattern = vec![vec![false; 8]; 16];
    pattern[0][0] = true;
    pattern[5][3] = true;
    pattern[10][7] = true;
    let pixel_count = 3;

    let mut state = create_state_with_pattern('A', pattern);

    // Move in various directions
    state.move_glyph('A', 2, 3).unwrap();

    let result = state.get_glyph_pixels('A');
    let result_count: usize = result.iter().flat_map(|row| row.iter()).filter(|&&p| p).count();
    assert_eq!(result_count, pixel_count, "pixel count should be preserved after move");
}

#[test]
fn test_move_undo() {
    let pattern = single_pixel_pattern(8, 16, 3, 5);
    let mut state = create_state_with_pattern('A', pattern.clone());

    state.move_glyph('A', 2, 3).unwrap();

    // Verify move happened
    assert!(!state.get_glyph_pixels('A')[5][3]);

    // Undo
    state.undo().unwrap();

    // Should be back to original
    assert!(state.get_glyph_pixels('A')[5][3], "pixel should be restored after undo");
}
