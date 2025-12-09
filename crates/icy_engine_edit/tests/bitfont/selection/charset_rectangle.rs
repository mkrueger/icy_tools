//! Rectangle charset selection tests
//!
//! Tests rectangle selection mode in the charset grid.
//! Rectangle mode selects characters in a rectangular area (Alt+drag).

use icy_engine::Position;
use icy_engine_edit::bitfont::{BitFontEditState, BitFontFocusedPanel};

#[test]
fn test_start_charset_selection_rectangle() {
    let mut state = BitFontEditState::new();
    state.set_charset_cursor(3, 2);
    state.start_charset_selection_with_mode(true); // Rectangle mode

    let sel = state.charset_selection().expect("should have selection");
    let (anchor, lead, is_rect) = sel;
    assert_eq!(anchor, Position::new(3, 2));
    assert_eq!(lead, Position::new(3, 2));
    assert!(is_rect, "should be rectangle mode");
}

#[test]
fn test_extend_charset_selection_rectangle() {
    let mut state = BitFontEditState::new();
    state.set_charset_cursor(1, 1);
    state.start_charset_selection_with_mode(true);

    state.set_charset_cursor(5, 3);
    state.extend_charset_selection_with_mode(true);

    let sel = state.charset_selection().expect("should have selection");
    let (anchor, lead, is_rect) = sel;
    assert_eq!(anchor, Position::new(1, 1));
    assert_eq!(lead, Position::new(5, 3));
    assert!(is_rect);
}

#[test]
fn test_get_target_chars_rectangle_2x2() {
    let mut state = BitFontEditState::new();
    state.set_focused_panel(BitFontFocusedPanel::CharSet);

    // Select a 2x2 rectangle from (0, 0) to (1, 1)
    // Should include chars 0, 1, 16, 17
    state.set_charset_cursor(0, 0);
    state.start_charset_selection_with_mode(true);
    state.set_charset_cursor(1, 1);
    state.extend_charset_selection_with_mode(true);

    let chars = state.get_target_chars();
    assert_eq!(chars.len(), 4);

    // Rectangle should include: (0,0)=0, (1,0)=1, (0,1)=16, (1,1)=17
    assert!(chars.contains(&'\x00'));
    assert!(chars.contains(&'\x01'));
    assert!(chars.contains(&'\x10'));
    assert!(chars.contains(&'\x11'));
}

#[test]
fn test_get_target_chars_rectangle_3x2() {
    let mut state = BitFontEditState::new();
    state.set_focused_panel(BitFontFocusedPanel::CharSet);

    // Select a 3x2 rectangle from (2, 1) to (4, 2)
    state.set_charset_cursor(2, 1);
    state.start_charset_selection_with_mode(true);
    state.set_charset_cursor(4, 2);
    state.extend_charset_selection_with_mode(true);

    let chars = state.get_target_chars();
    // 3 columns x 2 rows = 6 chars
    assert_eq!(chars.len(), 6);

    // Row 1: chars 18, 19, 20
    // Row 2: chars 34, 35, 36
    assert!(chars.contains(&'\x12')); // 18
    assert!(chars.contains(&'\x13')); // 19
    assert!(chars.contains(&'\x14')); // 20
    assert!(chars.contains(&'\x22')); // 34
    assert!(chars.contains(&'\x23')); // 35
    assert!(chars.contains(&'\x24')); // 36
}

#[test]
fn test_get_target_chars_rectangle_backwards() {
    let mut state = BitFontEditState::new();
    state.set_focused_panel(BitFontFocusedPanel::CharSet);

    // Select from (3, 2) to (1, 0) - backwards rectangle
    state.set_charset_cursor(3, 2);
    state.start_charset_selection_with_mode(true);
    state.set_charset_cursor(1, 0);
    state.extend_charset_selection_with_mode(true);

    let chars = state.get_target_chars();
    // Should still be 3x3 = 9 chars
    assert_eq!(chars.len(), 9);
}

#[test]
fn test_rectangle_single_row() {
    let mut state = BitFontEditState::new();
    state.set_focused_panel(BitFontFocusedPanel::CharSet);

    // Select horizontal strip: (5, 3) to (8, 3)
    state.set_charset_cursor(5, 3);
    state.start_charset_selection_with_mode(true);
    state.set_charset_cursor(8, 3);
    state.extend_charset_selection_with_mode(true);

    let chars = state.get_target_chars();
    // 4 chars in a row
    assert_eq!(chars.len(), 4);

    // Row 3: chars 53, 54, 55, 56
    assert!(chars.contains(&(53 as char)));
    assert!(chars.contains(&(54 as char)));
    assert!(chars.contains(&(55 as char)));
    assert!(chars.contains(&(56 as char)));
}

#[test]
fn test_rectangle_single_column() {
    let mut state = BitFontEditState::new();
    state.set_focused_panel(BitFontFocusedPanel::CharSet);

    // Select vertical strip: (5, 1) to (5, 4)
    state.set_charset_cursor(5, 1);
    state.start_charset_selection_with_mode(true);
    state.set_charset_cursor(5, 4);
    state.extend_charset_selection_with_mode(true);

    let chars = state.get_target_chars();
    // 4 chars in a column
    assert_eq!(chars.len(), 4);

    // Column 5: chars 21, 37, 53, 69
    assert!(chars.contains(&(21 as char)));
    assert!(chars.contains(&(37 as char)));
    assert!(chars.contains(&(53 as char)));
    assert!(chars.contains(&(69 as char)));
}
