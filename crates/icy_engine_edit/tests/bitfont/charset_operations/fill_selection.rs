//! Fill charset selection tests
//!
//! Tests filling multiple glyphs at once via charset selection.

use icy_engine_edit::bitfont::{BitFontEditState, BitFontFocusedPanel, BitFontUndoState};

#[test]
fn test_fill_single_glyph() {
    let mut state = BitFontEditState::new();
    state.set_selected_char('A');

    // No edit selection - should fill entire glyph
    state.fill_selection().unwrap();

    // All pixels should be set
    let all_set = state.get_glyph_pixels('A').iter().all(|row| row.iter().all(|&p| p));
    assert!(all_set, "entire glyph should be filled");
}

#[test]
fn test_fill_partial_selection() {
    let mut state = BitFontEditState::new();
    state.set_selected_char('A');

    // Clear the glyph first (default font has pixels set)
    state.clear_glyph('A').unwrap();

    // Select only a portion
    state.set_selection(Some((2, 2, 5, 5)));

    state.fill_selection().unwrap();

    // Pixels inside selection should be set
    assert!(state.get_glyph_pixels('A')[2][2]);
    assert!(state.get_glyph_pixels('A')[3][3]);
    assert!(state.get_glyph_pixels('A')[5][5]);

    // Pixels outside selection should still be empty
    assert!(!state.get_glyph_pixels('A')[0][0]);
    assert!(!state.get_glyph_pixels('A')[1][1]);
    assert!(!state.get_glyph_pixels('A')[6][6]);
}

#[test]
fn test_fill_multiple_glyphs_via_charset() {
    let mut state = BitFontEditState::new();
    state.set_focused_panel(BitFontFocusedPanel::CharSet);

    // Select A and B in charset
    state.set_charset_cursor(1, 4);
    state.start_charset_selection();
    state.set_charset_cursor(2, 4);
    state.extend_charset_selection();

    // In CharSet mode, fill_selection fills all target glyphs
    state.fill_selection().unwrap();

    // Both should be filled
    assert!(state.get_glyph_pixels('A').iter().all(|row| row.iter().all(|&p| p)));
    assert!(state.get_glyph_pixels('B').iter().all(|row| row.iter().all(|&p| p)));
}

#[test]
fn test_fill_selection_undo() {
    let mut state = BitFontEditState::new();
    state.set_selected_char('A');

    // Clear glyph first so we have a known starting state
    state.clear_glyph('A').unwrap();

    // Verify it's empty
    assert!(!state.get_glyph_pixels('A')[3][3]);

    state.set_selection(Some((2, 2, 4, 4)));
    state.fill_selection().unwrap();

    // Now it should be filled
    assert!(state.get_glyph_pixels('A')[3][3]);

    state.undo().unwrap();

    // After undo of fill, should be back to cleared state
    // (Note: the clear_glyph is still in effect, we only undo the fill)
    assert!(!state.get_glyph_pixels('A')[3][3]);
}

#[test]
fn test_fill_preserves_other_glyphs() {
    let mut state = BitFontEditState::new();
    state.set_selected_char('A');

    state.fill_selection().unwrap();

    // A should be filled
    assert!(state.get_glyph_pixels('A')[0][0]);

    // B should still be empty
    assert!(!state.get_glyph_pixels('B')[0][0]);
}
