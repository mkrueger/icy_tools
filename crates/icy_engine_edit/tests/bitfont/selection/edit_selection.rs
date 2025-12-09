//! Edit grid pixel selection tests
//!
//! Tests rectangular selection within the pixel edit grid.

use icy_engine_edit::bitfont::BitFontEditState;

#[test]
fn test_no_selection_initially() {
    let state = BitFontEditState::new();
    assert!(state.edit_selection().is_none());
}

#[test]
fn test_start_selection() {
    let mut state = BitFontEditState::new();
    state.set_cursor_pos(2, 3);
    state.start_edit_selection();

    let sel = state.edit_selection().expect("should have selection");
    assert_eq!(sel.anchor.x, 2);
    assert_eq!(sel.anchor.y, 3);
    assert_eq!(sel.lead.x, 2);
    assert_eq!(sel.lead.y, 3);
}

#[test]
fn test_extend_selection() {
    let mut state = BitFontEditState::new();
    state.set_cursor_pos(2, 3);
    state.start_edit_selection();

    state.set_cursor_pos(5, 7);
    state.extend_edit_selection();

    let sel = state.edit_selection().expect("should have selection");
    assert_eq!(sel.anchor.x, 2);
    assert_eq!(sel.anchor.y, 3);
    assert_eq!(sel.lead.x, 5);
    assert_eq!(sel.lead.y, 7);
}

#[test]
fn test_clear_selection() {
    let mut state = BitFontEditState::new();
    state.start_edit_selection();
    state.set_cursor_pos(3, 3);
    state.extend_edit_selection();

    assert!(state.edit_selection().is_some());

    state.clear_edit_selection();
    assert!(state.edit_selection().is_none());
}

#[test]
fn test_get_selection_or_all_with_selection() {
    let mut state = BitFontEditState::new();
    state.set_cursor_pos(1, 2);
    state.start_edit_selection();
    state.set_cursor_pos(4, 6);
    state.extend_edit_selection();

    let sel = state.get_edit_selection_or_all();
    // Should return the actual selection bounds
    let (min_x, min_y, max_x, max_y) = (
        sel.anchor.x.min(sel.lead.x),
        sel.anchor.y.min(sel.lead.y),
        sel.anchor.x.max(sel.lead.x),
        sel.anchor.y.max(sel.lead.y),
    );
    assert_eq!((min_x, min_y, max_x, max_y), (1, 2, 4, 6));
}

#[test]
fn test_get_selection_or_all_without_selection() {
    let state = BitFontEditState::new();
    let (width, height) = state.font_size();

    let sel = state.get_edit_selection_or_all();
    // Should return full glyph bounds
    assert_eq!(sel.anchor.x, 0);
    assert_eq!(sel.anchor.y, 0);
    assert_eq!(sel.lead.x, width - 1);
    assert_eq!(sel.lead.y, height - 1);
}

#[test]
fn test_selection_backwards() {
    let mut state = BitFontEditState::new();

    // Start selection at (5, 5), extend to (2, 2) - backwards
    state.set_cursor_pos(5, 5);
    state.start_edit_selection();
    state.set_cursor_pos(2, 2);
    state.extend_edit_selection();

    let sel = state.edit_selection().expect("should have selection");
    // Anchor stays at start point, lead follows cursor
    assert_eq!(sel.anchor.x, 5);
    assert_eq!(sel.anchor.y, 5);
    assert_eq!(sel.lead.x, 2);
    assert_eq!(sel.lead.y, 2);
}

#[test]
fn test_set_selection_legacy() {
    let mut state = BitFontEditState::new();

    // Use legacy set_selection method
    state.set_selection(Some((1, 2, 3, 4)));

    let sel = state.edit_selection().expect("should have selection");
    assert_eq!(sel.anchor.x, 1);
    assert_eq!(sel.anchor.y, 2);
    assert_eq!(sel.lead.x, 3);
    assert_eq!(sel.lead.y, 4);
}

#[test]
fn test_set_selection_none() {
    let mut state = BitFontEditState::new();
    state.set_selection(Some((1, 2, 3, 4)));

    state.set_selection(None);
    assert!(state.edit_selection().is_none());
}
