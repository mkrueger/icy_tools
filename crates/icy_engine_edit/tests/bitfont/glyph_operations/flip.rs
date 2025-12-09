//! Flip glyph tests
//!
//! Tests horizontal and vertical flip operations.
//!
//! Flip operations work on the current pixel selection (edit_selection).
//! If no selection exists, the entire glyph is flipped.

use icy_engine_edit::bitfont::{BitFontEditState, BitFontUndoState};

/// Helper to create a state with a test pattern
fn create_state_with_pattern(ch: char, pattern: Vec<Vec<bool>>) -> BitFontEditState {
    let mut state = BitFontEditState::new();
    state.set_glyph_pixels(ch, pattern).unwrap();
    state
}

#[test]
fn test_flip_x_single_pixel() {
    // Pixel at (0, 5), flip X should move to (7, 5)
    let mut pattern = vec![vec![false; 8]; 16];
    pattern[5][0] = true;

    let mut state = create_state_with_pattern('A', pattern);

    state.flip_glyph_x('A').unwrap();

    let result = state.get_glyph_pixels('A');
    assert!(!result[5][0], "original position should be empty");
    assert!(result[5][7], "pixel should be at mirrored X position");
}

#[test]
fn test_flip_y_single_pixel() {
    // Pixel at (3, 0), flip Y should move to (3, 15)
    let mut pattern = vec![vec![false; 8]; 16];
    pattern[0][3] = true;

    let mut state = create_state_with_pattern('A', pattern);

    state.flip_glyph_y('A').unwrap();

    let result = state.get_glyph_pixels('A');
    assert!(!result[0][3], "original position should be empty");
    assert!(result[15][3], "pixel should be at mirrored Y position");
}

#[test]
fn test_flip_x_preserves_center() {
    // Pixel at center column should stay in place
    let mut pattern = vec![vec![false; 8]; 16];
    // For width 8, there's no true center column (even width)
    // Pixels at column 3 flip to column 4 and vice versa
    pattern[5][3] = true;
    pattern[5][4] = true;

    let mut state = create_state_with_pattern('A', pattern);

    state.flip_glyph_x('A').unwrap();

    let result = state.get_glyph_pixels('A');
    // Column 3 -> Column 4, Column 4 -> Column 3
    assert!(result[5][3], "column 4 flipped to column 3");
    assert!(result[5][4], "column 3 flipped to column 4");
}

#[test]
fn test_flip_x_asymmetric_pattern() {
    // Create L-shaped pattern
    let mut pattern = vec![vec![false; 8]; 16];
    pattern[0][0] = true;
    pattern[1][0] = true;
    pattern[2][0] = true;
    pattern[2][1] = true;
    pattern[2][2] = true;

    let mut state = create_state_with_pattern('A', pattern);

    state.flip_glyph_x('A').unwrap();

    let result = state.get_glyph_pixels('A');
    // L should now be mirrored
    assert!(result[0][7], "top of L flipped");
    assert!(result[1][7]);
    assert!(result[2][7]);
    assert!(result[2][6]);
    assert!(result[2][5]);
}

#[test]
fn test_flip_y_asymmetric_pattern() {
    // Create pattern at top
    let mut pattern = vec![vec![false; 8]; 16];
    pattern[0][3] = true;
    pattern[1][3] = true;
    pattern[2][3] = true;

    let mut state = create_state_with_pattern('A', pattern);

    state.flip_glyph_y('A').unwrap();

    let result = state.get_glyph_pixels('A');
    // Should now be at bottom
    assert!(result[15][3]);
    assert!(result[14][3]);
    assert!(result[13][3]);
    assert!(!result[0][3]);
}

#[test]
fn test_flip_x_twice_returns_original() {
    let mut pattern = vec![vec![false; 8]; 16];
    pattern[3][1] = true;
    pattern[5][6] = true;
    pattern[10][2] = true;
    let original = pattern.clone();

    let mut state = create_state_with_pattern('A', pattern);

    state.flip_glyph_x('A').unwrap();
    state.flip_glyph_x('A').unwrap();

    let result = state.get_glyph_pixels('A');
    assert_eq!(result, &original, "double flip X should return to original");
}

#[test]
fn test_flip_y_twice_returns_original() {
    let mut pattern = vec![vec![false; 8]; 16];
    pattern[3][1] = true;
    pattern[5][6] = true;
    pattern[10][2] = true;
    let original = pattern.clone();

    let mut state = create_state_with_pattern('A', pattern);

    state.flip_glyph_y('A').unwrap();
    state.flip_glyph_y('A').unwrap();

    let result = state.get_glyph_pixels('A');
    assert_eq!(result, &original, "double flip Y should return to original");
}

#[test]
fn test_flip_x_undo() {
    let mut pattern = vec![vec![false; 8]; 16];
    pattern[5][1] = true;
    let original = pattern.clone();

    let mut state = create_state_with_pattern('A', pattern);

    state.flip_glyph_x('A').unwrap();

    // Verify flip happened
    assert!(!state.get_glyph_pixels('A')[5][1]);
    assert!(state.get_glyph_pixels('A')[5][6]);

    state.undo().unwrap();

    assert_eq!(state.get_glyph_pixels('A'), &original, "undo should restore original");
}

#[test]
fn test_flip_y_undo() {
    let mut pattern = vec![vec![false; 8]; 16];
    pattern[2][3] = true;
    let original = pattern.clone();

    let mut state = create_state_with_pattern('A', pattern);

    state.flip_glyph_y('A').unwrap();

    // Verify flip happened
    assert!(!state.get_glyph_pixels('A')[2][3]);
    assert!(state.get_glyph_pixels('A')[13][3]);

    state.undo().unwrap();

    assert_eq!(state.get_glyph_pixels('A'), &original, "undo should restore original");
}

