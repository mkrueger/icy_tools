//! Linear charset selection tests
//!
//! Tests linear selection mode in the charset grid.
//! Linear mode selects characters in reading order (left to right, top to bottom).

use icy_engine::Position;
use icy_engine_edit::bitfont::{BitFontEditState, BitFontFocusedPanel};

#[test]
fn test_no_charset_selection_initially() {
    let state = BitFontEditState::new();
    assert!(state.charset_selection().is_none());
}

#[test]
fn test_start_charset_selection_linear() {
    let mut state = BitFontEditState::new();
    state.set_charset_cursor(3, 2);
    state.start_charset_selection(); // Default is linear mode

    let sel = state.charset_selection().expect("should have selection");
    let (anchor, lead, is_rect) = sel;
    assert_eq!(anchor, Position::new(3, 2));
    assert_eq!(lead, Position::new(3, 2));
    assert!(!is_rect, "should be linear mode");
}

#[test]
fn test_extend_charset_selection_linear() {
    let mut state = BitFontEditState::new();
    state.set_charset_cursor(1, 1);
    state.start_charset_selection();

    state.set_charset_cursor(5, 3);
    state.extend_charset_selection(); // Default is linear mode

    let sel = state.charset_selection().expect("should have selection");
    let (anchor, lead, is_rect) = sel;
    assert_eq!(anchor, Position::new(1, 1));
    assert_eq!(lead, Position::new(5, 3));
    assert!(!is_rect);
}

#[test]
fn test_clear_charset_selection() {
    let mut state = BitFontEditState::new();
    state.start_charset_selection();
    state.set_charset_cursor(5, 5);
    state.extend_charset_selection();

    assert!(state.charset_selection().is_some());

    state.clear_charset_selection();
    assert!(state.charset_selection().is_none());
}

#[test]
fn test_charset_selection_backwards() {
    let mut state = BitFontEditState::new();

    // Start at (10, 10), extend to (2, 2)
    state.set_charset_cursor(10, 10);
    state.start_charset_selection();
    state.set_charset_cursor(2, 2);
    state.extend_charset_selection();

    let sel = state.charset_selection().expect("should have selection");
    let (anchor, lead, _) = sel;
    assert_eq!(anchor, Position::new(10, 10));
    assert_eq!(lead, Position::new(2, 2));
}

#[test]
fn test_get_target_chars_linear_single() {
    let state = BitFontEditState::new();

    // No selection - should return just the selected char
    let chars = state.get_target_chars();
    assert_eq!(chars.len(), 1);
    assert_eq!(chars[0], state.selected_char());
}

#[test]
fn test_get_target_chars_linear_range() {
    let mut state = BitFontEditState::new();
    state.set_focused_panel(BitFontFocusedPanel::CharSet);

    // Select from (0, 1) to (2, 1) in linear mode = chars 16, 17, 18
    state.set_charset_cursor(0, 1);
    state.start_charset_selection();
    state.set_charset_cursor(2, 1);
    state.extend_charset_selection();

    let chars = state.get_target_chars();
    assert_eq!(chars.len(), 3);
    assert_eq!(chars[0], '\x10'); // 16
    assert_eq!(chars[1], '\x11'); // 17
    assert_eq!(chars[2], '\x12'); // 18
}

#[test]
fn test_get_target_chars_linear_multirow() {
    let mut state = BitFontEditState::new();
    state.set_focused_panel(BitFontFocusedPanel::CharSet);

    // Select from (14, 0) to (1, 1) in linear mode
    // This should include chars 14, 15, 16, 17 (wrap around row)
    state.set_charset_cursor(14, 0);
    state.start_charset_selection();
    state.set_charset_cursor(1, 1);
    state.extend_charset_selection();

    let chars = state.get_target_chars();
    // Linear range from char 14 to char 17 = 4 chars
    assert_eq!(chars.len(), 4);
    assert_eq!(chars[0], '\x0E'); // 14
    assert_eq!(chars[1], '\x0F'); // 15
    assert_eq!(chars[2], '\x10'); // 16
    assert_eq!(chars[3], '\x11'); // 17
}
