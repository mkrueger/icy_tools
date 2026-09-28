use icy_engine::{AttributedChar, BitFont, FontMode, TextBuffer, TextPane};
use icy_engine_edit::{EditState, UndoState};

fn unrestricted_state() -> EditState {
    let mut buffer = TextBuffer::create((10, 5));
    buffer.font_mode = FontMode::Unlimited;
    EditState::from_buffer(buffer)
}

fn named_font(name: &str) -> BitFont {
    let mut font = BitFont::default();
    font.set_name(name);
    font
}

#[test]
fn applying_fonts_in_unrestricted_mode_preserves_previous_slots_and_undoes() {
    let mut state = unrestricted_state();
    let first = named_font("First");
    let second = named_font("Second");
    let base = state.get_buffer().font(0).unwrap().clone();
    let initial_undo = state.undo_stack_len();

    state.apply_font(first.clone()).unwrap();
    let first_page = state.get_caret().font_page();
    assert_ne!(first_page, 0);
    state.apply_font(second.clone()).unwrap();
    let second_page = state.get_caret().font_page();
    assert_ne!(second_page, first_page);
    assert_eq!(state.get_buffer().font(0), Some(&base));
    assert_eq!(state.get_buffer().font(first_page), Some(&first));
    assert_eq!(state.get_buffer().font(second_page), Some(&second));
    assert_eq!(state.undo_stack_len(), initial_undo + 2);

    state.undo().unwrap();
    assert_eq!(state.get_caret().font_page(), first_page);
    assert!(!state.get_buffer().has_font(second_page));
    state.redo().unwrap();
    assert_eq!(state.get_caret().font_page(), second_page);
    assert_eq!(state.get_buffer().font(second_page), Some(&second));

    state.apply_font(first.clone()).unwrap();
    assert_eq!(state.get_caret().font_page(), first_page);
    assert_eq!(state.get_buffer().font_count(), 3);
    state.undo().unwrap();
    assert_eq!(state.get_caret().font_page(), second_page);
    state.redo().unwrap();
    assert_eq!(state.get_caret().font_page(), first_page);
}

#[test]
fn selecting_a_predefined_font_does_not_add_or_replace_a_slot() {
    let mut state = unrestricted_state();
    let preset = state.get_buffer().font(42).unwrap().clone();
    let initial_undo = state.undo_stack_len();
    let initial_count = state.get_buffer().font_count();

    state.apply_font(preset.clone()).unwrap();
    assert_eq!(state.get_caret().font_page(), 42);
    assert_eq!(state.get_buffer().font_for_render(42), Some(&preset));
    assert_eq!(state.get_buffer().font_count(), initial_count + 1);
    assert_eq!(state.undo_stack_len(), initial_undo + 1);
    state.undo().unwrap();
    assert_eq!(state.get_caret().font_page(), 0);
    assert!(!state.get_buffer().has_font(42));
    assert_eq!(state.get_buffer().font_count(), initial_count);
    state.redo().unwrap();
    assert_eq!(state.get_caret().font_page(), 42);
    assert_eq!(state.get_buffer().font_for_render(42), Some(&preset));
}

#[test]
fn other_font_modes_do_not_install_predefined_slots_when_switching() {
    let mut state = unrestricted_state();
    state.get_buffer_mut().font_mode = FontMode::FixedSize;
    state.switch_to_font_page(1).unwrap();
    assert_eq!(state.get_caret().font_page(), 1);
    assert!(!state.get_buffer().has_font(1));
    state.undo().unwrap();
    assert_eq!(state.get_caret().font_page(), 0);
}

#[test]
fn adding_custom_fonts_never_occupies_predefined_slots() {
    let mut state = unrestricted_state();
    for page in 100..=u8::MAX {
        state.get_buffer_mut().set_font(page, named_font(&format!("Slot {page}")));
    }
    state.add_font(named_font("One more")).unwrap();
    assert_eq!(state.get_caret().font_page(), icy_engine::ANSI_SLOT_COUNT as u8);

    for page in icy_engine::ANSI_SLOT_COUNT as u8..100 {
        state.get_buffer_mut().set_font(page, named_font(&format!("Slot {page}")));
    }
    assert!(state.add_font(named_font("No more")).is_err());
    assert!(state.get_buffer().font(42).is_some());
    assert!(!state.get_buffer().has_font(42));
}

#[test]
fn applying_font_reports_full_font_table_without_overwriting() {
    let mut state = unrestricted_state();
    for slot in 0..=u8::MAX {
        state.get_buffer_mut().set_font(slot, named_font(&format!("Slot {slot}")));
    }
    let original = state.get_buffer().font(100).unwrap().clone();
    let initial_undo = state.undo_stack_len();
    assert!(state.apply_font(named_font("New")).is_err());
    assert_eq!(state.get_buffer().font(100), Some(&original));
    assert_eq!(state.get_caret().font_page(), 0);
    assert_eq!(state.undo_stack_len(), initial_undo);
}

#[test]
fn setting_a_new_font_slot_is_undoable_without_switching_the_caret() {
    let mut state = unrestricted_state();
    let font = named_font("In slot 42");
    let initial_undo = state.undo_stack_len();
    state.set_font_in_slot(42, font.clone()).unwrap();
    assert_eq!(state.get_buffer().font(42), Some(&font));
    assert_eq!(state.get_caret().font_page(), 0);
    assert_eq!(state.undo_stack_len(), initial_undo + 1);
    state.undo().unwrap();
    assert!(!state.get_buffer().has_font(42));
    assert_eq!(state.get_caret().font_page(), 0);
    state.redo().unwrap();
    assert_eq!(state.get_buffer().font(42), Some(&font));
    assert_eq!(state.get_caret().font_page(), 0);
}

#[test]
fn adding_a_font_does_not_repurpose_slots_already_used_by_artwork() {
    let mut state = unrestricted_state();
    let mut ch = AttributedChar::from_char('A');
    ch.set_font_page(100);
    state.get_cur_layer_mut().unwrap().set_char((0, 0), ch);

    state.add_font(named_font("New")).unwrap();
    assert_eq!(state.get_caret().font_page(), 101);
    assert_eq!(state.get_cur_layer().unwrap().char_at((0, 0).into()).font_page(), 100);
}

#[test]
fn replacing_current_font_undoes_to_the_correct_slot_font() {
    let mut state = unrestricted_state();
    let original = named_font("Original");
    state.set_font_in_slot(42, original.clone()).unwrap();
    state.switch_to_font_page(42).unwrap();
    let replacement = named_font("Replacement");
    state.set_font(replacement.clone()).unwrap();
    assert_eq!(state.get_buffer().font(42), Some(&replacement));
    state.undo().unwrap();
    assert_eq!(state.get_buffer().font(42), Some(&original));
    state.redo().unwrap();
    assert_eq!(state.get_buffer().font(42), Some(&replacement));
}
