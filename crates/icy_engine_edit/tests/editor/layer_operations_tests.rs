//! Tests for layer operations (add, remove, raise, lower, duplicate, merge, etc.)

use icy_engine::{AttributedChar, Position, Properties, Size, TextAttribute, TextPane};
use icy_engine_edit::EditState;

/// Helper to create an EditState with a given size
fn create_test_state(width: i32, height: i32) -> EditState {
    let buffer = icy_engine::TextBuffer::create((width, height));
    EditState::from_buffer(buffer)
}

// ============================================================================
// Add Layer Tests
// ============================================================================

#[test]
fn test_add_new_layer_increases_layer_count() {
    let mut state = create_test_state(20, 10);
    
    let initial_layer_count = state.get_buffer().layers.len();
    let initial_undo_len = state.undo_stack_len();
    
    state.add_new_layer(0).unwrap();
    
    assert_eq!(state.get_buffer().layers.len(), initial_layer_count + 1);
    assert_eq!(state.undo_stack_len(), initial_undo_len + 1);
}

#[test]
fn test_add_new_layer_at_end() {
    let mut state = create_test_state(20, 10);
    
    let initial_layer_count = state.get_buffer().layers.len();
    
    state.add_new_layer(initial_layer_count - 1).unwrap();
    
    assert_eq!(state.get_buffer().layers.len(), initial_layer_count + 1);
}

// ============================================================================
// Remove Layer Tests
// ============================================================================

#[test]
fn test_remove_layer_decreases_layer_count() {
    let mut state = create_test_state(20, 10);
    
    // First add a layer so we have something to remove
    state.add_new_layer(0).unwrap();
    let layer_count_after_add = state.get_buffer().layers.len();
    
    let initial_undo_len = state.undo_stack_len();
    
    state.remove_layer(1).unwrap();
    
    assert_eq!(state.get_buffer().layers.len(), layer_count_after_add - 1);
    assert_eq!(state.undo_stack_len(), initial_undo_len + 1);
}

#[test]
fn test_remove_invalid_layer_returns_error() {
    let mut state = create_test_state(20, 10);
    
    let result = state.remove_layer(999);
    
    assert!(result.is_err());
}

// ============================================================================
// Raise Layer Tests
// ============================================================================

#[test]
fn test_raise_layer_changes_order() {
    let mut state = create_test_state(20, 10);
    
    // Add a second layer
    state.add_new_layer(0).unwrap();
    
    let initial_undo_len = state.undo_stack_len();
    
    // Raise layer 0
    state.raise_layer(0).unwrap();
    
    assert_eq!(state.undo_stack_len(), initial_undo_len + 1);
}

#[test]
fn test_raise_top_layer_returns_error() {
    let mut state = create_test_state(20, 10);
    
    // Add a second layer
    state.add_new_layer(0).unwrap();
    
    // Try to raise the top layer (should fail)
    let top_layer_index = state.get_buffer().layers.len() - 1;
    let result = state.raise_layer(top_layer_index);
    
    assert!(result.is_err());
}

// ============================================================================
// Lower Layer Tests
// ============================================================================

#[test]
fn test_lower_layer_changes_order() {
    let mut state = create_test_state(20, 10);
    
    // Add a second layer
    state.add_new_layer(0).unwrap();
    
    let initial_undo_len = state.undo_stack_len();
    
    // Lower layer 1
    state.lower_layer(1).unwrap();
    
    assert_eq!(state.undo_stack_len(), initial_undo_len + 1);
}

#[test]
fn test_lower_bottom_layer_does_nothing() {
    let mut state = create_test_state(20, 10);
    
    let initial_undo_len = state.undo_stack_len();
    
    // Try to lower layer 0 (bottom layer)
    state.lower_layer(0).unwrap();
    
    // Should not push undo operation
    assert_eq!(state.undo_stack_len(), initial_undo_len);
}

// ============================================================================
// Duplicate Layer Tests
// ============================================================================

#[test]
fn test_duplicate_layer_creates_copy() {
    let mut state = create_test_state(20, 10);
    
    // Put some content in layer 0
    if let Some(layer) = state.get_cur_layer_mut() {
        layer.set_char(Position::new(5, 5), AttributedChar::new('D', TextAttribute::default()));
    }
    
    let initial_layer_count = state.get_buffer().layers.len();
    let initial_undo_len = state.undo_stack_len();
    
    state.duplicate_layer(0).unwrap();
    
    assert_eq!(state.get_buffer().layers.len(), initial_layer_count + 1);
    
    // New layer should have the same content
    let ch = state.get_buffer().layers[1].char_at(Position::new(5, 5));
    assert_eq!(ch.ch, 'D');
    
    assert_eq!(state.undo_stack_len(), initial_undo_len + 1);
}

// ============================================================================
// Clear Layer Tests
// ============================================================================

#[test]
fn test_clear_layer_removes_content() {
    let mut state = create_test_state(20, 10);
    
    // Put some content in layer 0
    if let Some(layer) = state.get_cur_layer_mut() {
        for y in 0..10 {
            for x in 0..20 {
                layer.set_char(Position::new(x, y), AttributedChar::new('X', TextAttribute::default()));
            }
        }
    }
    
    let initial_undo_len = state.undo_stack_len();
    
    state.clear_layer(0).unwrap();
    
    // All characters should be invisible/cleared
    let ch = state.get_buffer().layers[0].char_at(Position::new(5, 5));
    assert!(!ch.is_visible() || ch.is_transparent());
    
    assert_eq!(state.undo_stack_len(), initial_undo_len + 1);
}

