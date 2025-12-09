//! Clear glyph tests
//!
//! Tests clearing glyphs and selections.

use icy_engine_edit::bitfont::{BitFontEditState, BitFontUndoState};

/// Helper to create a state with a filled glyph
fn create_state_with_filled_glyph(ch: char) -> BitFontEditState {
    let pattern = vec![vec![true; 8]; 16];
    let mut state = BitFontEditState::new();
    state.set_glyph_pixels(ch, pattern).unwrap();
    state
}

/// Helper to create a state with a test pattern
fn create_state_with_pattern(ch: char, pattern: Vec<Vec<bool>>) -> BitFontEditState {
    let mut state = BitFontEditState::new();
    state.set_glyph_pixels(ch, pattern).unwrap();
    state
}

#[test]
fn test_clear_glyph() {
    let mut state = create_state_with_filled_glyph('A');

    // Verify glyph is filled
    assert!(state.get_glyph_pixels('A')[0][0]);

    state.clear_glyph('A').unwrap();

    let result = state.get_glyph_pixels('A');
    for row in result {
        for pixel in row {
            assert!(!*pixel, "all pixels should be cleared");
        }
    }
}

#[test]
fn test_clear_already_empty_glyph() {
    let pattern = vec![vec![false; 8]; 16];
    let mut state = create_state_with_pattern('A', pattern);

    // Should not panic on empty glyph
    state.clear_glyph('A').unwrap();

    let result = state.get_glyph_pixels('A');
    for row in result {
        for pixel in row {
            assert!(!*pixel);
        }
    }
}

#[test]
fn test_clear_glyph_undo() {
    let mut pattern = vec![vec![false; 8]; 16];
    pattern[5][3] = true;
    pattern[10][7] = true;
    let original = pattern.clone();

    let mut state = create_state_with_pattern('A', pattern);

    state.clear_glyph('A').unwrap();

    // Verify cleared
    assert!(!state.get_glyph_pixels('A')[5][3]);

    state.undo().unwrap();

    assert_eq!(state.get_glyph_pixels('A'), &original);
}

#[test]
fn test_erase_selection() {
    let mut state = create_state_with_filled_glyph('A');
    state.set_selected_char('A');

    // Select a portion (2,2) to (5,5)
    state.set_selection(Some((2, 2, 5, 5)));

    state.erase_selection().unwrap();

    let result = state.get_glyph_pixels('A');

    // Pixels outside selection should still be set
    assert!(result[0][0], "pixel outside selection unchanged");
    assert!(result[1][1], "pixel outside selection unchanged");
    assert!(result[7][7], "pixel outside selection unchanged");

    // Pixels inside selection should be cleared
    assert!(!result[2][2], "pixel inside selection cleared");
    assert!(!result[3][3], "pixel inside selection cleared");
    assert!(!result[5][5], "pixel inside selection cleared");
}

#[test]
fn test_erase_selection_no_selection_clears_all() {
    let mut state = create_state_with_filled_glyph('A');
    state.set_selected_char('A');

    // No selection - should clear entire glyph
    state.clear_selection();

    state.erase_selection().unwrap();

    let result = state.get_glyph_pixels('A');
    for row in result {
        for pixel in row {
            assert!(!*pixel, "all pixels should be cleared when no selection");
        }
    }
}

#[test]
fn test_erase_selection_undo() {
    let mut state = create_state_with_filled_glyph('A');
    state.set_selected_char('A');
    state.set_selection(Some((3, 3, 6, 6)));

    state.erase_selection().unwrap();

    // Verify some pixels cleared
    assert!(!state.get_glyph_pixels('A')[4][4]);

    state.undo().unwrap();

    // Should be restored
    assert!(state.get_glyph_pixels('A')[4][4]);
}

#[test]
fn test_clear_different_glyphs() {
    let mut state = create_state_with_filled_glyph('A');
    // Also fill 'B'
    state.set_glyph_pixels('B', vec![vec![true; 8]; 16]).unwrap();

    // Clear only 'A'
    state.clear_glyph('A').unwrap();

    // 'A' should be empty
    assert!(!state.get_glyph_pixels('A')[0][0]);

    // 'B' should still be filled
    assert!(state.get_glyph_pixels('B')[0][0]);
}
