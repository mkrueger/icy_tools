//! Edit grid cursor movement tests
//!
//! Tests cursor movement within the pixel edit grid.
//! The cursor wraps at boundaries (moving right from last column wraps to first column, etc.)
//! This matches the charset cursor behavior where X and Y wrap independently.

use icy_engine_edit::bitfont::BitFontEditState;

#[test]
fn test_cursor_initial_position() {
    let state = BitFontEditState::new();
    assert_eq!(state.cursor_pos(), (0, 0));
}

#[test]
fn test_cursor_move_right() {
    let mut state = BitFontEditState::new();
    state.move_cursor(1, 0);
    assert_eq!(state.cursor_pos(), (1, 0));
}

#[test]
fn test_cursor_move_down() {
    let mut state = BitFontEditState::new();
    state.move_cursor(0, 1);
    assert_eq!(state.cursor_pos(), (0, 1));
}

#[test]
fn test_cursor_move_left_from_origin_wraps() {
    let mut state = BitFontEditState::new();
    let (width, _) = state.font_size();
    state.move_cursor(-1, 0);
    // Cursor wraps to last column
    assert_eq!(state.cursor_pos(), (width - 1, 0));
}

#[test]
fn test_cursor_move_up_from_origin_wraps() {
    let mut state = BitFontEditState::new();
    let (_, height) = state.font_size();
    state.move_cursor(0, -1);
    // Cursor wraps to last row
    assert_eq!(state.cursor_pos(), (0, height - 1));
}

#[test]
fn test_cursor_move_right_wraps_at_boundary() {
    let mut state = BitFontEditState::new();
    let (width, _) = state.font_size();

    // Move to last column
    state.set_cursor_pos(width - 1, 0);
    assert_eq!(state.cursor_pos(), (width - 1, 0));

    // Move right should wrap to first column
    state.move_cursor(1, 0);
    assert_eq!(state.cursor_pos(), (0, 0));
}

#[test]
fn test_cursor_move_down_wraps_at_boundary() {
    let mut state = BitFontEditState::new();
    let (_, height) = state.font_size();

    // Move to last row
    state.set_cursor_pos(0, height - 1);
    assert_eq!(state.cursor_pos(), (0, height - 1));

    // Move down should wrap to first row
    state.move_cursor(0, 1);
    assert_eq!(state.cursor_pos(), (0, 0));
}

#[test]
fn test_cursor_set_position_clamps_to_bounds() {
    let mut state = BitFontEditState::new();
    let (width, height) = state.font_size();

    // Try to set beyond bounds - set_cursor_pos still clamps
    state.set_cursor_pos(100, 100);
    assert_eq!(state.cursor_pos(), (width - 1, height - 1));

    // Try negative - set_cursor_pos clamps to 0
    state.set_cursor_pos(-10, -10);
    assert_eq!(state.cursor_pos(), (0, 0));
}

#[test]
fn test_cursor_multiple_moves() {
    let mut state = BitFontEditState::new();

    // Move right 3, down 2
    state.move_cursor(3, 0);
    state.move_cursor(0, 2);
    assert_eq!(state.cursor_pos(), (3, 2));

    // Move left 1, up 1
    state.move_cursor(-1, 0);
    state.move_cursor(0, -1);
    assert_eq!(state.cursor_pos(), (2, 1));
}

#[test]
fn test_cursor_diagonal_move() {
    let mut state = BitFontEditState::new();

    // Move diagonally
    state.move_cursor(2, 3);
    assert_eq!(state.cursor_pos(), (2, 3));
}

#[test]
fn test_cursor_wrap_multiple_times() {
    let mut state = BitFontEditState::new();
    let (width, height) = state.font_size();

    // Moving by full width should end up at same column
    state.set_cursor_pos(3, 5);
    state.move_cursor(width, 0);
    assert_eq!(state.cursor_pos(), (3, 5));

    // Moving by full height should end up at same row
    state.move_cursor(0, height);
    assert_eq!(state.cursor_pos(), (3, 5));
}

#[test]
fn test_cursor_wrap_negative_multiple() {
    let mut state = BitFontEditState::new();
    let (width, height) = state.font_size();

    // Start at a middle position
    state.set_cursor_pos(4, 8);

    // Moving left by width should wrap back to same position
    state.move_cursor(-width, 0);
    assert_eq!(state.cursor_pos(), (4, 8));

    // Moving up by height should wrap back to same position
    state.move_cursor(0, -height);
    assert_eq!(state.cursor_pos(), (4, 8));
}
