//! Charset cursor movement tests
//!
//! Tests cursor movement within the 16x16 character set grid.
//! The cursor wraps independently on X and Y axes.

use icy_engine_edit::bitfont::BitFontEditState;

#[test]
fn test_charset_cursor_initial_position() {
    let state = BitFontEditState::new();
    // Initial position is (0, 4) which is near 'A' (char 64 = '@')
    assert_eq!(state.charset_cursor(), (0, 4));
}

#[test]
fn test_charset_cursor_move_right() {
    let mut state = BitFontEditState::new();
    state.set_charset_cursor(0, 0); // Reset to known position
    state.move_charset_cursor(1, 0);
    assert_eq!(state.charset_cursor(), (1, 0));
}

#[test]
fn test_charset_cursor_move_down() {
    let mut state = BitFontEditState::new();
    state.set_charset_cursor(0, 0); // Reset to known position
    state.move_charset_cursor(0, 1);
    assert_eq!(state.charset_cursor(), (0, 1));
}

#[test]
fn test_charset_cursor_move_left_from_origin_wraps_to_last_column() {
    let mut state = BitFontEditState::new();
    state.set_charset_cursor(0, 0); // Start at origin
    state.move_charset_cursor(-1, 0);
    // Should wrap to column 15, stay on row 0 (independent wrap)
    assert_eq!(state.charset_cursor(), (15, 0));
}

#[test]
fn test_charset_cursor_move_up_from_origin_wraps_to_last_row() {
    let mut state = BitFontEditState::new();
    state.set_charset_cursor(0, 0); // Start at origin
    state.move_charset_cursor(0, -1);
    // Should wrap to row 15, stay on column 0 (independent wrap)
    assert_eq!(state.charset_cursor(), (0, 15));
}

#[test]
fn test_charset_cursor_move_right_from_last_column_wraps_to_first() {
    let mut state = BitFontEditState::new();
    state.set_charset_cursor(15, 5);
    state.move_charset_cursor(1, 0);
    // Should wrap to column 0, stay on row 5 (no row change)
    assert_eq!(state.charset_cursor(), (0, 5));
}

#[test]
fn test_charset_cursor_move_down_from_last_row_wraps_to_first() {
    let mut state = BitFontEditState::new();
    state.set_charset_cursor(5, 15);
    state.move_charset_cursor(0, 1);
    // Should wrap to row 0, stay on column 5
    assert_eq!(state.charset_cursor(), (5, 0));
}

#[test]
fn test_charset_cursor_diagonal_wrap() {
    let mut state = BitFontEditState::new();
    state.set_charset_cursor(15, 15);
    state.move_charset_cursor(1, 1);
    // Both should wrap independently
    assert_eq!(state.charset_cursor(), (0, 0));
}

#[test]
fn test_charset_cursor_negative_diagonal_wrap() {
    let mut state = BitFontEditState::new();
    state.set_charset_cursor(0, 0);
    state.move_charset_cursor(-1, -1);
    // Both should wrap independently to max values
    assert_eq!(state.charset_cursor(), (15, 15));
}

#[test]
fn test_charset_cursor_multiple_step_wrap() {
    let mut state = BitFontEditState::new();
    state.set_charset_cursor(14, 14);
    state.move_charset_cursor(3, 3);
    // 14 + 3 = 17, 17 % 16 = 1
    assert_eq!(state.charset_cursor(), (1, 1));
}

#[test]
fn test_charset_cursor_large_negative_move() {
    let mut state = BitFontEditState::new();
    state.set_charset_cursor(5, 5);
    state.move_charset_cursor(-20, -20);
    // 5 - 20 = -15, (-15).rem_euclid(16) = 1
    assert_eq!(state.charset_cursor(), (1, 1));
}

#[test]
fn test_charset_set_cursor_clamps() {
    let mut state = BitFontEditState::new();

    // set_charset_cursor clamps, doesn't wrap
    state.set_charset_cursor(20, 20);
    assert_eq!(state.charset_cursor(), (15, 15));

    state.set_charset_cursor(-5, -5);
    assert_eq!(state.charset_cursor(), (0, 0));
}

#[test]
fn test_charset_cursor_char_at_cursor() {
    let mut state = BitFontEditState::new();

    // Position (0, 0) = char 0
    state.set_charset_cursor(0, 0);
    assert_eq!(state.char_at_charset_cursor(), '\x00');

    // Position (1, 0) = char 1
    state.set_charset_cursor(1, 0);
    assert_eq!(state.char_at_charset_cursor(), '\x01');

    // Position (0, 1) = char 16
    state.set_charset_cursor(0, 1);
    assert_eq!(state.char_at_charset_cursor(), '\x10');

    // Position (15, 15) = char 255
    state.set_charset_cursor(15, 15);
    assert_eq!(state.char_at_charset_cursor(), '\u{FF}');
}
