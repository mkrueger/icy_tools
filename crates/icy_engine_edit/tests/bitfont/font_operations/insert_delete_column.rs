//! Insert and delete column tests
//!
//! Tests inserting and deleting vertical columns across all glyphs.

use icy_engine_edit::bitfont::{BitFontEditState, BitFontUndoState, MAX_FONT_WIDTH, MIN_FONT_WIDTH};

/// Test that delete_column does nothing when font is at minimum width (1)
#[test]
fn test_delete_column_minimum_width() {
    let mut state = BitFontEditState::new();

    // Resize to minimum width
    state.resize_font(MIN_FONT_WIDTH, 8).unwrap();
    assert_eq!(state.font_size().0, MIN_FONT_WIDTH);

    // Set a pixel to verify glyph content is preserved
    state.clear_glyph('A').unwrap();
    state.set_pixel('A', 0, 0, true).unwrap();

    // Try to delete column - should have no effect
    state.set_cursor_pos(0, 0);
    state.delete_column().unwrap();

    // Width should still be 1
    assert_eq!(state.font_size().0, MIN_FONT_WIDTH, "width should not go below MIN_FONT_WIDTH");

    // Pixel should still be there
    assert!(state.get_glyph_pixels('A')[0][0], "pixel should be preserved");
}

/// Test that insert_column does nothing when font is at maximum width (8)
#[test]
fn test_insert_column_maximum_width() {
    let mut state = BitFontEditState::new();

    // VGA font is already at width 8 (MAX_FONT_WIDTH)
    assert_eq!(state.font_size().0, MAX_FONT_WIDTH);

    // Set a pixel at rightmost column
    state.clear_glyph('A').unwrap();
    state.set_pixel('A', (MAX_FONT_WIDTH - 1) as i32, 0, true).unwrap();

    // Try to insert column - should have no effect
    state.set_cursor_pos(0, 0);
    state.insert_column().unwrap();

    // Width should still be 8
    assert_eq!(state.font_size().0, MAX_FONT_WIDTH, "width should not exceed MAX_FONT_WIDTH");

    // Pixel should still be at rightmost column (not pushed off)
    assert!(state.get_glyph_pixels('A')[0][(MAX_FONT_WIDTH - 1) as usize], "pixel should not be pushed off");
}

#[test]
fn test_insert_column() {
    let mut state = BitFontEditState::new();

    // Resize to width 7 so insert can work (MAX_FONT_WIDTH = 8)
    state.resize_font(7, 16).unwrap();
    state.clear_glyph('A').unwrap();

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

    // Resize to width 7 so insert can work (MAX_FONT_WIDTH = 8)
    state.resize_font(7, 16).unwrap();
    state.clear_glyph('A').unwrap();

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

    // Resize to width 7 so insert can work (MAX_FONT_WIDTH = 8)
    state.resize_font(7, 16).unwrap();
    let (width, _) = state.font_size();

    // Clear the glyph first (VGA font has pixels set)
    state.clear_glyph('A').unwrap();

    // Set pixel at rightmost column
    state.set_pixel('A', width - 1, 5, true).unwrap();

    state.set_cursor_pos(0, 0);
    state.insert_column().unwrap();

    // After insert, width is now 8, but original rightmost pixel should be pushed off
    let (new_width, _) = state.font_size();
    let glyph = state.get_glyph_pixels('A');
    // The pixel that was at column 6 (width-1) should now be at column 7 (new rightmost)
    assert!(glyph[5][(new_width - 1) as usize], "pixel should have moved to rightmost column");
}

#[test]
fn test_insert_column_affects_all_glyphs() {
    let mut state = BitFontEditState::new();

    // Resize to width 7 so insert can work (MAX_FONT_WIDTH = 8)
    state.resize_font(7, 16).unwrap();
    state.clear_glyph('A').unwrap();
    state.clear_glyph('B').unwrap();
    state.clear_glyph('Z').unwrap();

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

    // Resize to width 7 so insert can work (MAX_FONT_WIDTH = 8)
    state.resize_font(7, 16).unwrap();
    state.clear_glyph('A').unwrap();

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

    // Resize to width 7 so insert can work (MAX_FONT_WIDTH = 8)
    state.resize_font(7, 16).unwrap();
    state.clear_glyph('A').unwrap();

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
