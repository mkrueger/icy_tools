//! Selection after undo tests
//!
//! Tests that selection can be started after emptying the undo stack.
//! This was a bug where move_cursor_and_extend_selection and
//! move_charset_cursor_and_extend_selection would not create a new
//! selection if edit_selection/charset_selection was None.

use icy_engine_edit::bitfont::{BitFontEditState, BitFontUndoState};

/// Test that edit selection can be started with Shift+Arrow after undo empties stack
#[test]
fn test_edit_selection_after_undo_stack_empty() {
    let mut state = BitFontEditState::new();

    // 1. Make some edits to populate undo stack
    state.set_cursor_pos(2, 2);
    state.start_edit_selection();
    state.move_cursor_and_extend_selection(1, 0);
    state.move_cursor_and_extend_selection(0, 1);

    // Verify selection exists
    assert!(state.edit_selection().is_some());

    // 2. Clear selection and undo everything
    state.clear_edit_selection();

    while state.can_undo() {
        let _ = state.undo();
    }

    // 3. After all undos, selection should be None
    assert!(state.edit_selection().is_none());
    assert_eq!(state.undo_stack_len(), 0);

    // 4. Now try to start a new selection with move_cursor_and_extend_selection
    // (This simulates Shift+Arrow when no selection exists)
    state.set_cursor_pos(4, 4);
    state.move_cursor_and_extend_selection(1, 0); // Shift+Right

    // 5. Selection should now exist with anchor at (4,4) and lead at (5,4)
    let sel = state.edit_selection().expect("Selection should be created");
    assert_eq!(sel.anchor.x, 4);
    assert_eq!(sel.anchor.y, 4);
    assert_eq!(sel.lead.x, 5);
    assert_eq!(sel.lead.y, 4);
}

/// Test that charset selection can be started with Shift+Arrow after undo empties stack
#[test]
fn test_charset_selection_after_undo_stack_empty() {
    let mut state = BitFontEditState::new();

    // 1. Make some edits to populate undo stack
    state.set_charset_cursor(2, 2);
    state.start_charset_selection();
    state.move_charset_cursor_and_extend_selection(1, 0, false);
    state.move_charset_cursor_and_extend_selection(0, 1, false);

    // Verify selection exists
    assert!(state.charset_selection().is_some());

    // 2. Clear selection and undo everything
    state.clear_charset_selection();

    while state.can_undo() {
        let _ = state.undo();
    }

    // 3. After all undos, selection should be None
    assert!(state.charset_selection().is_none());
    assert_eq!(state.undo_stack_len(), 0);

    // 4. Now try to start a new selection with move_charset_cursor_and_extend_selection
    // (This simulates Shift+Arrow when no selection exists)
    state.set_charset_cursor(4, 4);
    state.move_charset_cursor_and_extend_selection(1, 0, false); // Shift+Right

    // 5. Selection should now exist with anchor at (4,4) and lead at (5,4)
    let sel = state.charset_selection().expect("Selection should be created");
    let (anchor, lead, is_rect) = sel;
    assert_eq!(anchor.x, 4);
    assert_eq!(anchor.y, 4);
    assert_eq!(lead.x, 5);
    assert_eq!(lead.y, 4);
    assert!(!is_rect);
}

/// Test multiple undo/redo cycles don't break selection
#[test]
fn test_selection_survives_undo_redo_cycles() {
    let mut state = BitFontEditState::new();

    // Create selection
    state.set_cursor_pos(1, 1);
    state.move_cursor_and_extend_selection(2, 2);

    let sel = state.edit_selection().expect("should have selection");
    assert_eq!((sel.anchor.x, sel.anchor.y), (1, 1));
    assert_eq!((sel.lead.x, sel.lead.y), (3, 3));

    // Undo
    let _ = state.undo();
    assert!(state.edit_selection().is_none());

    // Redo
    let _ = state.redo();
    let sel = state.edit_selection().expect("should have selection after redo");
    assert_eq!((sel.anchor.x, sel.anchor.y), (1, 1));
    assert_eq!((sel.lead.x, sel.lead.y), (3, 3));

    // Multiple undo/redo cycles
    for _ in 0..3 {
        let _ = state.undo();
        let _ = state.redo();
    }

    // Selection should still be intact
    let sel = state.edit_selection().expect("should have selection after cycles");
    assert_eq!((sel.anchor.x, sel.anchor.y), (1, 1));
    assert_eq!((sel.lead.x, sel.lead.y), (3, 3));
}

/// Test that selection can be extended multiple times after undo
#[test]
fn test_extend_selection_multiple_times_after_undo() {
    let mut state = BitFontEditState::new();

    // Create and undo a selection
    state.set_cursor_pos(0, 0);
    state.start_edit_selection();
    state.move_cursor_and_extend_selection(1, 1);
    state.clear_edit_selection();

    while state.can_undo() {
        let _ = state.undo();
    }

    // Now create a new selection with multiple extends
    state.set_cursor_pos(5, 5);
    state.move_cursor_and_extend_selection(1, 0); // First extend creates selection
    state.move_cursor_and_extend_selection(1, 0); // Second extend
    state.move_cursor_and_extend_selection(0, 1); // Third extend

    let sel = state.edit_selection().expect("should have selection");
    assert_eq!((sel.anchor.x, sel.anchor.y), (5, 5));
    assert_eq!((sel.lead.x, sel.lead.y), (7, 6));
}
