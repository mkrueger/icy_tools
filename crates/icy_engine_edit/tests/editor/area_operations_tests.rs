//! Tests for area operations (crop, flip, justify, scroll, erase)

use icy_engine::{AttributedChar, Position, Rectangle, Selection, Shape, Size, TextAttribute, TextPane};
use icy_engine_edit::EditState;

/// Helper to create an EditState with a given size
fn create_test_state(width: i32, height: i32) -> EditState {
    let buffer = icy_engine::TextBuffer::create((width, height));
    EditState::from_buffer(buffer)
}

/// Helper to fill a region with a specific character
fn fill_region(state: &mut EditState, rect: Rectangle, ch: char) {
    let attributed_char = AttributedChar::new(ch, TextAttribute::default());
    if let Some(layer) = state.get_cur_layer_mut() {
        for y in rect.y_range() {
            for x in rect.x_range() {
                layer.set_char(Position::new(x, y), attributed_char);
            }
        }
    }
}

/// Helper to get character at position
fn char_at(state: &EditState, x: i32, y: i32) -> char {
    state.get_buffer().layers[0].char_at(Position::new(x, y)).ch
}

/// Helper to create a rectangle selection from coordinates
fn rect_selection(x: i32, y: i32, w: i32, h: i32) -> Selection {
    let mut sel = Selection::new(Position::new(x, y));
    sel.lead = Position::new(x + w - 1, y + h - 1);
    sel.shape = Shape::Rectangle;
    sel
}

// ============================================================================
// Crop Tests
// ============================================================================

#[test]
fn test_crop_reduces_buffer_size() {
    let mut state = create_test_state(80, 25);
    fill_region(&mut state, Rectangle::from_min_size((0, 0), (80, 25)), 'A');

    let initial_undo_len = state.undo_stack_len();

    // Crop to a 10x10 region starting at (5, 5)
    let crop_rect = Rectangle::from_min_size((5, 5), (10, 10));
    state.crop_rect(crop_rect).unwrap();

    // Verify size changed
    assert_eq!(state.get_buffer().size(), Size::new(10, 10));

    // Verify exactly one undo operation was pushed
    assert_eq!(state.undo_stack_len(), initial_undo_len + 1);
}

#[test]
fn test_crop_preserves_content() {
    let mut state = create_test_state(20, 20);

    // Fill specific area with 'X'
    fill_region(&mut state, Rectangle::from_min_size((5, 5), (5, 5)), 'X');

    // Crop to that area
    state.crop_rect(Rectangle::from_min_size((5, 5), (5, 5))).unwrap();

    // All characters should be 'X'
    for y in 0..5 {
        for x in 0..5 {
            assert_eq!(char_at(&state, x, y), 'X', "Expected 'X' at ({}, {})", x, y);
        }
    }
}

#[test]
fn test_crop_with_selection() {
    let mut state = create_test_state(40, 20);
    fill_region(&mut state, Rectangle::from_min_size((0, 0), (40, 20)), 'B');

    // Set selection (inclusive bounds: anchor and lead are both part of selection)
    state.set_selection(rect_selection(10, 5, 15, 8)).unwrap();

    let initial_undo_len = state.undo_stack_len();
    state.crop().unwrap();

    // Verify size is selection size
    assert_eq!(state.get_buffer().size(), Size::new(15, 8));
    assert_eq!(state.undo_stack_len(), initial_undo_len + 1);
}

// ============================================================================
// Flip X Tests
// ============================================================================

#[test]
fn test_flip_x_mirrors_content() {
    let mut state = create_test_state(10, 5);

    // Put 'L' on left side, 'R' on right side
    fill_region(&mut state, Rectangle::from_min_size((0, 0), (5, 5)), 'L');
    fill_region(&mut state, Rectangle::from_min_size((5, 0), (5, 5)), 'R');

    let initial_undo_len = state.undo_stack_len();
    state.flip_x().unwrap();

    // After flip, left side should have 'R', right side should have 'L'
    assert_eq!(char_at(&state, 0, 0), 'R');
    assert_eq!(char_at(&state, 9, 0), 'L');

    // Verify exactly one undo operation
    assert_eq!(state.undo_stack_len(), initial_undo_len + 1);
}

#[test]
fn test_flip_x_with_selection() {
    let mut state = create_test_state(20, 10);
    fill_region(&mut state, Rectangle::from_min_size((0, 0), (20, 10)), '.');

    // Fill selection area: left='A', right='B'
    fill_region(&mut state, Rectangle::from_min_size((5, 2), (3, 4)), 'A');
    fill_region(&mut state, Rectangle::from_min_size((8, 2), (3, 4)), 'B');

    // Set selection
    state.set_selection(rect_selection(5, 2, 6, 4)).unwrap();

    let initial_undo_len = state.undo_stack_len();
    state.flip_x().unwrap();

    // After flip within selection, positions should be swapped
    assert_eq!(char_at(&state, 5, 2), 'B');
    assert_eq!(char_at(&state, 10, 2), 'A');

    assert_eq!(state.undo_stack_len(), initial_undo_len + 1);
}

// ============================================================================
// Flip Y Tests
// ============================================================================

