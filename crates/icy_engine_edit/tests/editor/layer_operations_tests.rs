//! Tests for layer operations (add, remove, raise, lower, duplicate, merge, etc.)

use icy_engine::{AttributedChar, LayerProperties, Position, Role, Size, Sixel, TextAttribute, TextPane};
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
fn test_merge_layer_down_respects_layer_offsets() {
    let mut state = create_test_state(20, 10);

    // Move the base layer so merging must translate coordinates.
    state.get_buffer_mut().layers[0].set_offset(Position::new(15, 0));
    state
        .get_buffer_mut()
        .layers
        .get_mut(0)
        .unwrap()
        .set_char(Position::new(0, 0), AttributedChar::new('B', TextAttribute::default()));

    // Add a top layer and offset it relative to the base.
    state.add_new_layer(0).unwrap();
    state
        .get_buffer_mut()
        .layers
        .get_mut(1)
        .unwrap()
        .set_offset(Position::new(17, 0));
    state
        .get_buffer_mut()
        .layers
        .get_mut(1)
        .unwrap()
        .set_char(Position::new(0, 0), AttributedChar::new('T', TextAttribute::default()));

    state.merge_layer_down(1).unwrap();

    assert_eq!(state.get_buffer().layers.len(), 1);

    let merged = &state.get_buffer().layers[0];
    assert_eq!(merged.offset(), Position::new(15, 0));

    // Base layer char stays at its local origin.
    assert_eq!(merged.char_at(Position::new(0, 0)).ch, 'B');

    // Top layer (doc pos 17,0) ends up at merged local x=2.
    assert_eq!(merged.char_at(Position::new(2, 0)).ch, 'T');
}

#[test]
fn test_merge_layer_down_works_when_base_layer_locked() {
    let mut state = create_test_state(20, 10);

    // Base content.
    state
        .get_buffer_mut()
        .layers
        .get_mut(0)
        .unwrap()
        .set_char(Position::new(1, 1), AttributedChar::new('B', TextAttribute::default()));

    // Lock the base layer after drawing on it.
    state.get_buffer_mut().layers[0].properties.is_locked = true;

    // Add a second layer with content.
    state.add_new_layer(0).unwrap();
    state
        .get_buffer_mut()
        .layers
        .get_mut(1)
        .unwrap()
        .set_char(Position::new(2, 1), AttributedChar::new('T', TextAttribute::default()));

    state.merge_layer_down(1).unwrap();

    assert_eq!(state.get_buffer().layers.len(), 1);
    let merged = &state.get_buffer().layers[0];
    assert_eq!(merged.char_at(Position::new(1, 1)).ch, 'B');
    assert_eq!(merged.char_at(Position::new(2, 1)).ch, 'T');
}

// ============================================================================
// Stamp Layer Down Tests
// ============================================================================

#[test]
fn test_stamp_layer_down_respects_base_layer_offset() {
    let mut state = create_test_state(20, 10);

    // Move the base layer so stamping must translate coordinates.
    state.get_buffer_mut().layers[0].set_offset(Position::new(15, 0));

    // Add a top layer (the paste/floating layer) and position it in document space.
    state.add_new_layer(0).unwrap();
    state.set_current_layer(1);
    state
        .get_buffer_mut()
        .layers
        .get_mut(1)
        .unwrap()
        .set_offset(Position::new(17, 0));

    // Put a visible character at the top-left of the source layer.
    state
        .get_buffer_mut()
        .layers
        .get_mut(1)
        .unwrap()
        .set_char(Position::new(0, 0), AttributedChar::new('S', TextAttribute::default()));

    // Stamp onto the layer below (index 0). Expected destination local position:
    // dest = src_local + (src_offset - base_offset) = (0,0) + (17,0) - (15,0) = (2,0)
    state.stamp_layer_down().unwrap();

    let stamped = state.get_buffer().layers[0].char_at(Position::new(2, 0));
    assert_eq!(stamped.ch, 'S');
}

