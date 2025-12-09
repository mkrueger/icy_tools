//! Insert and delete column tests
//!
//! Tests inserting and deleting vertical columns across all glyphs.

use icy_engine_edit::bitfont::{BitFontEditState, BitFontUndoState};

#[test]
fn test_insert_column() {
    let mut state = BitFontEditState::new();

    // Set pixel at column 3
    state.set_pixel('A', 3, 5, true).unwrap();

    // Position cursor at column 3
    state.set_cursor_pos(3, 0);

    state.insert_column().unwrap();

    // Pixel should have moved right
    assert!(!state.get_glyph_pixels('A')[5][3], "original column should be empty");
    assert!(state.get_glyph_pixels('A')[5][4], "pixel should have moved right");
}

#[test]
fn test_insert_column_at_left() {
    let mut state = BitFontEditState::new();

    // Set pixel at column 0
    state.set_pixel('A', 0, 5, true).unwrap();

    state.set_cursor_pos(0, 0);
    state.insert_column().unwrap();

    // Pixel should have moved right
    assert!(!state.get_glyph_pixels('A')[5][0]);
    assert!(state.get_glyph_pixels('A')[5][1]);
}

#[test]
fn test_insert_column_pushes_right_off() {
    let mut state = BitFontEditState::new();
    let (width, _) = state.font_size();

    // Clear the glyph first (VGA font has pixels set)
    state.clear_glyph('A').unwrap();

    // Set pixel at rightmost column
    state.set_pixel('A', width - 1, 5, true).unwrap();

    state.set_cursor_pos(0, 0);
    state.insert_column().unwrap();

    // Rightmost pixel should be gone (pushed off)
    let glyph = state.get_glyph_pixels('A');
    assert!(!glyph[5][(width - 1) as usize], "rightmost column should be empty after push");
}

#[test]
fn test_insert_column_affects_all_glyphs() {
    let mut state = BitFontEditState::new();

    // Set pixels in multiple glyphs at column 3
    state.set_pixel('A', 3, 5, true).unwrap();
    state.set_pixel('B', 3, 5, true).unwrap();
    state.set_pixel('Z', 3, 5, true).unwrap();

    state.set_cursor_pos(3, 0);
    state.insert_column().unwrap();

    // All glyphs should be affected
    assert!(state.get_glyph_pixels('A')[5][4]);
    assert!(state.get_glyph_pixels('B')[5][4]);
    assert!(state.get_glyph_pixels('Z')[5][4]);
}

#[test]
fn test_insert_column_undo() {
    let mut state = BitFontEditState::new();

    state.set_pixel('A', 3, 5, true).unwrap();
    state.set_cursor_pos(3, 0);

    state.insert_column().unwrap();
    assert!(state.get_glyph_pixels('A')[5][4]);

    state.undo().unwrap();
    assert!(state.get_glyph_pixels('A')[5][3]);
}

#[test]
fn test_delete_column() {
    let mut state = BitFontEditState::new();

    // Set pixels at columns 3 and 4
    state.set_pixel('A', 3, 5, true).unwrap();
    state.set_pixel('A', 4, 5, true).unwrap();

    state.set_cursor_pos(3, 0);
    state.delete_column().unwrap();

    // Column 3 should now contain what was in column 4
    assert!(state.get_glyph_pixels('A')[5][3], "column 4 content moved left to column 3");
}

#[test]
fn test_delete_column_at_right() {
    let mut state = BitFontEditState::new();
    let (width, _) = state.font_size();

    // Clear the glyph first (VGA font has pixels set)
    state.clear_glyph('A').unwrap();

    // Set pixel at rightmost column
    state.set_pixel('A', width - 1, 5, true).unwrap();
    // Also set a pixel at second-to-last column for verification
    state.set_pixel('A', width - 2, 5, true).unwrap();

    state.set_cursor_pos(width - 1, 0);
    state.delete_column().unwrap();

    // After deleting rightmost column, font width should be reduced
    let (new_width, _) = state.font_size();
    assert_eq!(new_width, width - 1, "width should be reduced by 1");

    // The pixel that was at column 6 should still be there (now at the new rightmost)
    let glyph = state.get_glyph_pixels('A');
    assert!(glyph[5][(new_width - 1) as usize], "second-to-last column pixel should remain");
}

#[test]
fn test_delete_column_undo() {
    let mut state = BitFontEditState::new();

    state.set_pixel('A', 3, 5, true).unwrap();
    let original = state.get_glyph_pixels('A').clone();

    state.set_cursor_pos(3, 0);
    state.delete_column().unwrap();

    state.undo().unwrap();

    assert_eq!(state.get_glyph_pixels('A'), &original);
}

#[test]
fn test_delete_column_affects_all_glyphs() {
    let mut state = BitFontEditState::new();

    state.set_pixel('A', 4, 5, true).unwrap();
    state.set_pixel('B', 4, 5, true).unwrap();

    state.set_cursor_pos(3, 0);
    state.delete_column().unwrap();

    // Column 4 content should have moved to column 3 in all glyphs
    assert!(state.get_glyph_pixels('A')[5][3]);
    assert!(state.get_glyph_pixels('B')[5][3]);
}

#[test]
fn test_insert_column_affects_entire_column() {
    let mut state = BitFontEditState::new();

    // Set pixels in entire column 3
    let (_, height) = state.font_size();
    for row in 0..height {
        state.set_pixel('A', 3, row, true).unwrap();
    }

    state.set_cursor_pos(3, 0);
    state.insert_column().unwrap();

    // All pixels should have moved right
    for row in 0..height as usize {
        assert!(!state.get_glyph_pixels('A')[row][3], "column 3 row {} should be empty", row);
        assert!(state.get_glyph_pixels('A')[row][4], "column 4 row {} should have pixel", row);
    }
}