#[test]
fn test_flip_y_mirrors_content() {
    let mut state = create_test_state(10, 10);

    // Put 'T' on top half, 'B' on bottom half
    fill_region(&mut state, Rectangle::from_min_size((0, 0), (10, 5)), 'T');
    fill_region(&mut state, Rectangle::from_min_size((0, 5), (10, 5)), 'B');

    let initial_undo_len = state.undo_stack_len();
    state.flip_y().unwrap();

    // After flip, top should have 'B', bottom should have 'T'
    assert_eq!(char_at(&state, 0, 0), 'B');
    assert_eq!(char_at(&state, 0, 9), 'T');

    assert_eq!(state.undo_stack_len(), initial_undo_len + 1);
}

// ============================================================================
// Justify Left Tests
// ============================================================================

#[test]
fn test_justify_left_moves_content() {
    let mut state = create_test_state(20, 5);

    // Put some spaces followed by 'X' characters
    if let Some(layer) = state.get_cur_layer_mut() {
        for x in 5..10 {
            layer.set_char(Position::new(x, 0), AttributedChar::new('X', TextAttribute::default()));
        }
    }

    let initial_undo_len = state.undo_stack_len();
    state.justify_left().unwrap();

    // Content should be at the left edge now
    assert_eq!(char_at(&state, 0, 0), 'X');

    assert_eq!(state.undo_stack_len(), initial_undo_len + 1);
}

#[test]
fn test_justify_left_with_selection() {
    let mut state = create_test_state(30, 10);

    // Put content in selection area with leading spaces
    if let Some(layer) = state.get_cur_layer_mut() {
        for x in 15..20 {
            layer.set_char(Position::new(x, 5), AttributedChar::new('Y', TextAttribute::default()));
        }
    }

    // Select the area
    state.set_selection(rect_selection(10, 5, 15, 1)).unwrap();

    let initial_undo_len = state.undo_stack_len();
    state.justify_left().unwrap();

    // Content should be at left edge of selection (x=10)
    assert_eq!(char_at(&state, 10, 5), 'Y');

    assert_eq!(state.undo_stack_len(), initial_undo_len + 1);
}

// ============================================================================
// Center Tests
// ============================================================================

#[test]
fn test_center_moves_content() {
    let mut state = create_test_state(20, 5);

    // Put 'C' characters at the left edge
    if let Some(layer) = state.get_cur_layer_mut() {
        for x in 0..4 {
            layer.set_char(Position::new(x, 0), AttributedChar::new('C', TextAttribute::default()));
        }
    }

    let initial_undo_len = state.undo_stack_len();
    state.center().unwrap();

    // Content should be centered (approximately at position 8)
    // Original: "CCCC" (4 chars) in 20 width
    // After center: should have ~8 spaces on each side
    assert_eq!(char_at(&state, 8, 0), 'C');

    assert_eq!(state.undo_stack_len(), initial_undo_len + 1);
}

// ============================================================================
// Justify Right Tests
// ============================================================================

#[test]
fn test_justify_right_moves_content() {
    let mut state = create_test_state(20, 5);

    // Put 'R' characters at the left edge
    if let Some(layer) = state.get_cur_layer_mut() {
        for x in 0..5 {
            layer.set_char(Position::new(x, 0), AttributedChar::new('R', TextAttribute::default()));
        }
    }

    let initial_undo_len = state.undo_stack_len();
    state.justify_right().unwrap();

    // Content should be at the right edge
    assert_eq!(char_at(&state, 19, 0), 'R');
    assert_eq!(char_at(&state, 15, 0), 'R');

    assert_eq!(state.undo_stack_len(), initial_undo_len + 1);
}

// ============================================================================
// Erase Selection Tests
// ============================================================================

#[test]
fn test_erase_selection_clears_content() {
    let mut state = create_test_state(20, 10);
    fill_region(&mut state, Rectangle::from_min_size((0, 0), (20, 10)), 'E');

    // Set selection
    state.set_selection(rect_selection(5, 3, 8, 4)).unwrap();

    let initial_undo_len = state.undo_stack_len();
    state.erase_selection().unwrap();

    // Selected area should be cleared (invisible chars)
    let ch = state.get_buffer().layers[0].char_at(Position::new(5, 3));
    assert!(!ch.is_visible() || ch.is_transparent());

    // Outside selection should still have 'E'
    assert_eq!(char_at(&state, 0, 0), 'E');

    assert_eq!(state.undo_stack_len(), initial_undo_len + 1);
}

#[test]
fn test_erase_selection_without_selection_does_nothing() {
    let mut state = create_test_state(20, 10);
    fill_region(&mut state, Rectangle::from_min_size((0, 0), (20, 10)), 'N');

    // No selection - clear any existing selection
    let _ = state.clear_selection();

    let initial_undo_len = state.undo_stack_len();
    state.erase_selection().unwrap();

    // Nothing should change
    assert_eq!(char_at(&state, 0, 0), 'N');

    // No undo operation should be pushed
    assert_eq!(state.undo_stack_len(), initial_undo_len);
}

// ============================================================================
// Scroll Area Up Tests
// ============================================================================

