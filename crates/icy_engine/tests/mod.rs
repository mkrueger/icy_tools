use icy_engine::{
    editor::{EditState, UndoState},
    Buffer,
};

#[test]
fn test_set_aspect_ratio() {
    let mut buffer = Buffer::new((80, 25));
    buffer.set_use_aspect_ratio(false);
    let mut edit_state = EditState::from_buffer(buffer);

    edit_state.set_use_aspect_ratio(true).unwrap();
    assert!(edit_state.get_buffer().use_aspect_ratio());
    edit_state.set_use_aspect_ratio(false).unwrap();
    assert!(!edit_state.get_buffer().use_aspect_ratio());
    edit_state.undo().unwrap();
    assert!(edit_state.get_buffer().use_aspect_ratio());
}

#[test]
fn test_set_letter_spacing() {
    let mut buffer = Buffer::new((80, 25));
    buffer.set_use_letter_spacing(false);
    let mut edit_state = EditState::from_buffer(buffer);

    edit_state.set_use_letter_spacing(true).unwrap();
    assert!(edit_state.get_buffer().use_letter_spacing());
    edit_state.set_use_letter_spacing(false).unwrap();
    assert!(!edit_state.get_buffer().use_letter_spacing());
    edit_state.undo().unwrap();
    assert!(edit_state.get_buffer().use_letter_spacing());
}