#[test]
fn test_flip_preserves_pixel_count() {
    let mut pattern = vec![vec![false; 8]; 16];
    let pixel_count = 15;
    for i in 0..pixel_count {
        pattern[i % 16][i % 8] = true;
    }

    let mut state = create_state_with_pattern('A', pattern);

    state.flip_glyph_x('A').unwrap();

    let result = state.get_glyph_pixels('A');
    let count: usize = result.iter().flat_map(|r| r.iter()).filter(|&&p| p).count();
    assert_eq!(count, pixel_count, "flip should preserve pixel count");
}

#[test]
fn test_flip_x_with_selection() {
    // Test that flip X only affects pixels within the selection
    let mut pattern = vec![vec![false; 8]; 16];
    // Put pixels at left side: columns 0, 1, 2
    pattern[5][0] = true;
    pattern[5][1] = true;
    pattern[5][2] = true;
    // Put a pixel outside selection area
    pattern[10][0] = true;

    let mut state = create_state_with_pattern('A', pattern);

    // Select only columns 0-3, rows 4-6 (a 4x3 region)
    state.set_selection(Some((0, 4, 3, 6)));

    state.flip_glyph_x('A').unwrap();

    let result = state.get_glyph_pixels('A');

    // Within selection (row 5, columns 0-3): pixels should be flipped horizontally
    // Original: columns 0,1,2 set -> Flipped within selection (width 4): columns 3,2,1
    assert!(!result[5][0], "column 0 in selection should now be empty");
    assert!(result[5][1], "column 1 should have pixel (from column 2)");
    assert!(result[5][2], "column 2 should have pixel (from column 1)");
    assert!(result[5][3], "column 3 should have pixel (from column 0)");

    // Outside selection should be unchanged
    assert!(result[10][0], "pixel outside selection should be unchanged");
}

#[test]
fn test_flip_y_with_selection() {
    // Test that flip Y only affects pixels within the selection
    let mut pattern = vec![vec![false; 8]; 16];
    // Put pixels at top of selection area
    pattern[2][3] = true;
    pattern[3][3] = true;
    pattern[4][3] = true;
    // Put a pixel outside selection area
    pattern[0][3] = true;

    let mut state = create_state_with_pattern('A', pattern);

    // Select rows 2-6, columns 2-4 (a 3x5 region)
    state.set_selection(Some((2, 2, 4, 6)));

    state.flip_glyph_y('A').unwrap();

    let result = state.get_glyph_pixels('A');

    // Within selection: pixels should be flipped vertically
    // Original rows 2,3,4 -> Flipped within selection (height 5, rows 2-6): rows 6,5,4
    assert!(!result[2][3], "row 2 in selection should now be empty");
    assert!(!result[3][3], "row 3 in selection should now be empty");
    assert!(result[4][3], "row 4 should have pixel (from row 4 - center)");
    assert!(result[5][3], "row 5 should have pixel (from row 3)");
    assert!(result[6][3], "row 6 should have pixel (from row 2)");

    // Outside selection should be unchanged
    assert!(result[0][3], "pixel outside selection should be unchanged");
}

#[test]
fn test_flip_x_with_selection_undo() {
    let mut pattern = vec![vec![false; 8]; 16];
    pattern[5][0] = true;
    pattern[5][1] = true;
    let original = pattern.clone();

    let mut state = create_state_with_pattern('A', pattern);
    state.set_selection(Some((0, 4, 3, 6)));

    state.flip_glyph_x('A').unwrap();

    // Verify flip happened
    assert!(!state.get_glyph_pixels('A')[5][0]);
    assert!(state.get_glyph_pixels('A')[5][3]);

    state.undo().unwrap();

    assert_eq!(state.get_glyph_pixels('A'), &original, "undo should restore original");
}

#[test]
fn test_flip_y_with_selection_undo() {
    let mut pattern = vec![vec![false; 8]; 16];
    pattern[2][3] = true;
    let original = pattern.clone();

    let mut state = create_state_with_pattern('A', pattern);
    state.set_selection(Some((2, 2, 4, 6)));

    state.flip_glyph_y('A').unwrap();

    // Verify flip happened
    assert!(!state.get_glyph_pixels('A')[2][3]);
    assert!(state.get_glyph_pixels('A')[6][3]);

    state.undo().unwrap();

    assert_eq!(state.get_glyph_pixels('A'), &original, "undo should restore original");
}

#[test]
fn test_flip_x_no_selection_flips_entire_glyph() {
    // Without selection, flip should affect entire glyph
    let mut pattern = vec![vec![false; 8]; 16];
    pattern[5][0] = true;

    let mut state = create_state_with_pattern('A', pattern);
    // No selection set

    state.flip_glyph_x('A').unwrap();

    let result = state.get_glyph_pixels('A');
    assert!(!result[5][0], "original position should be empty");
    assert!(result[5][7], "pixel should be at mirrored position");
}

#[test]
fn test_flip_y_no_selection_flips_entire_glyph() {
    // Without selection, flip should affect entire glyph
    let mut pattern = vec![vec![false; 8]; 16];
    pattern[0][3] = true;

    let mut state = create_state_with_pattern('A', pattern);
    // No selection set

    state.flip_glyph_y('A').unwrap();

    let result = state.get_glyph_pixels('A');
    assert!(!result[0][3], "original position should be empty");
    assert!(result[15][3], "pixel should be at mirrored position");
}