#[test]
fn test_scroll_area_up_moves_content() {
    let mut state = create_test_state(10, 10);

    // Put distinct content on each row
    if let Some(layer) = state.get_cur_layer_mut() {
        for y in 0..10 {
            let ch = char::from_u32('0' as u32 + y as u32).unwrap();
            for x in 0..10 {
                layer.set_char(Position::new(x, y), AttributedChar::new(ch, TextAttribute::default()));
            }
        }
    }

    let initial_undo_len = state.undo_stack_len();
    state.scroll_area_up().unwrap();

    // Row 0 should now have content from row 1
    assert_eq!(char_at(&state, 0, 0), '1');
    // Row 9 should have content from row 0 (wrapped)
    assert_eq!(char_at(&state, 0, 9), '0');

    assert_eq!(state.undo_stack_len(), initial_undo_len + 1);
}

#[test]
fn test_scroll_area_up_with_selection() {
    let mut state = create_test_state(20, 10);
    fill_region(&mut state, Rectangle::from_min_size((0, 0), (20, 10)), '.');

    // Fill selection area with distinct rows
    if let Some(layer) = state.get_cur_layer_mut() {
        for y in 2..6 {
            let ch = char::from_u32('A' as u32 + (y - 2) as u32).unwrap();
            for x in 5..10 {
                layer.set_char(Position::new(x, y), AttributedChar::new(ch, TextAttribute::default()));
            }
        }
    }

    state.set_selection(rect_selection(5, 2, 5, 4)).unwrap();

    let initial_undo_len = state.undo_stack_len();
    state.scroll_area_up().unwrap();

    // Row 2 (first in selection) should have 'B' (from row 3)
    assert_eq!(char_at(&state, 5, 2), 'B');

    assert_eq!(state.undo_stack_len(), initial_undo_len + 1);
}

// ============================================================================
// Scroll Area Down Tests
// ============================================================================

#[test]
fn test_scroll_area_down_moves_content() {
    let mut state = create_test_state(10, 10);

    // Put distinct content on each row
    if let Some(layer) = state.get_cur_layer_mut() {
        for y in 0..10 {
            let ch = char::from_u32('0' as u32 + y as u32).unwrap();
            for x in 0..10 {
                layer.set_char(Position::new(x, y), AttributedChar::new(ch, TextAttribute::default()));
            }
        }
    }

    let initial_undo_len = state.undo_stack_len();
    state.scroll_area_down().unwrap();

    // Row 1 should now have content from row 0
    assert_eq!(char_at(&state, 0, 1), '0');
    // Row 0 should have content from row 9 (wrapped)
    assert_eq!(char_at(&state, 0, 0), '9');

    assert_eq!(state.undo_stack_len(), initial_undo_len + 1);
}

// ============================================================================
// Scroll Area Left Tests
// ============================================================================

#[test]
fn test_scroll_area_left_moves_content() {
    let mut state = create_test_state(10, 5);

    // Put distinct content in each column
    if let Some(layer) = state.get_cur_layer_mut() {
        for x in 0..10 {
            let ch = char::from_u32('0' as u32 + x as u32).unwrap();
            layer.set_char(Position::new(x, 0), AttributedChar::new(ch, TextAttribute::default()));
        }
    }

    let initial_undo_len = state.undo_stack_len();
    state.scroll_area_left().unwrap();

    // Column 0 should now have content from column 1
    assert_eq!(char_at(&state, 0, 0), '1');
    // Column 9 should have content from column 0 (wrapped)
    assert_eq!(char_at(&state, 9, 0), '0');

    assert_eq!(state.undo_stack_len(), initial_undo_len + 1);
}

// ============================================================================
// Scroll Area Right Tests
// ============================================================================

#[test]
fn test_scroll_area_right_moves_content() {
    let mut state = create_test_state(10, 5);

    // Put distinct content in each column
    if let Some(layer) = state.get_cur_layer_mut() {
        for x in 0..10 {
            let ch = char::from_u32('0' as u32 + x as u32).unwrap();
            layer.set_char(Position::new(x, 0), AttributedChar::new(ch, TextAttribute::default()));
        }
    }

    let initial_undo_len = state.undo_stack_len();
    state.scroll_area_right().unwrap();

    // Column 1 should now have content from column 0
    assert_eq!(char_at(&state, 1, 0), '0');
    // Column 0 should have content from column 9 (wrapped)
    assert_eq!(char_at(&state, 0, 0), '9');

    assert_eq!(state.undo_stack_len(), initial_undo_len + 1);
}

// ============================================================================
// Empty Area Tests (operations on empty areas)
// ============================================================================

#[test]
fn test_scroll_empty_area_does_nothing() {
    let mut state = create_test_state(0, 0);

    let initial_undo_len = state.undo_stack_len();

    // These should not panic on empty buffers
    let _ = state.scroll_area_up();
    let _ = state.scroll_area_down();
    let _ = state.scroll_area_left();
    let _ = state.scroll_area_right();

    // No undo operations should be pushed for empty operations
    assert_eq!(state.undo_stack_len(), initial_undo_len);
}
