//! Tests for edit operations (set_char, swap_char, paste, resize, rows, columns)

use icy_engine::{AttributedChar, Position, Size, TextAttribute, TextPane};
use icy_engine_edit::EditState;

/// Helper to create an EditState with a given size
fn create_test_state(width: i32, height: i32) -> EditState {
    let buffer = icy_engine::TextBuffer::create((width, height));
    EditState::from_buffer(buffer)
}

/// Helper to get character at position
fn char_at(state: &EditState, x: i32, y: i32) -> char {
    state.get_buffer().layers[0].char_at(Position::new(x, y)).ch
}

// ============================================================================
// Set Char Tests
// ============================================================================

#[test]
fn test_set_char_changes_character() {
    let mut state = create_test_state(20, 10);

    let initial_undo_len = state.undo_stack_len();

    let ch = AttributedChar::new('X', TextAttribute::default());
    state.set_char(Position::new(5, 3), ch).unwrap();

    assert_eq!(char_at(&state, 5, 3), 'X');
    assert_eq!(state.undo_stack_len(), initial_undo_len + 1);
}

#[test]
fn test_set_char_in_atomic_does_not_push_separate_undo() {
    let mut state = create_test_state(20, 10);

    let initial_undo_len = state.undo_stack_len();

    {
        let _guard = state.begin_atomic_undo("test atomic");
        let ch = AttributedChar::new('A', TextAttribute::default());
        state.set_char_in_atomic(Position::new(1, 1), ch).unwrap();
        state.set_char_in_atomic(Position::new(2, 1), ch).unwrap();
        state.set_char_in_atomic(Position::new(3, 1), ch).unwrap();
    }

    // All three set_char_in_atomic calls should result in exactly one undo operation
    assert_eq!(state.undo_stack_len(), initial_undo_len + 1);
}

#[test]
fn test_set_char_at_layer_in_atomic() {
    let mut state = create_test_state(20, 10);

    let initial_undo_len = state.undo_stack_len();

    {
        let _guard = state.begin_atomic_undo("test layer atomic");
        let ch = AttributedChar::new('L', TextAttribute::default());
        state.set_char_at_layer_in_atomic(0, Position::new(5, 5), ch).unwrap();
    }

    assert_eq!(char_at(&state, 5, 5), 'L');
    assert_eq!(state.undo_stack_len(), initial_undo_len + 1);
}

// ============================================================================
// Swap Char Tests
// ============================================================================

#[test]
fn test_swap_char_exchanges_characters() {
    let mut state = create_test_state(20, 10);

    // Set two different characters
    if let Some(layer) = state.get_cur_layer_mut() {
        layer.set_char(Position::new(1, 1), AttributedChar::new('A', TextAttribute::default()));
        layer.set_char(Position::new(5, 5), AttributedChar::new('B', TextAttribute::default()));
    }

    let initial_undo_len = state.undo_stack_len();

    state.swap_char(Position::new(1, 1), Position::new(5, 5)).unwrap();

    assert_eq!(char_at(&state, 1, 1), 'B');
    assert_eq!(char_at(&state, 5, 5), 'A');
    assert_eq!(state.undo_stack_len(), initial_undo_len + 1);
}

// ============================================================================
// Resize Buffer Tests
// ============================================================================

#[test]
fn test_resize_buffer_changes_size() {
    let mut state = create_test_state(80, 25);

    let initial_undo_len = state.undo_stack_len();

    state.resize_buffer(false, Size::new(40, 20)).unwrap();

    assert_eq!(state.get_buffer().size(), Size::new(40, 20));
    assert_eq!(state.undo_stack_len(), initial_undo_len + 1);
}

