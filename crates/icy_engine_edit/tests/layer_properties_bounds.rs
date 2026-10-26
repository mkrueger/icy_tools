use icy_engine_edit::{EditState, UndoState};

#[test]
fn invalid_layer_properties_preserve_buffer_and_redo() {
    for locked in [false, true] {
        for index in [1, usize::MAX] {
            let mut state = EditState::default();
            let original = state.get_buffer().layers[0].properties.clone();
            let mut changed = original.clone();
            changed.title = "changed".into();
            state.update_layer_properties(0, changed).unwrap();
            state.undo().unwrap();
            assert!(state.can_redo());

            let mut invalid = original.clone();
            invalid.is_locked = locked;
            assert!(state.update_layer_properties(index, invalid).is_err());
            assert_eq!(state.get_buffer().layers.len(), 1);
            assert_eq!(state.get_buffer().layers[0].properties, original);
            assert_eq!(state.undo_stack_len(), 0);
            assert!(state.can_redo());
            state.redo().unwrap();
            assert_eq!(state.get_buffer().layers[0].properties.title, "changed");
        }
    }
}

#[test]
fn missing_layer_properties_return_error_for_empty_buffer() {
    for locked in [false, true] {
        let mut state = EditState::default();
        let mut properties = state.get_buffer().layers[0].properties.clone();
        properties.is_locked = locked;
        state.get_buffer_mut().layers.clear();
        assert!(state.update_layer_properties(0, properties).is_err());
        assert!(state.get_buffer().layers.is_empty());
        assert!(!state.can_undo());
        assert!(!state.can_redo());
    }
}

#[test]
fn valid_layer_property_locking_remains_undoable() {
    let mut state = EditState::default();
    let original = state.get_buffer().layers[0].properties.clone();
    let mut changed = original.clone();
    changed.title = "locked".into();
    changed.is_locked = true;
    state.update_layer_properties(0, changed.clone()).unwrap();
    assert_eq!(state.get_buffer().layers[0].properties, changed);
    state.undo().unwrap();
    assert_eq!(state.get_buffer().layers[0].properties, original);
    state.redo().unwrap();
    assert_eq!(state.get_buffer().layers[0].properties, changed);
}
