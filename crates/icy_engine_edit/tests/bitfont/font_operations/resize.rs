//! Font resize tests
//!
//! Tests changing font dimensions.

use icy_engine_edit::bitfont::{BitFontEditState, BitFontUndoState};

#[test]
fn test_initial_font_size() {
    let state = BitFontEditState::new();
    // Default is 8x16 VGA font
    assert_eq!(state.font_size(), (8, 16));
}

#[test]
fn test_resize_font_larger() {
    let mut state = BitFontEditState::new();

    state.resize_font(8, 20).unwrap();

    assert_eq!(state.font_size(), (8, 20));

    // Check that glyph data was resized
    let glyph = state.get_glyph_pixels('A');
    assert_eq!(glyph.len(), 20);
    assert_eq!(glyph[0].len(), 8);
}

#[test]
fn test_resize_font_smaller() {
    let mut state = BitFontEditState::new();

    state.resize_font(6, 12).unwrap();

    assert_eq!(state.font_size(), (6, 12));

    let glyph = state.get_glyph_pixels('A');
    assert_eq!(glyph.len(), 12);
    assert_eq!(glyph[0].len(), 6);
}

#[test]
fn test_resize_preserves_existing_pixels() {
    let mut state = BitFontEditState::new();

    // Set a pixel
    state.set_pixel('A', 2, 3, true).unwrap();

    // Resize larger
    state.resize_font(8, 20).unwrap();

    // Pixel should still be there
    assert!(state.get_glyph_pixels('A')[3][2]);
}

#[test]
fn test_resize_smaller_clips_pixels() {
    let mut state = BitFontEditState::new();

    // Set a pixel at the edge
    state.set_pixel('A', 7, 15, true).unwrap();

    // Resize smaller - pixel is now outside bounds
    state.resize_font(6, 12).unwrap();

    // Check new dimensions
    assert_eq!(state.font_size(), (6, 12));

    // The original pixel position no longer exists
    let glyph = state.get_glyph_pixels('A');
    assert_eq!(glyph.len(), 12);
    assert_eq!(glyph[0].len(), 6);
}

#[test]
fn test_resize_undo() {
    let mut state = BitFontEditState::new();

    assert_eq!(state.font_size(), (8, 16));

    state.resize_font(6, 12).unwrap();
    assert_eq!(state.font_size(), (6, 12));

    state.undo().unwrap();
    assert_eq!(state.font_size(), (8, 16));
}

#[test]
fn test_resize_width_only() {
    let mut state = BitFontEditState::new();

    state.resize_font(4, 16).unwrap();

    assert_eq!(state.font_size(), (4, 16));
}

#[test]
fn test_resize_height_only() {
    let mut state = BitFontEditState::new();

    state.resize_font(8, 32).unwrap();

    assert_eq!(state.font_size(), (8, 32));
}

#[test]
fn test_resize_all_glyphs_affected() {
    let mut state = BitFontEditState::new();

    // Set pixels in multiple glyphs
    state.set_pixel('A', 2, 2, true).unwrap();
    state.set_pixel('Z', 2, 2, true).unwrap();

    state.resize_font(8, 20).unwrap();

    // All glyphs should have new dimensions
    assert_eq!(state.get_glyph_pixels('A').len(), 20);
    assert_eq!(state.get_glyph_pixels('Z').len(), 20);
    assert_eq!(state.get_glyph_pixels(char::from_u32(0x00).unwrap()).len(), 20);
    assert_eq!(state.get_glyph_pixels(char::from_u32(0xFF).unwrap()).len(), 20);
}

#[test]
fn test_resize_marks_dirty() {
    let mut state = BitFontEditState::new();
    state.mark_clean();

    assert!(!state.is_dirty());

    state.resize_font(8, 20).unwrap();

    assert!(state.is_dirty());
}
