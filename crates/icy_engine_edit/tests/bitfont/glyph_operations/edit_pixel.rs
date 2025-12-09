//! Edit pixel tests
//!
//! Tests setting, clearing, and toggling individual pixels.

use icy_engine_edit::bitfont::{BitFontEditState, BitFontUndoState};

#[test]
fn test_set_pixel_on() {
    let mut state = BitFontEditState::new();

    state.set_pixel('A', 3, 5, true).unwrap();

    assert!(state.get_glyph_pixels('A')[5][3]);
}

#[test]
fn test_set_pixel_off() {
    let mut state = BitFontEditState::new();

    // First set it on
    state.set_pixel('A', 3, 5, true).unwrap();
    assert!(state.get_glyph_pixels('A')[5][3]);

    // Then set it off
    state.set_pixel('A', 3, 5, false).unwrap();
    assert!(!state.get_glyph_pixels('A')[5][3]);
}

#[test]
fn test_toggle_pixel_from_off() {
    let mut state = BitFontEditState::new();

    // Initially off
    assert!(!state.get_glyph_pixels('A')[5][3]);

    state.toggle_pixel('A', 3, 5).unwrap();

    assert!(state.get_glyph_pixels('A')[5][3]);
}

#[test]
fn test_toggle_pixel_from_on() {
    let mut state = BitFontEditState::new();

    // Set it on first
    state.set_pixel('A', 3, 5, true).unwrap();

    state.toggle_pixel('A', 3, 5).unwrap();

    assert!(!state.get_glyph_pixels('A')[5][3]);
}

#[test]
fn test_toggle_pixel_twice_returns_original() {
    let mut state = BitFontEditState::new();

    let original = state.get_glyph_pixels('A')[5][3];

    state.toggle_pixel('A', 3, 5).unwrap();
    state.toggle_pixel('A', 3, 5).unwrap();

    assert_eq!(state.get_glyph_pixels('A')[5][3], original);
}

#[test]
fn test_set_pixel_undo() {
    let mut state = BitFontEditState::new();

    assert!(!state.get_glyph_pixels('A')[5][3]);

    state.set_pixel('A', 3, 5, true).unwrap();
    assert!(state.get_glyph_pixels('A')[5][3]);

    state.undo().unwrap();
    assert!(!state.get_glyph_pixels('A')[5][3]);
}

#[test]
fn test_toggle_pixel_undo() {
    let mut state = BitFontEditState::new();

    state.set_pixel('A', 3, 5, true).unwrap();
    state.toggle_pixel('A', 3, 5).unwrap();

    assert!(!state.get_glyph_pixels('A')[5][3]);

    state.undo().unwrap();

    assert!(state.get_glyph_pixels('A')[5][3]);
}

#[test]
fn test_set_pixel_at_boundaries() {
    let mut state = BitFontEditState::new();
    let (width, height) = state.font_size();

    // Top-left corner
    state.set_pixel('A', 0, 0, true).unwrap();
    assert!(state.get_glyph_pixels('A')[0][0]);

    // Top-right corner
    state.set_pixel('A', width - 1, 0, true).unwrap();
    assert!(state.get_glyph_pixels('A')[0][(width - 1) as usize]);

    // Bottom-left corner
    state.set_pixel('A', 0, height - 1, true).unwrap();
    assert!(state.get_glyph_pixels('A')[(height - 1) as usize][0]);

    // Bottom-right corner
    state.set_pixel('A', width - 1, height - 1, true).unwrap();
    assert!(state.get_glyph_pixels('A')[(height - 1) as usize][(width - 1) as usize]);
}

#[test]
fn test_set_pixel_different_glyphs() {
    let mut state = BitFontEditState::new();

    state.set_pixel('A', 1, 1, true).unwrap();
    state.set_pixel('B', 2, 2, true).unwrap();

    // Each glyph should have its own pixel set
    assert!(state.get_glyph_pixels('A')[1][1]);
    assert!(!state.get_glyph_pixels('A')[2][2]);

    assert!(!state.get_glyph_pixels('B')[1][1]);
    assert!(state.get_glyph_pixels('B')[2][2]);
}

#[test]
fn test_fill_selection() {
    let mut state = BitFontEditState::new();
    state.set_selected_char('A');

    // Clear the glyph first (VGA font has pixels set)
    state.clear_glyph('A').unwrap();

    // Select a region
    state.set_selection(Some((2, 2, 4, 4)));

    state.fill_selection().unwrap();

    let result = state.get_glyph_pixels('A');

    // Pixels inside selection should be filled
    assert!(result[2][2]);
    assert!(result[3][3]);
    assert!(result[4][4]);

    // Pixels outside selection should be empty
    assert!(!result[0][0]);
    assert!(!result[1][1]);
    assert!(!result[5][5]);
}

#[test]
fn test_fill_selection_undo() {
    let mut state = BitFontEditState::new();
    state.set_selected_char('A');

    // Clear the glyph first (VGA font has pixels set)
    state.clear_glyph('A').unwrap();

    state.set_selection(Some((2, 2, 4, 4)));

    state.fill_selection().unwrap();

    assert!(state.get_glyph_pixels('A')[3][3]);

    state.undo().unwrap();

    assert!(!state.get_glyph_pixels('A')[3][3]);
}