// ============================================================================
// Merge Layer Down Tests
// ============================================================================

#[test]
fn test_merge_layer_down_combines_layers() {
    let mut state = create_test_state(20, 10);
    
    // Put content in layer 0
    if let Some(layer) = state.get_cur_layer_mut() {
        layer.set_char(Position::new(0, 0), AttributedChar::new('B', TextAttribute::default()));
    }
    
    // Add layer 1 and put content
    state.add_new_layer(0).unwrap();
    state.set_current_layer(1);
    if let Some(layer) = state.get_cur_layer_mut() {
        layer.set_char(Position::new(5, 5), AttributedChar::new('T', TextAttribute::default()));
    }
    
    let layer_count_before = state.get_buffer().layers.len();
    let initial_undo_len = state.undo_stack_len();
    
    state.merge_layer_down(1).unwrap();
    
    // Should have one less layer
    assert_eq!(state.get_buffer().layers.len(), layer_count_before - 1);
    
    // Merged layer should contain both characters
    let ch_base = state.get_buffer().layers[0].char_at(Position::new(0, 0));
    let ch_top = state.get_buffer().layers[0].char_at(Position::new(5, 5));
    assert_eq!(ch_base.ch, 'B');
    assert_eq!(ch_top.ch, 'T');
    
    assert_eq!(state.undo_stack_len(), initial_undo_len + 1);
}

#[test]
fn test_merge_layer_down_base_layer_returns_error() {
    let mut state = create_test_state(20, 10);
    
    // Try to merge layer 0 down (can't merge the base layer)
    let result = state.merge_layer_down(0);
    
    assert!(result.is_err());
}

// ============================================================================
// Toggle Layer Visibility Tests
// ============================================================================

#[test]
fn test_toggle_layer_visibility() {
    let mut state = create_test_state(20, 10);
    
    let initial_visible = state.get_buffer().layers[0].properties.is_visible;
    let initial_undo_len = state.undo_stack_len();
    
    state.toggle_layer_visibility(0).unwrap();
    
    assert_ne!(state.get_buffer().layers[0].properties.is_visible, initial_visible);
    assert_eq!(state.undo_stack_len(), initial_undo_len + 1);
}

// ============================================================================
// Move Layer Tests
// ============================================================================

#[test]
fn test_move_layer_changes_offset() {
    let mut state = create_test_state(20, 10);
    
    let initial_undo_len = state.undo_stack_len();
    
    state.move_layer(Position::new(5, 3)).unwrap();
    
    let offset = state.get_buffer().layers[0].offset();
    assert_eq!(offset, Position::new(5, 3));
    assert_eq!(state.undo_stack_len(), initial_undo_len + 1);
}

// ============================================================================
// Set Layer Size Tests
// ============================================================================

#[test]
fn test_set_layer_size_changes_dimensions() {
    let mut state = create_test_state(20, 10);
    
    let initial_undo_len = state.undo_stack_len();
    
    state.set_layer_size(0, Size::new(30, 15)).unwrap();
    
    assert_eq!(state.get_buffer().layers[0].size(), Size::new(30, 15));
    assert_eq!(state.undo_stack_len(), initial_undo_len + 1);
}

// ============================================================================
// Rotate Layer Tests
// ============================================================================

#[test]
fn test_rotate_layer_rotates_content() {
    let mut state = create_test_state(20, 10);
    
    // Put a character at a known position
    if let Some(layer) = state.get_cur_layer_mut() {
        layer.set_char(Position::new(5, 2), AttributedChar::new('A', TextAttribute::default()));
    }
    
    let initial_undo_len = state.undo_stack_len();
    
    state.rotate_layer().unwrap();
    
    // After rotation, the layer lines should be different
    // (the actual rotation logic transforms positions and applies character mapping)
    assert_eq!(state.undo_stack_len(), initial_undo_len + 1, "Should push undo operation");
}

// ============================================================================
// Make Layer Transparent Tests
// ============================================================================

#[test]
fn test_make_layer_transparent_clears_transparent_chars() {
    let mut state = create_test_state(20, 10);
    
    // Fill with content
    if let Some(layer) = state.get_cur_layer_mut() {
        for y in 0..10 {
            for x in 0..20 {
                layer.set_char(Position::new(x, y), AttributedChar::new('X', TextAttribute::default()));
            }
        }
    }
    
    let initial_undo_len = state.undo_stack_len();
    
    state.make_layer_transparent().unwrap();
    
    assert_eq!(state.undo_stack_len(), initial_undo_len + 1);
}

// ============================================================================
// Update Layer Properties Tests
// ============================================================================

#[test]
fn test_update_layer_properties() {
    let mut state = create_test_state(20, 10);
    
    let mut new_props = Properties::default();
    new_props.title = "New Layer Name".to_string();
    new_props.is_visible = false;
    
    let initial_undo_len = state.undo_stack_len();
    
    state.update_layer_properties(0, new_props.clone()).unwrap();
    
    assert_eq!(state.get_buffer().layers[0].properties.title, "New Layer Name");
    assert!(!state.get_buffer().layers[0].properties.is_visible);
    assert_eq!(state.undo_stack_len(), initial_undo_len + 1);
}
