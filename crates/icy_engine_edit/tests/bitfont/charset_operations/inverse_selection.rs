//! Inverse charset selection tests
//!
//! Tests inverting multiple glyphs at once via charset selection.

use icy_engine_edit::bitfont::{BitFontEditState, BitFontFocusedPanel};

#[test]
fn test_inverse_multiple_glyphs_linear() {
    let mut state = BitFontEditState::new();
    state.set_focused_panel(BitFontFocusedPanel::CharSet);

    // Set up initial patterns
    state.set_pixel('A', 0, 0, true).unwrap();
    state.set_pixel('B', 1, 1, true).unwrap();

    // Select A and B
    state.set_charset_cursor(1, 4); // 'A'
    state.start_charset_selection();
    state.set_charset_cursor(2, 4); // 'B'
    state.extend_charset_selection();

    let targets = state.get_target_chars();
    for ch in targets {
        state.inverse_glyph(ch).unwrap();
    }

    // Original set pixels should now be clear
    assert!(!state.get_glyph_pixels('A')[0][0]);
    assert!(!state.get_glyph_pixels('B')[1][1]);

    // Some originally clear pixels should now be set
    assert!(state.get_glyph_pixels('A')[0][1]);
    assert!(state.get_glyph_pixels('B')[0][0]);
}

#[test]
fn test_inverse_multiple_glyphs_rectangle() {
    let mut state = BitFontEditState::new();
    state.set_focused_panel(BitFontFocusedPanel::CharSet);

    // Clear the glyphs first so they start empty
    for &ch in &['\x00', '\x01', '\x10', '\x11'] {
        state.clear_glyph(ch).unwrap();
    }

    // Select 2x2 rectangle and inverse
    state.set_charset_cursor(0, 0);
    state.start_charset_selection_with_mode(true);
    state.set_charset_cursor(1, 1);
    state.extend_charset_selection_with_mode(true);

    let targets = state.get_target_chars();
    assert_eq!(targets.len(), 4);

    for ch in targets {
        state.inverse_glyph(ch).unwrap();
    }

    // All 4 glyphs should now be fully filled (were empty, now inverted)
    for &ch in &['\x00', '\x01', '\x10', '\x11'] {
        let all_set = state.get_glyph_pixels(ch).iter().all(|row| row.iter().all(|&p| p));
        assert!(all_set, "glyph {:?} should be fully set", ch as u8);
    }
}

#[test]
fn test_inverse_charset_selection_preserves_others() {
    let mut state = BitFontEditState::new();

    // Only select char 0
    state.set_charset_cursor(0, 0);
    state.set_selected_char('\x00');

    state.inverse_glyph('\x00').unwrap();

    // Char 0 should be filled
    assert!(state.get_glyph_pixels('\x00')[0][0]);

    // Char 1 should still be empty
    assert!(!state.get_glyph_pixels('\x01')[0][0]);
}
