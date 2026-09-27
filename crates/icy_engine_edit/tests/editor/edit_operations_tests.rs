//! Tests for edit operations (set_char, swap_char, paste, resize, rows, columns)

use icy_engine::{AttributedChar, Position, Size, TextAttribute, TextPane};
use icy_engine_edit::{EditState, UndoState};

/// Helper to create an EditState with a given size
fn create_test_state(width: i32, height: i32) -> EditState {
    let buffer = icy_engine::TextBuffer::create((width, height));
    EditState::from_buffer(buffer)
}

/// Helper to get character at position
fn char_at(state: &EditState, x: i32, y: i32) -> char {
    state.get_buffer().layers[0].char_at(Position::new(x, y)).ch
}

#[test]
fn selection_mask_covers_exactly_the_selected_cells() {
    let mut state = create_test_state(20, 10);
    let selection = icy_engine::Selection::from(icy_engine::Rectangle::from(2, 3, 4, 2));
    state.set_selection(selection).unwrap();
    state.add_selection_to_mask().unwrap();
    state.deselect().unwrap();
    for (x, y) in [(2, 3), (5, 3), (2, 4), (5, 4)] {
        assert!(state.get_is_mask_selected(Position::new(x, y)), "({x}, {y}) belongs to the selection");
    }
    for (x, y) in [(1, 3), (6, 3), (2, 2), (2, 5)] {
        assert!(!state.get_is_mask_selected(Position::new(x, y)), "({x}, {y}) is outside the selection");
    }
    let bounds = state.selected_rectangle();
    assert_eq!((bounds.start, bounds.size()), (Position::new(2, 3), icy_engine::Size::new(4, 2)));
}

#[test]
fn selection_deselect_and_subtract_roundtrip() {
    let mut state = create_test_state(20, 10);
    let mut selection = icy_engine::Selection::from(icy_engine::Rectangle::from(2, 2, 6, 3));
    state.set_selection(selection).unwrap();
    state.deselect().unwrap();
    assert!(state.selection().is_none());
    state.undo().unwrap();
    assert_eq!(state.selection(), Some(selection));
    state.redo().unwrap();
    assert!(state.selection().is_none());
    state.set_selection(selection).unwrap();
    state.add_selection_to_mask().unwrap();
    state.deselect().unwrap();
    selection = icy_engine::Selection::from(icy_engine::Rectangle::from(3, 2, 2, 2));
    selection.add_type = icy_engine::AddType::Subtract;
    state.set_selection(selection).unwrap();
    {
        let _undo = state.begin_atomic_undo("Subtract mask");
        state.add_selection_to_mask().unwrap();
        state.deselect().unwrap();
    }
    assert!(state.is_selected(Position::new(2, 2)));
    assert!(!state.is_selected(Position::new(3, 2)));
    state.undo().unwrap();
    assert_eq!(state.selection(), Some(selection));
    state.redo().unwrap();
    assert!(state.selection().is_none());
    assert!(!state.is_selected(Position::new(3, 2)));
    assert!(state.is_selected(Position::new(2, 2)));
}

#[test]
fn tag_properties_and_position_roundtrip() {
    let mut state = create_test_state(20, 10);
    let tag = icy_engine::Tag {
        is_enabled: true,
        preview: "TAG".into(),
        replacement_value: String::new(),
        position: Position::new(2, 3),
        length: 3,
        alignment: std::fmt::Alignment::Left,
        tag_placement: icy_engine::TagPlacement::InText,
        tag_role: icy_engine::TagRole::Displaycode,
        attribute: TextAttribute::default(),
    };
    state.add_new_tag(tag.clone()).unwrap();
    state.clone_tag(0).unwrap();
    assert_eq!(state.get_buffer().tags.len(), 2);
    state.undo().unwrap();
    assert_eq!(state.get_buffer().tags, vec![tag.clone()]);
    state.redo().unwrap();
    assert_eq!(state.get_buffer().tags.len(), 2);
    state.undo().unwrap();
    let mut edited = tag.clone();
    edited.preview = "EDITED".into();
    state.update_tag(edited.clone(), 0).unwrap();
    for _ in 0..2 {
        state.undo().unwrap();
        assert_eq!(state.get_buffer().tags[0], tag);
        state.redo().unwrap();
        assert_eq!(state.get_buffer().tags[0], edited);
    }
    state.move_tag(0, Position::new(5, 6)).unwrap();
    for _ in 0..2 {
        state.undo().unwrap();
        assert_eq!(state.get_buffer().tags[0].position, tag.position);
        state.redo().unwrap();
        assert_eq!(state.get_buffer().tags[0].position, Position::new(5, 6));
    }
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

#[test]
fn test_resize_buffer_with_layer_resize_expands_width_while_shrinking_height() {
    let mut state = create_test_state(80, 25);

    state.resize_buffer(true, Size::new(96, 18)).unwrap();

    assert_eq!(state.get_buffer().size(), Size::new(96, 18));
    assert_eq!(state.get_buffer().layers[0].size(), Size::new(96, 18));
}

#[test]
fn test_switch_to_palette_preserves_layers() {
    let mut state = create_test_state(20, 10);
    state.add_new_layer(0).unwrap();

    state.get_buffer_mut().layers[0].set_char(Position::new(1, 1), AttributedChar::new('B', TextAttribute::default()));
    state.get_buffer_mut().layers[1].set_char(Position::new(2, 2), AttributedChar::new('T', TextAttribute::default()));

    let old_palette_title = state.get_buffer().palette.title.clone();
    let mut new_palette = state.get_buffer().palette.clone();
    new_palette.title = "Test palette".to_string();

    let initial_undo_len = state.undo_stack_len();
    state.switch_to_palette(new_palette).unwrap();

    assert_eq!(state.get_buffer().palette.title, "Test palette");
    assert_eq!(state.get_buffer().layers.len(), 2);
    assert_eq!(state.get_buffer().layers[0].char_at(Position::new(1, 1)).ch, 'B');
    assert_eq!(state.get_buffer().layers[1].char_at(Position::new(2, 2)).ch, 'T');
    assert_eq!(state.undo_stack_len(), initial_undo_len + 1);

    state.undo().unwrap();
    assert_eq!(state.get_buffer().palette.title, old_palette_title);
    assert_eq!(state.get_buffer().layers.len(), 2);
    assert_eq!(state.get_buffer().layers[0].char_at(Position::new(1, 1)).ch, 'B');
    assert_eq!(state.get_buffer().layers[1].char_at(Position::new(2, 2)).ch, 'T');

    state.redo().unwrap();
    assert_eq!(state.get_buffer().palette.title, "Test palette");
    assert_eq!(state.get_buffer().layers.len(), 2);
    assert_eq!(state.get_buffer().layers[0].char_at(Position::new(1, 1)).ch, 'B');
    assert_eq!(state.get_buffer().layers[1].char_at(Position::new(2, 2)).ch, 'T');
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
