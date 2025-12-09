//! Clear charset selection tests
//!
//! Tests clearing multiple glyphs at once via charset selection.

use icy_engine_edit::bitfont::{BitFontEditState, BitFontFocusedPanel};

/// Helper to create a state with filled glyphs, focused on CharSet panel
fn create_state_with_filled_glyphs(chars: &[char]) -> BitFontEditState {
    let mut state = BitFontEditState::new();
    state.set_focused_panel(BitFontFocusedPanel::CharSet);
    for &ch in chars {
        state.set_glyph_pixels(ch, vec![vec![true; 8]; 16]).unwrap();
    }
    state
}

#[test]
fn test_clear_single_glyph_via_charset() {
    let mut state = create_state_with_filled_glyphs(&['A']);

    // Position charset cursor at 'A' (char 65 = column 1, row 4)
    state.set_charset_cursor(1, 4);
    state.set_selected_char('A');

    // No charset selection - should clear just the selected char
    state.clear_glyph('A').unwrap();

    // 'A' should be empty
    for row in state.get_glyph_pixels('A') {
        for &pixel in row {
            assert!(!pixel);
        }
    }
}

#[test]
fn test_clear_multiple_glyphs_linear() {
    let mut state = create_state_with_filled_glyphs(&['A', 'B', 'C']);

    // Select A, B, C in linear mode (chars 65, 66, 67)
    // A is at (1, 4), B at (2, 4), C at (3, 4)
    state.set_charset_cursor(1, 4);
    state.start_charset_selection();
    state.set_charset_cursor(3, 4);
    state.extend_charset_selection();

    // Get target chars and clear each
    let targets = state.get_target_chars();
    assert_eq!(targets.len(), 3);

    for ch in targets {
        state.clear_glyph(ch).unwrap();
    }

    // All should be empty
    for &ch in &['A', 'B', 'C'] {
        for row in state.get_glyph_pixels(ch) {
            for &pixel in row {
                assert!(!pixel, "glyph {} should be cleared", ch);
            }
        }
    }
}

#[test]
fn test_clear_multiple_glyphs_rectangle() {
    let mut state = create_state_with_filled_glyphs(&['\x00', '\x01', '\x10', '\x11']);

    // Select 2x2 rectangle at top-left corner
    state.set_charset_cursor(0, 0);
    state.start_charset_selection_with_mode(true);
    state.set_charset_cursor(1, 1);
    state.extend_charset_selection_with_mode(true);

    let targets = state.get_target_chars();
    assert_eq!(targets.len(), 4);

    for ch in targets {
        state.clear_glyph(ch).unwrap();
    }

    // All 4 corner glyphs should be empty
    for &ch in &['\x00', '\x01', '\x10', '\x11'] {
        let all_empty = state.get_glyph_pixels(ch).iter().all(|row| row.iter().all(|&p| !p));
        assert!(all_empty, "glyph {:?} should be cleared", ch as u8);
    }
}

#[test]
fn test_clear_charset_selection_preserves_others() {
    let mut state = create_state_with_filled_glyphs(&['A', 'B', 'C', 'D']);

    // Only select A and B
    state.set_charset_cursor(1, 4);
    state.start_charset_selection();
    state.set_charset_cursor(2, 4);
    state.extend_charset_selection();

    let targets = state.get_target_chars();
    for ch in targets {
        state.clear_glyph(ch).unwrap();
    }

    // A and B should be empty
    assert!(state.get_glyph_pixels('A').iter().all(|row| row.iter().all(|&p| !p)));
    assert!(state.get_glyph_pixels('B').iter().all(|row| row.iter().all(|&p| !p)));

    // C and D should still be filled
    assert!(state.get_glyph_pixels('C')[0][0]);
    assert!(state.get_glyph_pixels('D')[0][0]);
}
