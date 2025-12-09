//! Swap characters tests
//!
//! Tests swapping the pixel data between two characters.

use icy_engine_edit::bitfont::{BitFontEditState, BitFontUndoState};

#[test]
fn test_swap_chars_basic() {
    let mut state = BitFontEditState::new();

    // Clear glyphs first (VGA font has pixels set)
    state.clear_glyph('A').unwrap();
    state.clear_glyph('B').unwrap();

    // Set different patterns in A and B
    state.set_pixel('A', 1, 1, true).unwrap();
    state.set_pixel('B', 5, 5, true).unwrap();

    state.swap_chars('A', 'B').unwrap();

    // Patterns should be swapped
    assert!(!state.get_glyph_pixels('A')[1][1], "A should not have its original pixel");
    assert!(state.get_glyph_pixels('A')[5][5], "A should have B's pixel");

    assert!(state.get_glyph_pixels('B')[1][1], "B should have A's pixel");
    assert!(!state.get_glyph_pixels('B')[5][5], "B should not have its original pixel");
}

#[test]
fn test_swap_chars_with_empty() {
    let mut state = BitFontEditState::new();

    // Clear glyphs first (VGA font has pixels set)
    state.clear_glyph('A').unwrap();
    state.clear_glyph('B').unwrap();

    // Only set pattern in A, B is empty
    state.set_pixel('A', 2, 2, true).unwrap();
    state.set_pixel('A', 3, 3, true).unwrap();

    state.swap_chars('A', 'B').unwrap();

    // A should now be empty
    assert!(!state.get_glyph_pixels('A')[2][2]);
    assert!(!state.get_glyph_pixels('A')[3][3]);

    // B should have A's pattern
    assert!(state.get_glyph_pixels('B')[2][2]);
    assert!(state.get_glyph_pixels('B')[3][3]);
}

#[test]
fn test_swap_chars_same_char() {
    let mut state = BitFontEditState::new();

    state.set_pixel('A', 2, 2, true).unwrap();
    let original = state.get_glyph_pixels('A').clone();

    // Swapping with self should be no-op
    state.swap_chars('A', 'A').unwrap();

    assert_eq!(state.get_glyph_pixels('A'), &original);
}

#[test]
fn test_swap_chars_twice_returns_original() {
    let mut state = BitFontEditState::new();

    state.set_pixel('A', 1, 1, true).unwrap();
    state.set_pixel('B', 5, 5, true).unwrap();

    let original_a = state.get_glyph_pixels('A').clone();
    let original_b = state.get_glyph_pixels('B').clone();

    state.swap_chars('A', 'B').unwrap();
    state.swap_chars('A', 'B').unwrap();

    assert_eq!(state.get_glyph_pixels('A'), &original_a);
    assert_eq!(state.get_glyph_pixels('B'), &original_b);
}

#[test]
fn test_swap_chars_undo() {
    let mut state = BitFontEditState::new();

    state.set_pixel('A', 1, 1, true).unwrap();
    state.set_pixel('B', 5, 5, true).unwrap();

    let original_a = state.get_glyph_pixels('A').clone();
    let original_b = state.get_glyph_pixels('B').clone();

    state.swap_chars('A', 'B').unwrap();

    // Verify swap happened
    assert!(state.get_glyph_pixels('A')[5][5]);
    assert!(state.get_glyph_pixels('B')[1][1]);

    state.undo().unwrap();

    assert_eq!(state.get_glyph_pixels('A'), &original_a);
    assert_eq!(state.get_glyph_pixels('B'), &original_b);
}

#[test]
fn test_swap_chars_reversed_order() {
    let mut state = BitFontEditState::new();

    state.set_pixel('A', 1, 1, true).unwrap();
    state.set_pixel('B', 5, 5, true).unwrap();

    // Swap B with A (reversed order) should have same effect
    state.swap_chars('B', 'A').unwrap();

    assert!(state.get_glyph_pixels('A')[5][5]);
    assert!(state.get_glyph_pixels('B')[1][1]);
}

#[test]
fn test_swap_chars_special_characters() {
    let mut state = BitFontEditState::new();

    // Test with special characters (control codes)
    let null_char = char::from_u32(0x00).unwrap();
    let high_char = char::from_u32(0xFF).unwrap();

    state.set_pixel(null_char, 0, 0, true).unwrap();
    state.set_pixel(high_char, 7, 15, true).unwrap();

    state.swap_chars(null_char, high_char).unwrap();

    assert!(state.get_glyph_pixels(null_char)[15][7]);
    assert!(state.get_glyph_pixels(high_char)[0][0]);
}

#[test]
fn test_swap_chars_marks_dirty() {
    let mut state = BitFontEditState::new();
    state.set_pixel('A', 1, 1, true).unwrap();
    state.mark_clean();

    assert!(!state.is_dirty());

    state.swap_chars('A', 'B').unwrap();

    assert!(state.is_dirty());
}
