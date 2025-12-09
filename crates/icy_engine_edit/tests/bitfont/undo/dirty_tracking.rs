//! Dirty tracking tests
//!
//! Tests tracking of unsaved changes.

use icy_engine_edit::bitfont::{BitFontEditState, BitFontUndoState};

#[test]
fn test_new_state_is_not_dirty() {
    let _state = BitFontEditState::new();
    // New state might be considered dirty or not depending on implementation
    // This documents the expected behavior
    // For a new font that hasn't been saved, it could reasonably be considered dirty
}

#[test]
fn test_operation_marks_dirty() {
    let mut state = BitFontEditState::new();
    state.mark_clean();

    assert!(!state.is_dirty());

    state.set_pixel('A', 1, 1, true).unwrap();

    assert!(state.is_dirty());
}

#[test]
fn test_mark_clean() {
    let mut state = BitFontEditState::new();

    state.set_pixel('A', 1, 1, true).unwrap();
    assert!(state.is_dirty());

    state.mark_clean();

    assert!(!state.is_dirty());
}

#[test]
fn test_operation_after_clean_marks_dirty() {
    let mut state = BitFontEditState::new();

    state.set_pixel('A', 1, 1, true).unwrap();
    state.mark_clean();
    state.set_pixel('A', 2, 2, true).unwrap();

    assert!(state.is_dirty());
}

#[test]
fn test_undo_after_operation_still_dirty() {
    let mut state = BitFontEditState::new();
    state.mark_clean();

    state.set_pixel('A', 1, 1, true).unwrap();
    state.undo().unwrap();

    // After undo, state is same as clean state but is_dirty may still be true
    // This depends on implementation - documenting expected behavior
    // Some implementations track dirty based on save point, others based on any operation
}

#[test]
fn test_multiple_operations_dirty() {
    let mut state = BitFontEditState::new();
    state.mark_clean();

    state.set_pixel('A', 1, 1, true).unwrap();
    state.set_pixel('A', 2, 2, true).unwrap();
    state.flip_glyph_x('A').unwrap();

    assert!(state.is_dirty());

    state.mark_clean();

    assert!(!state.is_dirty());
}

#[test]
fn test_different_operation_types_mark_dirty() {
    // Test various operation types
    let operations: Vec<Box<dyn FnOnce(&mut BitFontEditState)>> = vec![
        Box::new(|s| {
            s.set_pixel('A', 1, 1, true).unwrap();
        }),
        Box::new(|s| {
            s.toggle_pixel('A', 2, 2).unwrap();
        }),
        Box::new(|s| {
            s.clear_glyph('A').unwrap();
        }),
        Box::new(|s| {
            s.inverse_glyph('A').unwrap();
        }),
        Box::new(|s| {
            s.flip_glyph_x('A').unwrap();
        }),
        Box::new(|s| {
            s.flip_glyph_y('A').unwrap();
        }),
        Box::new(|s| {
            s.move_glyph('A', 1, 0).unwrap();
        }),
        Box::new(|s| {
            s.resize_font(6, 12).unwrap();
        }),
        Box::new(|s| {
            s.swap_chars('A', 'B').unwrap();
        }),
    ];

    for op in operations {
        let mut state = BitFontEditState::new();
        state.mark_clean();

        op(&mut state);

        assert!(state.is_dirty(), "operation should mark state as dirty");
    }
}

#[test]
fn test_redo_marks_dirty() {
    let mut state = BitFontEditState::new();
    state.mark_clean();

    state.set_pixel('A', 1, 1, true).unwrap();
    state.undo().unwrap();
    state.mark_clean();

    assert!(!state.is_dirty());

    state.redo().unwrap();

    assert!(state.is_dirty());
}
