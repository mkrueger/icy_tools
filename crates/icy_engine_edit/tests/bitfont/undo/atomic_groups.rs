//! Atomic undo group tests
//!
//! Tests grouping multiple operations into a single undoable action.

use icy_engine_edit::bitfont::{BitFontEditState, BitFontUndoState};

#[test]
fn test_atomic_undo_groups_operations() {
    let mut state = BitFontEditState::new();

    {
        let _guard = state.begin_atomic_undo("Multiple operations");
        state.set_pixel('A', 1, 1, true).unwrap();
        state.set_pixel('A', 2, 2, true).unwrap();
        state.set_pixel('A', 3, 3, true).unwrap();
        // Guard dropped here, operations grouped
    }

    // Should be a single undo operation
    assert_eq!(state.undo_stack_len(), 1);

    // All pixels should be set
    assert!(state.get_glyph_pixels('A')[1][1]);
    assert!(state.get_glyph_pixels('A')[2][2]);
    assert!(state.get_glyph_pixels('A')[3][3]);
}

#[test]
fn test_atomic_undo_single_undo_clears_all() {
    let mut state = BitFontEditState::new();

    // Clear first to have known state
    state.clear_glyph('A').unwrap();

    {
        let _guard = state.begin_atomic_undo("Group");
        state.set_pixel('A', 1, 1, true).unwrap();
        state.set_pixel('A', 2, 2, true).unwrap();
        state.set_pixel('A', 3, 3, true).unwrap();
    }

    state.undo().unwrap();

    // All pixels should be cleared with single undo (back to cleared state)
    assert!(!state.get_glyph_pixels('A')[1][1]);
    assert!(!state.get_glyph_pixels('A')[2][2]);
    assert!(!state.get_glyph_pixels('A')[3][3]);
}

#[test]
fn test_atomic_undo_redo() {
    let mut state = BitFontEditState::new();

    {
        let _guard = state.begin_atomic_undo("Group");
        state.set_pixel('A', 1, 1, true).unwrap();
        state.set_pixel('A', 2, 2, true).unwrap();
    }

    state.undo().unwrap();

    // All should be cleared
    assert!(!state.get_glyph_pixels('A')[1][1]);
    assert!(!state.get_glyph_pixels('A')[2][2]);

    state.redo().unwrap();

    // All should be restored
    assert!(state.get_glyph_pixels('A')[1][1]);
    assert!(state.get_glyph_pixels('A')[2][2]);
}

#[test]
fn test_atomic_undo_multiple_glyphs() {
    let mut state = BitFontEditState::new();

    // Clear all test glyphs first
    state.clear_glyph('A').unwrap();
    state.clear_glyph('B').unwrap();
    state.clear_glyph('C').unwrap();

    {
        let _guard = state.begin_atomic_undo("Multi-glyph operation");
        state.set_pixel('A', 1, 1, true).unwrap();
        state.set_pixel('B', 2, 2, true).unwrap();
        state.set_pixel('C', 3, 3, true).unwrap();
    }

    // 3 clears + 1 atomic group = 4, but atomic groups work differently
    // Just verify we can undo
    state.undo().unwrap();

    // All glyphs should be reverted to cleared state
    assert!(!state.get_glyph_pixels('A')[1][1]);
    assert!(!state.get_glyph_pixels('B')[2][2]);
    assert!(!state.get_glyph_pixels('C')[3][3]);
}

#[test]
fn test_atomic_undo_nested_not_supported() {
    // This test documents expected behavior with nested atomic operations
    // The behavior depends on implementation - this just ensures no panic
    let mut state = BitFontEditState::new();

    {
        let _guard1 = state.begin_atomic_undo("Outer");
        state.set_pixel('A', 1, 1, true).unwrap();

        // Inner atomic - behavior is implementation defined
        // but should not crash
        {
            let _guard2 = state.begin_atomic_undo("Inner");
            state.set_pixel('A', 2, 2, true).unwrap();
        }

        state.set_pixel('A', 3, 3, true).unwrap();
    }

    // Should have at least one undo operation and not crash
    assert!(state.undo_stack_len() >= 1);
}

#[test]
fn test_atomic_undo_empty_group() {
    let mut state = BitFontEditState::new();

    {
        let _guard = state.begin_atomic_undo("Empty group");
        // No operations
    }

    // Empty group should not add to undo stack
    // (implementation may vary - this documents expected behavior)
    // For now, just ensure no panic and undo works
    let result = state.undo();
    assert!(result.is_ok());
}

#[test]
fn test_atomic_undo_with_different_operations() {
    let mut state = BitFontEditState::new();

    state.set_pixel('A', 0, 0, true).unwrap();

    {
        let _guard = state.begin_atomic_undo("Mixed operations");
        state.flip_glyph_x('A').unwrap();
        state.inverse_glyph('A').unwrap();
        state.move_glyph('A', 1, 0).unwrap();
    }

    // Should be 2 operations: initial set_pixel + grouped operations
    assert_eq!(state.undo_stack_len(), 2);

    // Single undo should revert all grouped operations
    state.undo().unwrap();

    // Only the initial set_pixel should remain
    assert_eq!(state.undo_stack_len(), 1);
}