#[test]
fn test_resize_buffer_with_layer_resize() {
    let mut state = create_test_state(80, 25);

    // Fill with content
    if let Some(layer) = state.get_cur_layer_mut() {
        for y in 0..25 {
            for x in 0..80 {
                layer.set_char(Position::new(x, y), AttributedChar::new('X', TextAttribute::default()));
            }
        }
    }

    let initial_undo_len = state.undo_stack_len();

    state.resize_buffer(true, Size::new(40, 12)).unwrap();

    assert_eq!(state.get_buffer().size(), Size::new(40, 12));
    // Content should be preserved in the resized area
    assert_eq!(char_at(&state, 0, 0), 'X');
    assert_eq!(state.undo_stack_len(), initial_undo_len + 1);
}

// ============================================================================
// Row Operations Tests
// ============================================================================

#[test]
fn test_delete_row_removes_row() {
    let mut state = create_test_state(20, 10);

    // Fill each row with a different character
    if let Some(layer) = state.get_cur_layer_mut() {
        for y in 0..10 {
            let ch = char::from_u32('0' as u32 + y as u32).unwrap();
            for x in 0..20 {
                layer.set_char(Position::new(x, y), AttributedChar::new(ch, TextAttribute::default()));
            }
        }
    }

    // Position caret on row 3
    state.set_caret_position(Position::new(0, 3));

    let initial_undo_len = state.undo_stack_len();
    state.delete_row().unwrap();

    // Row 3 should now contain what was row 4
    assert_eq!(char_at(&state, 0, 3), '4');
    assert_eq!(state.undo_stack_len(), initial_undo_len + 1);
}

#[test]
fn test_insert_row_adds_row() {
    let mut state = create_test_state(20, 10);

    // Fill each row with a different character
    if let Some(layer) = state.get_cur_layer_mut() {
        for y in 0..10 {
            let ch = char::from_u32('0' as u32 + y as u32).unwrap();
            for x in 0..20 {
                layer.set_char(Position::new(x, y), AttributedChar::new(ch, TextAttribute::default()));
            }
        }
    }

    // Position caret on row 3
    state.set_caret_position(Position::new(0, 3));

    let initial_undo_len = state.undo_stack_len();
    state.insert_row().unwrap();

    // Row 4 should now contain what was row 3
    assert_eq!(char_at(&state, 0, 4), '3');
    assert_eq!(state.undo_stack_len(), initial_undo_len + 1);
}

// ============================================================================
// Column Operations Tests
// ============================================================================

#[test]
fn test_delete_column_removes_column() {
    let mut state = create_test_state(20, 10);

    // Fill each column with a different character
    if let Some(layer) = state.get_cur_layer_mut() {
        for x in 0..20 {
            let ch = char::from_u32('A' as u32 + x as u32).unwrap();
            for y in 0..10 {
                layer.set_char(Position::new(x, y), AttributedChar::new(ch, TextAttribute::default()));
            }
        }
    }

    // Position caret on column 5
    state.set_caret_position(Position::new(5, 0));

    let initial_undo_len = state.undo_stack_len();
    state.delete_column().unwrap();

    // Column 5 should now contain what was column 6 ('G')
    assert_eq!(char_at(&state, 5, 0), 'G');
    assert_eq!(state.undo_stack_len(), initial_undo_len + 1);
}

#[test]
fn test_insert_column_adds_column() {
    let mut state = create_test_state(20, 10);

    // Fill each column with a different character
    if let Some(layer) = state.get_cur_layer_mut() {
        for x in 0..20 {
            let ch = char::from_u32('A' as u32 + x as u32).unwrap();
            for y in 0..10 {
                layer.set_char(Position::new(x, y), AttributedChar::new(ch, TextAttribute::default()));
            }
        }
    }

    // Position caret on column 5
    state.set_caret_position(Position::new(5, 0));

    let initial_undo_len = state.undo_stack_len();
    state.insert_column().unwrap();

    // Column 6 should now contain what was column 5 ('F')
    assert_eq!(char_at(&state, 6, 0), 'F');
    assert_eq!(state.undo_stack_len(), initial_undo_len + 1);
}

// ============================================================================
// Erase Row/Column Tests
// ============================================================================

