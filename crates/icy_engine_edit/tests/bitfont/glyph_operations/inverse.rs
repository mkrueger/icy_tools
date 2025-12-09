//! Inverse glyph tests
//!
//! Tests inverting pixel values (toggling all pixels).

use icy_engine_edit::bitfont::{BitFontEditState, BitFontUndoState};

/// Helper to create a state with a test pattern
fn create_state_with_pattern(ch: char, pattern: Vec<Vec<bool>>) -> BitFontEditState {
    let mut state = BitFontEditState::new();
    state.set_glyph_pixels(ch, pattern).unwrap();
    state
}

#[test]
fn test_inverse_empty_glyph() {
    // Empty glyph should become all filled
    let pattern = vec![vec![false; 8]; 16];
    let mut state = create_state_with_pattern('A', pattern);

    state.inverse_glyph('A').unwrap();

    let result = state.get_glyph_pixels('A');
    for row in result {
        for pixel in row {
            assert!(*pixel, "all pixels should be set after inverting empty glyph");
        }
    }
}

#[test]
fn test_inverse_filled_glyph() {
    // Filled glyph should become empty
    let pattern = vec![vec![true; 8]; 16];
    let mut state = create_state_with_pattern('A', pattern);

    state.inverse_glyph('A').unwrap();

    let result = state.get_glyph_pixels('A');
    for row in result {
        for pixel in row {
            assert!(!*pixel, "all pixels should be cleared after inverting filled glyph");
        }
    }
}

#[test]
fn test_inverse_pattern() {
    let mut pattern = vec![vec![false; 8]; 16];
    pattern[0][0] = true;
    pattern[5][3] = true;
    pattern[10][7] = true;

    let mut state = create_state_with_pattern('A', pattern);

    state.inverse_glyph('A').unwrap();

    let result = state.get_glyph_pixels('A');
    // Original set pixels should now be clear
    assert!(!result[0][0]);
    assert!(!result[5][3]);
    assert!(!result[10][7]);
    // Some originally clear pixels should now be set
    assert!(result[0][1]);
    assert!(result[1][0]);
}

#[test]
fn test_inverse_twice_returns_original() {
    let mut pattern = vec![vec![false; 8]; 16];
    pattern[2][3] = true;
    pattern[7][5] = true;
    pattern[12][1] = true;
    let original = pattern.clone();

    let mut state = create_state_with_pattern('A', pattern);

    state.inverse_glyph('A').unwrap();
    state.inverse_glyph('A').unwrap();

    let result = state.get_glyph_pixels('A');
    assert_eq!(result, &original, "double inverse should return to original");
}

#[test]
fn test_inverse_undo() {
    let mut pattern = vec![vec![false; 8]; 16];
    pattern[5][5] = true;
    let original = pattern.clone();

    let mut state = create_state_with_pattern('A', pattern);

    state.inverse_glyph('A').unwrap();

    // Verify inverse happened
    assert!(!state.get_glyph_pixels('A')[5][5]);
    assert!(state.get_glyph_pixels('A')[0][0]);

    state.undo().unwrap();

    assert_eq!(state.get_glyph_pixels('A'), &original);
}

#[test]
fn test_inverse_selection_partial() {
    // Create a glyph with some pixels set
    let mut pattern = vec![vec![false; 8]; 16];
    pattern[2][2] = true;
    pattern[3][3] = true;

    let mut state = create_state_with_pattern('A', pattern);
    state.set_selected_char('A');

    // Select just a portion (4,4) to (6,6) that doesn't include the set pixels
    state.set_selection(Some((4, 4, 6, 6)));

    state.inverse_edit_selection().unwrap();

    let result = state.get_glyph_pixels('A');
    // Original pixels outside selection should be unchanged
    assert!(result[2][2], "pixel outside selection unchanged");
    assert!(result[3][3], "pixel outside selection unchanged");
    // Pixels inside selection should now be set (were clear)
    assert!(result[4][4], "pixel inside selection inverted");
    assert!(result[5][5], "pixel inside selection inverted");
}

#[test]
fn test_inverse_selection_undo() {
    let pattern = vec![vec![false; 8]; 16];
    let original = pattern.clone();

    let mut state = create_state_with_pattern('A', pattern);
    state.set_selected_char('A');
    state.set_selection(Some((2, 2, 4, 4)));

    state.inverse_edit_selection().unwrap();

    // Verify some pixels are now set
    assert!(state.get_glyph_pixels('A')[2][2]);

    state.undo().unwrap();

    assert_eq!(state.get_glyph_pixels('A'), &original);
}
