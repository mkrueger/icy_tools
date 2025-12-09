//! Insert and delete line tests
//!
//! Tests inserting and deleting horizontal lines across all glyphs.

use icy_engine_edit::bitfont::{BitFontEditState, BitFontUndoState};

#[test]
fn test_insert_line() {
    let mut state = BitFontEditState::new();

    // Set pixel at row 5
    state.set_pixel('A', 3, 5, true).unwrap();

    // Position cursor at row 5
    state.set_cursor_pos(0, 5);

    state.insert_line().unwrap();

    // Pixel should have moved down
    assert!(!state.get_glyph_pixels('A')[5][3], "original row should be empty");
    assert!(state.get_glyph_pixels('A')[6][3], "pixel should have moved down");
}

#[test]
fn test_insert_line_at_top() {
    let mut state = BitFontEditState::new();

    // Set pixel at row 0
    state.set_pixel('A', 3, 0, true).unwrap();

    state.set_cursor_pos(0, 0);
    state.insert_line().unwrap();

    // Pixel should have moved down
    assert!(!state.get_glyph_pixels('A')[0][3]);
    assert!(state.get_glyph_pixels('A')[1][3]);
}

#[test]
fn test_insert_line_pushes_bottom_off() {
    let mut state = BitFontEditState::new();
    let (_, height) = state.font_size();

    // Set pixel at bottom row
    state.set_pixel('A', 3, height - 1, true).unwrap();

    state.set_cursor_pos(0, 0);
    state.insert_line().unwrap();

    // Bottom pixel should be gone (pushed off)
    // Check that pixel moved from 15 to... it's gone
    let glyph = state.get_glyph_pixels('A');
    assert!(!glyph[(height - 1) as usize][3], "bottom row should be empty after push");
}

#[test]
fn test_insert_line_affects_all_glyphs() {
    let mut state = BitFontEditState::new();

    // Set pixels in multiple glyphs at row 5
    state.set_pixel('A', 3, 5, true).unwrap();
    state.set_pixel('B', 3, 5, true).unwrap();
    state.set_pixel('Z', 3, 5, true).unwrap();

    state.set_cursor_pos(0, 5);
    state.insert_line().unwrap();

    // All glyphs should be affected
    assert!(state.get_glyph_pixels('A')[6][3]);
    assert!(state.get_glyph_pixels('B')[6][3]);
    assert!(state.get_glyph_pixels('Z')[6][3]);
}

#[test]
fn test_insert_line_undo() {
    let mut state = BitFontEditState::new();

    state.set_pixel('A', 3, 5, true).unwrap();
    state.set_cursor_pos(0, 5);

    state.insert_line().unwrap();
    assert!(state.get_glyph_pixels('A')[6][3]);

    state.undo().unwrap();
    assert!(state.get_glyph_pixels('A')[5][3]);
}

#[test]
fn test_delete_line() {
    let mut state = BitFontEditState::new();

    // Set pixels at rows 5 and 6
    state.set_pixel('A', 3, 5, true).unwrap();
    state.set_pixel('A', 3, 6, true).unwrap();

    state.set_cursor_pos(0, 5);
    state.delete_line().unwrap();

    // Row 5 should now contain what was in row 6
    assert!(state.get_glyph_pixels('A')[5][3], "row 6 content moved up to row 5");
}

#[test]
fn test_delete_line_at_bottom() {
    let mut state = BitFontEditState::new();
    let (_, height) = state.font_size();

    // Clear the glyph first (VGA font has pixels set)
    state.clear_glyph('A').unwrap();

    // Set pixel at bottom row
    state.set_pixel('A', 3, height - 1, true).unwrap();
    // Also set a pixel at second-to-last row for verification
    state.set_pixel('A', 3, height - 2, true).unwrap();

    state.set_cursor_pos(0, height - 1);
    state.delete_line().unwrap();

    // After deleting bottom row, font height should be reduced
    let (_, new_height) = state.font_size();
    assert_eq!(new_height, height - 1, "height should be reduced by 1");

    // The pixel that was at row height-2 should still be there (now at the new bottom)
    let glyph = state.get_glyph_pixels('A');
    assert!(glyph[(new_height - 1) as usize][3], "second-to-last row pixel should remain");
}

#[test]
fn test_delete_line_undo() {
    let mut state = BitFontEditState::new();

    state.set_pixel('A', 3, 5, true).unwrap();
    let original = state.get_glyph_pixels('A').clone();

    state.set_cursor_pos(0, 5);
    state.delete_line().unwrap();

    state.undo().unwrap();

    assert_eq!(state.get_glyph_pixels('A'), &original);
}

#[test]
fn test_delete_line_affects_all_glyphs() {
    let mut state = BitFontEditState::new();

    state.set_pixel('A', 3, 6, true).unwrap();
    state.set_pixel('B', 3, 6, true).unwrap();

    state.set_cursor_pos(0, 5);
    state.delete_line().unwrap();

    // Row 6 content should have moved to row 5 in all glyphs
    assert!(state.get_glyph_pixels('A')[5][3]);
    assert!(state.get_glyph_pixels('B')[5][3]);
}