#[test]
fn test_erase_row_clears_row() {
    let mut state = create_test_state(20, 10);

    // Fill with content using set_char to ensure proper undo tracking
    for y in 0..10 {
        for x in 0..20 {
            let ch = AttributedChar::new('X', TextAttribute::default());
            state.set_char(Position::new(x, y), ch).unwrap();
        }
    }

    // Verify content is set
    assert_eq!(char_at(&state, 0, 5), 'X', "Content should be set before erase");

    // Position caret on row 5
    state.set_caret_position(Position::new(0, 5));

    let initial_undo_len = state.undo_stack_len();
    state.erase_row().unwrap();

    // Row 5 should be cleared
    let ch = state.get_buffer().layers[0].char_at(Position::new(0, 5));
    assert!(!ch.is_visible() || ch.is_transparent(), "Row 5 should be cleared");

    // Should have pushed at least one undo operation
    assert!(state.undo_stack_len() > initial_undo_len, "Should push undo operation");
}

#[test]
fn test_erase_column_clears_column() {
    let mut state = create_test_state(20, 10);

    // Fill with content using set_char
    for y in 0..10 {
        for x in 0..20 {
            let ch = AttributedChar::new('Y', TextAttribute::default());
            state.set_char(Position::new(x, y), ch).unwrap();
        }
    }

    // Verify content is set
    assert_eq!(char_at(&state, 8, 0), 'Y', "Content should be set before erase");

    // Position caret on column 8
    state.set_caret_position(Position::new(8, 0));

    let initial_undo_len = state.undo_stack_len();
    state.erase_column().unwrap();

    // Column 8 should be cleared
    let ch = state.get_buffer().layers[0].char_at(Position::new(8, 0));
    assert!(!ch.is_visible() || ch.is_transparent(), "Column 8 should be cleared");

    // Should have pushed at least one undo operation
    assert!(state.undo_stack_len() > initial_undo_len, "Should push undo operation");
}

// ============================================================================
// Line Justify Tests
// ============================================================================

#[test]
fn test_center_line_centers_content() {
    let mut state = create_test_state(20, 10);

    // Put content at the left of row 3
    if let Some(layer) = state.get_cur_layer_mut() {
        for x in 0..4 {
            layer.set_char(Position::new(x, 3), AttributedChar::new('C', TextAttribute::default()));
        }
    }

    // Position caret on row 3
    state.set_caret_position(Position::new(0, 3));

    let initial_undo_len = state.undo_stack_len();
    state.center_line().unwrap();

    // Content should be approximately centered
    assert_eq!(char_at(&state, 8, 3), 'C');

    assert_eq!(state.undo_stack_len(), initial_undo_len + 1);
}

#[test]
fn test_justify_line_left_moves_content() {
    let mut state = create_test_state(20, 10);

    // Put content at the right of row 3
    if let Some(layer) = state.get_cur_layer_mut() {
        for x in 15..20 {
            layer.set_char(Position::new(x, 3), AttributedChar::new('L', TextAttribute::default()));
        }
    }

    // Position caret on row 3
    state.set_caret_position(Position::new(0, 3));

    let initial_undo_len = state.undo_stack_len();
    state.justify_line_left().unwrap();

    // Content should be at left edge
    assert_eq!(char_at(&state, 0, 3), 'L');

    assert_eq!(state.undo_stack_len(), initial_undo_len + 1);
}

#[test]
fn test_justify_line_right_moves_content() {
    let mut state = create_test_state(20, 10);

    // Put content at the left of row 3
    if let Some(layer) = state.get_cur_layer_mut() {
        for x in 0..5 {
            layer.set_char(Position::new(x, 3), AttributedChar::new('R', TextAttribute::default()));
        }
    }

    // Position caret on row 3
    state.set_caret_position(Position::new(0, 3));

    let initial_undo_len = state.undo_stack_len();
    state.justify_line_right().unwrap();

    // Content should be at right edge
    assert_eq!(char_at(&state, 19, 3), 'R');

    assert_eq!(state.undo_stack_len(), initial_undo_len + 1);
}
