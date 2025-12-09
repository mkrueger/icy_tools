//! Single operation undo/redo tests
//!
//! Tests that each operation type can be properly undone and redone.

use icy_engine_edit::bitfont::{BitFontEditState, BitFontUndoState};

#[test]
fn test_undo_stack_initially_empty() {
    let state = BitFontEditState::new();
    assert_eq!(state.undo_stack_len(), 0);
    assert_eq!(state.redo_stack_len(), 0);
}

#[test]
fn test_operation_adds_to_undo_stack() {
    let mut state = BitFontEditState::new();

    state.set_pixel('A', 1, 1, true).unwrap();

    assert_eq!(state.undo_stack_len(), 1);
    assert_eq!(state.redo_stack_len(), 0);
}

#[test]
fn test_undo_moves_to_redo_stack() {
    let mut state = BitFontEditState::new();

    state.set_pixel('A', 1, 1, true).unwrap();
    state.undo().unwrap();

    assert_eq!(state.undo_stack_len(), 0);
    assert_eq!(state.redo_stack_len(), 1);
}

#[test]
fn test_redo_moves_back_to_undo_stack() {
    let mut state = BitFontEditState::new();

    state.set_pixel('A', 1, 1, true).unwrap();
    state.undo().unwrap();
    state.redo().unwrap();

    assert_eq!(state.undo_stack_len(), 1);
    assert_eq!(state.redo_stack_len(), 0);
}

#[test]
fn test_new_operation_clears_redo_stack() {
    let mut state = BitFontEditState::new();

    state.set_pixel('A', 1, 1, true).unwrap();
    state.undo().unwrap();

    assert_eq!(state.redo_stack_len(), 1);

    // New operation should clear redo stack
    state.set_pixel('A', 2, 2, true).unwrap();

    assert_eq!(state.redo_stack_len(), 0);
}

#[test]
fn test_multiple_undo() {
    let mut state = BitFontEditState::new();

    // Clear 'A' first so we know the initial state
    state.clear_glyph('A').unwrap();

    // Now the clear is on the undo stack, so set pixels add 3 more
    state.set_pixel('A', 1, 1, true).unwrap();
    state.set_pixel('A', 2, 2, true).unwrap();
    state.set_pixel('A', 3, 3, true).unwrap();

    assert_eq!(state.undo_stack_len(), 4); // clear + 3 set_pixel

    state.undo().unwrap();
    state.undo().unwrap();

    assert_eq!(state.undo_stack_len(), 2); // clear + 1 set_pixel
    assert_eq!(state.redo_stack_len(), 2);

    // Only first pixel should remain (plus the cleared base)
    assert!(state.get_glyph_pixels('A')[1][1]);
    assert!(!state.get_glyph_pixels('A')[2][2]);
    assert!(!state.get_glyph_pixels('A')[3][3]);
}

#[test]
fn test_undo_empty_stack_is_noop() {
    let mut state = BitFontEditState::new();

    // Should not panic or error on empty stack
    let result = state.undo();
    assert!(result.is_ok());
    assert_eq!(state.undo_stack_len(), 0);
}

#[test]
fn test_redo_empty_stack_is_noop() {
    let mut state = BitFontEditState::new();

    // Should not panic or error on empty stack
    let result = state.redo();
    assert!(result.is_ok());
    assert_eq!(state.redo_stack_len(), 0);
}

#[test]
fn test_undo_redo_preserves_state() {
    let mut state = BitFontEditState::new();

    state.set_pixel('A', 1, 1, true).unwrap();
    state.set_pixel('A', 2, 2, true).unwrap();

    let after_two = state.get_glyph_pixels('A').clone();

    state.undo().unwrap();
    state.redo().unwrap();

    assert_eq!(state.get_glyph_pixels('A'), &after_two);
}

#[test]
fn test_undo_different_operation_types() {
    let mut state = BitFontEditState::new();

    // Start with a clean glyph
    state.clear_glyph('A').unwrap();

    // Various operations (starting from empty)
    state.set_pixel('A', 1, 1, true).unwrap();
    state.flip_glyph_x('A').unwrap();
    state.inverse_glyph('A').unwrap();
    state.move_glyph('A', 1, 1).unwrap();

    assert_eq!(state.undo_stack_len(), 5); // clear + 4 operations

    // Undo all (including the clear)
    for _ in 0..5 {
        state.undo().unwrap();
    }

    // After undoing clear, we're back to original VGA 'A' - not necessarily empty
    // Just verify the undo stack is empty now
    assert_eq!(state.undo_stack_len(), 0);
}