#[test]
fn test_merge_layer_down_base_layer_returns_error() {
    let mut state = create_test_state(20, 10);

    // Try to merge layer 0 down (can't merge the base layer)
    let result = state.merge_layer_down(0);

    assert!(result.is_err());
}

#[test]
fn test_merge_layer_down_image_layer_returns_error() {
    let mut state = create_test_state(20, 10);

    state.add_new_layer(0).unwrap();
    state.get_buffer_mut().layers[1].role = Role::Image;

    let result = state.merge_layer_down(1);
    assert!(result.is_err());
}

#[test]
fn test_anchor_layer_image_layer_returns_error() {
    let mut state = create_test_state(20, 10);

    // Simulate a floating/paste layer being an image layer.
    state.add_new_layer(0).unwrap();
    state.set_current_layer(1);
    state.get_buffer_mut().layers[1].role = Role::Image;

    let result = state.anchor_layer();
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
// Paste Rotate Tests
// ============================================================================

#[test]
fn test_paste_rotate_rotates_content() {
    let mut state = create_test_state(20, 10);

    // Put a character at a known position
    if let Some(layer) = state.get_cur_layer_mut() {
        layer.set_char(Position::new(5, 2), AttributedChar::new('A', TextAttribute::default()));
    }

    let initial_undo_len = state.undo_stack_len();

    state.paste_rotate().unwrap();

    // After rotation, the layer lines should be different
    // (the actual rotation logic transforms positions and applies character mapping)
    assert_eq!(state.undo_stack_len(), initial_undo_len + 1, "Should push undo operation");
}

/// Test that paste_rotate swaps the layer dimensions (width and height).
#[test]
fn test_paste_rotate_swaps_dimensions() {
    // Create a non-square layer (20x10) to verify dimensions swap
    let mut state = create_test_state(20, 10);

    let original_size = state.get_cur_layer().unwrap().size();
    assert_eq!(original_size, Size::new(20, 10), "Initial size should be 20x10");

    state.paste_rotate().unwrap();

    let rotated_size = state.get_cur_layer().unwrap().size();
    assert_eq!(rotated_size, Size::new(10, 20), "After rotation, size should be 10x20");
}

/// Test that paste_rotate marks the buffer as dirty for render cache invalidation.
#[test]
fn test_paste_rotate_marks_buffer_dirty() {
    let mut state = create_test_state(20, 10);

    // Clear any initial dirty state
    state.get_buffer().clear_dirty();

    // Verify buffer is not dirty
    assert!(state.get_buffer().get_dirty_lines().is_none(), "Buffer should start clean");

    state.paste_rotate().unwrap();

    // Verify buffer is marked dirty after rotation
    assert!(state.get_buffer().get_dirty_lines().is_some(), "Buffer should be marked dirty after rotate");
}

#[test]
fn test_paste_rotate_rotates_sixel_image_layer() {
    let mut state = create_test_state(20, 10);

    // Ensure font dimensions are non-zero (used for sixel rectangle computations).
    state.get_buffer_mut().set_font_dimensions((8, 16).into());

    // Build a non-square RGBA image so rotate changes dimensions.
    let w = 16;
    let h = 48;
    let mut rgba = vec![0u8; (w * h * 4) as usize];
    // Top-left pixel = red.
    rgba[0] = 255;
    rgba[3] = 255;

    let mut sixel = Sixel::from_data((w, h), 1, 1, rgba);
    sixel.position = Position::new(0, 0);

    // Configure current layer as an image layer with the expected cell footprint.
    {
        let layer = state.get_cur_layer_mut().unwrap();
        layer.role = Role::Image;
        layer.sixels.clear();
        layer.sixels.push(sixel);
        layer.set_size(Size::new(2, 3));
    }

    let initial_undo_len = state.undo_stack_len();

    state.paste_rotate().unwrap();

    assert_eq!(state.undo_stack_len(), initial_undo_len + 1, "Should push undo operation");
    assert_eq!(state.get_cur_layer().unwrap().size(), Size::new(3, 2), "Layer size should swap");

    let layer = state.get_cur_layer().unwrap();
    assert_eq!(layer.sixels.len(), 1);
    let rotated = &layer.sixels[0];
    assert_eq!(rotated.width(), h);
    assert_eq!(rotated.height(), w);

    // After CW rotation, the old (0,0) pixel should end up at (h-1, 0).
    let idx = ((0 * h + (h - 1)) * 4) as usize;
    assert_eq!(rotated.picture_data[idx], 255);
    assert_eq!(rotated.picture_data[idx + 3], 255);
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

    let mut new_props = LayerProperties::default();
    new_props.title = "New Layer Name".to_string();
    new_props.is_visible = false;

    let initial_undo_len = state.undo_stack_len();

    state.update_layer_properties(0, new_props.clone()).unwrap();

    assert_eq!(state.get_buffer().layers[0].properties.title, "New Layer Name");
    assert!(!state.get_buffer().layers[0].properties.is_visible);
    assert_eq!(state.undo_stack_len(), initial_undo_len + 1);
}

// ============================================================================
// Paste Cancel Tests
// ============================================================================

/// Test that discard_and_undo properly reverts a paste operation by removing the pasted layer.
#[test]
fn test_discard_and_undo_removes_pasted_layer() {
    let mut state = create_test_state(20, 10);

    // Initial state: 1 layer
    let initial_layer_count = state.get_buffer().layers.len();
    assert_eq!(initial_layer_count, 1);

    // Simulate paste: add a layer manually and use discard_and_undo
    {
        let mut guard = state.begin_atomic_undo("Paste");

        // Use paste_text which uses the public API
        state.paste_text("Test").unwrap();

        // Verify paste layer was added
        assert_eq!(state.get_buffer().layers.len(), 2, "Paste should add a layer");
        assert_eq!(state.get_current_layer().unwrap(), 1, "Current layer should be the pasted one");

        // Use discard_and_undo to properly revert all operations
        guard.discard_and_undo(&mut state);
    }

    // After discard_and_undo: the pasted layer should be removed
    assert_eq!(
        state.get_buffer().layers.len(),
        initial_layer_count,
        "Pasted layer should be removed after discard_and_undo"
    );
}

/// Test that discard_and_undo restores the current layer index after paste cancel.
#[test]
fn test_discard_and_undo_restores_current_layer() {
    let mut state = create_test_state(20, 10);

    // Start on layer 0
    state.set_current_layer(0);
    let initial_layer = state.get_current_layer().unwrap();
    assert_eq!(initial_layer, 0);

    // Simulate paste and discard
    {
        let mut guard = state.begin_atomic_undo("Paste");

        // Use paste_text which uses the public API
        state.paste_text("Test").unwrap();

        // Verify we're on the new layer
        assert_eq!(state.get_current_layer().unwrap(), 1);

        // Use discard_and_undo
        guard.discard_and_undo(&mut state);
    }

    // Current layer should be restored to 0
    assert_eq!(
        state.get_current_layer().unwrap(),
        initial_layer,
        "Current layer should be restored after discard_and_undo"
    );
}

/// Test that discard_and_undo reverts multiple operations (paste + move).
#[test]
fn test_discard_and_undo_reverts_multiple_operations() {
    let mut state = create_test_state(20, 10);

    let initial_layer_count = state.get_buffer().layers.len();

    {
        let mut guard = state.begin_atomic_undo("Paste");

        // Paste a layer using public API
        state.paste_text("Test").unwrap();

        // Move the pasted layer
        state.move_layer(Position::new(10, 5)).unwrap();

        // Verify the layer is at the new position
        assert_eq!(state.get_buffer().layers[1].offset(), Position::new(10, 5));

        // Discard and undo all operations
        guard.discard_and_undo(&mut state);
    }

    // Everything should be reverted
    assert_eq!(state.get_buffer().layers.len(), initial_layer_count, "All operations should be reverted");
}
