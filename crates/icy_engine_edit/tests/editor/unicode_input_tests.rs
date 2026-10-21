use icy_engine::{AttributedChar, Position, Rectangle, Screen, TextAttribute, TextPane, TextScreen};
use icy_engine_edit::{EditState, UndoState};

fn editor(width: i32, height: i32) -> EditState {
    let mut screen = TextScreen::new((width, height));
    screen.set_unicode_width(true);
    EditState::from_buffer(screen.buffer)
}

#[test]
fn unicode_flips_keep_grapheme_text_width_and_undo() {
    let mut state = editor(8, 2);
    state.type_unicode_text("a界e\u{301}").unwrap();
    state.flip_x().unwrap();
    assert_eq!(glyph(&state, 7, 0), Some(("a", 1)));
    assert_eq!(glyph(&state, 5, 0), Some(("界", 2)));
    assert_eq!(glyph(&state, 4, 0), Some(("e\u{301}", 1)));
    state.flip_y().unwrap();
    assert_eq!(glyph(&state, 5, 1), Some(("界", 2)));
    state.undo().unwrap();
    assert_eq!(glyph(&state, 5, 0), Some(("界", 2)));
    state.undo().unwrap();
    assert_eq!(glyph(&state, 1, 0), Some(("界", 2)));
    state.redo().unwrap();
    state.redo().unwrap();
    assert_eq!(glyph(&state, 5, 1), Some(("界", 2)));
}

#[test]
fn unicode_flip_rejects_a_selection_cutting_a_grapheme_before_mutation() {
    let mut state = editor(8, 2);
    state.type_unicode_text("a界e\u{301}").unwrap();
    state.set_selection(Rectangle::from(2, 0, 1, 1)).unwrap();
    assert!(state.flip_x().is_err());
    assert_eq!(glyph(&state, 1, 0), Some(("界", 2)));
    let rectangle = state.selection().unwrap().as_rectangle();
    assert_eq!((rectangle.start, rectangle.size()), (Position::new(2, 0), icy_engine::Size::new(1, 1)));
}

fn glyph(state: &EditState, x: i32, y: i32) -> Option<(&str, usize)> {
    state.get_buffer().layers[0].grapheme_at(Position::new(x, y))
}

fn cell(state: &EditState, x: i32, y: i32) -> char {
    state.get_buffer().layers[0].char_at(Position::new(x, y)).ch
}

#[test]
fn unicode_clipboard_bounds_expand_a_continuation_on_a_later_row_in_document_coordinates() {
    let mut state = editor(8, 3);
    state.type_unicode_text("ab\n界").unwrap();
    state.get_buffer_mut().layers[0].set_offset(Position::new(4, 2));
    state.set_selection(Rectangle::from(5, 2, 1, 2)).unwrap();
    let bounds = state.unicode_clipboard_bounds().unwrap().unwrap();
    assert_eq!(bounds.start, Position::new(4, 2));
    assert_eq!(bounds.size(), icy_engine::Size::new(2, 2));
    let data = state.unicode_clipboard_data().unwrap().unwrap();
    let decoded = icy_engine::FileFormat::IcyDraw.from_bytes(&data, None).unwrap();
    assert_eq!(decoded.screen.buffer.size(), bounds.size());
    assert_eq!(decoded.screen.buffer.layers[0].grapheme_at(Position::new(0, 1)), Some(("界", 2)));
    assert_eq!(decoded.screen.buffer.layers[0].grapheme_at(Position::new(1, 0)), Some(("b", 1)));
}

#[test]
fn unicode_rich_clipboard_preserves_colors_graphemes_and_disjoint_selection_holes() {
    let mut source = editor(8, 2);
    let mut color = TextAttribute::default();
    color.set_foreground_color(icy_engine::AttributeColor::Rgb(13, 27, 91));
    color.set_background(4);
    source.set_caret_attribute(color);
    source.type_unicode_text("e\u{301}x界").unwrap();
    source.set_selection(Rectangle::from(0, 0, 1, 2)).unwrap();
    source.add_selection_to_mask().unwrap();
    source.set_selection(Rectangle::from(3, 0, 1, 1)).unwrap();
    let bytes = source.unicode_clipboard_data().unwrap().expect("Unicode selection has rich clipboard data");
    assert_eq!(source.clipboard_data().unwrap(), bytes);
    assert!(bytes.starts_with(b"\x89PNG\r\n\x1a\n"));
    let mut target = editor(8, 2);
    target.paste_clipboard_data(&bytes).unwrap();
    let layer = target.get_cur_layer().unwrap();
    assert_eq!(layer.grapheme_at(Position::new(0, 0)), Some(("e\u{301}", 1)));
    assert_eq!(layer.grapheme_at(Position::new(2, 0)), Some(("界", 2)));
    assert!(!layer.char_at(Position::new(1, 0)).is_visible());
    assert_eq!(layer.char_at(Position::new(0, 0)).attribute, color);
    assert_eq!(layer.char_at(Position::new(2, 0)).attribute, color);
    target.anchor_layer().unwrap();
    assert_eq!(glyph(&target, 2, 0), Some(("界", 2)));
    target.undo().unwrap();
    target.undo().unwrap();
    assert_eq!(target.get_buffer().layers.len(), 1);
    assert_eq!(glyph(&target, 0, 0), None);
    target.redo().unwrap();
    target.redo().unwrap();
    assert_eq!(glyph(&target, 0, 0), Some(("e\u{301}", 1)));
}

#[test]
fn unicode_rich_selection_cut_and_float_works_after_selection_is_erased() {
    let mut state = editor(8, 2);
    state.type_unicode_text("a界e\u{301}").unwrap();
    state.set_selection(Rectangle::from(1, 0, 3, 1)).unwrap();
    let bytes = state.unicode_clipboard_data().unwrap().unwrap();
    state.set_caret_position(Position::new(1, 0));
    {
        let _undo = state.begin_atomic_undo("Move Unicode block");
        state.erase_selection().unwrap();
        state.clear_selection().unwrap();
        state.paste_clipboard_data(&bytes).unwrap();
        state.move_layer(Position::new(3, 1)).unwrap();
        state.anchor_layer().unwrap();
    }
    assert_eq!(glyph(&state, 1, 0), None);
    assert_eq!(glyph(&state, 3, 1), Some(("界", 2)));
    assert_eq!(glyph(&state, 5, 1), Some(("e\u{301}", 1)));
    state.undo().unwrap();
    assert_eq!(glyph(&state, 1, 0), Some(("界", 2)));
    assert_eq!(glyph(&state, 3, 0), Some(("e\u{301}", 1)));
    assert!(state.selection().is_some());
    state.redo().unwrap();
    assert_eq!(glyph(&state, 3, 1), Some(("界", 2)));
}

#[test]
fn unicode_scalar_input_combines_and_undo_restores_metadata_and_caret() {
    let mut state = editor(12, 2);
    state.type_key('e').unwrap();
    state.type_key('\u{301}').unwrap();
    assert_eq!(glyph(&state, 0, 0), Some(("e\u{301}", 1)));
    assert_eq!(state.get_caret().position(), Position::new(1, 0));
    state.undo().unwrap();
    assert_eq!(glyph(&state, 0, 0), Some(("e", 1)));
    state.redo().unwrap();
    assert_eq!(glyph(&state, 0, 0), Some(("e\u{301}", 1)));
    assert_eq!(state.get_caret().position(), Position::new(1, 0));
}

#[test]
fn unicode_emoji_sequences_keep_two_cell_width() {
    for text in ["👩\u{200d}🚀", "👍🏽", "🇩🇪", "❤️"] {
        let mut state = editor(12, 2);
        for ch in text.chars() {
            state.type_key(ch).unwrap();
        }
        assert_eq!(glyph(&state, 0, 0), Some((text, 2)), "{text}");
        assert!(state.get_buffer().layers[0].is_grapheme_continuation(Position::new(1, 0)));
        assert_eq!(state.get_caret().position(), Position::new(2, 0));
    }
}

#[test]
fn unicode_insert_shifts_complete_graphemes_and_undo_restores_clipped_right_edge() {
    let mut state = editor(5, 1);
    state.type_unicode_text("a界語").unwrap();
    state.set_caret_position(Position::new(1, 0));
    state.toggle_insert_mode();
    state.type_key('x').unwrap();
    assert_eq!(glyph(&state, 1, 0), Some(("x", 1)));
    assert_eq!(glyph(&state, 2, 0), Some(("界", 2)));
    assert_eq!(glyph(&state, 4, 0), None);
    state.undo().unwrap();
    assert_eq!(glyph(&state, 1, 0), Some(("界", 2)));
    assert_eq!(glyph(&state, 3, 0), Some(("語", 2)));
    assert_eq!(state.get_caret().position(), Position::new(1, 0));
    state.redo().unwrap();
    assert_eq!(glyph(&state, 2, 0), Some(("界", 2)));
}

#[test]
fn unicode_overwrite_on_continuation_replaces_whole_grapheme() {
    let mut state = editor(8, 1);
    state.type_unicode_text("界z").unwrap();
    state.set_caret_position(Position::new(1, 0));
    state.type_key('x').unwrap();
    assert_eq!(glyph(&state, 0, 0), Some(("x", 1)));
    assert_eq!(cell(&state, 1, 0), ' ');
    assert_eq!(glyph(&state, 2, 0), Some(("z", 1)));
    assert_eq!(state.get_caret().position(), Position::new(1, 0));
    state.undo().unwrap();
    assert_eq!(glyph(&state, 0, 0), Some(("界", 2)));
}

#[test]
fn unicode_backspace_clears_both_cells_or_shifts_in_insert_mode() {
    for insert in [false, true] {
        let mut state = editor(8, 1);
        state.type_unicode_text("界z").unwrap();
        state.set_caret_position(Position::new(2, 0));
        if insert {
            state.toggle_insert_mode();
        }
        state.backspace().unwrap();
        assert_eq!(state.get_caret().position(), Position::new(0, 0));
        assert_eq!(glyph(&state, 0, 0), if insert { Some(("z", 1)) } else { None });
        assert!(!state.get_buffer().layers[0].is_grapheme_continuation(Position::new(1, 0)));
        state.undo().unwrap();
        assert_eq!(glyph(&state, 0, 0), Some(("界", 2)));
        assert_eq!(state.get_caret().position(), Position::new(2, 0));
        state.redo().unwrap();
        assert_eq!(state.get_caret().position(), Position::new(0, 0));
    }
}

#[test]
fn unicode_delete_from_continuation_shifts_one_whole_grapheme() {
    let mut state = editor(8, 1);
    state.type_unicode_text("界語x").unwrap();
    state.set_caret_position(Position::new(1, 0));
    state.delete_key().unwrap();
    assert_eq!(glyph(&state, 0, 0), Some(("語", 2)));
    assert_eq!(glyph(&state, 2, 0), Some(("x", 1)));
    assert_eq!(state.get_caret().position(), Position::new(0, 0));
    state.undo().unwrap();
    assert_eq!(glyph(&state, 0, 0), Some(("界", 2)));
    assert_eq!(glyph(&state, 2, 0), Some(("語", 2)));
}

#[test]
fn unicode_right_margin_never_splits_a_wide_grapheme() {
    let mut state = editor(3, 2);
    state.type_unicode_text("ab界").unwrap();
    assert_eq!(glyph(&state, 0, 1), Some(("界", 2)));
    assert_eq!(cell(&state, 2, 0), ' ');
    state.type_key('x').unwrap();
    assert_eq!(state.get_caret().position(), Position::new(3, 1));
    state.type_key('y').unwrap();
    assert_eq!(state.get_caret().position(), Position::new(3, 1));
    state.backspace().unwrap();
    assert_eq!(state.get_caret().position(), Position::new(2, 1));
}

#[test]
fn unicode_paste_is_lossless_and_one_undo_step() {
    let mut state = editor(12, 3);
    state.type_unicode_text("e\u{301}界\r\n👩\u{200d}🚀").unwrap();
    assert_eq!(glyph(&state, 0, 0), Some(("e\u{301}", 1)));
    assert_eq!(glyph(&state, 1, 0), Some(("界", 2)));
    assert_eq!(glyph(&state, 0, 1), Some(("👩\u{200d}🚀", 2)));
    state.undo().unwrap();
    assert_eq!(glyph(&state, 0, 0), None);
    assert_eq!(glyph(&state, 0, 1), None);
    assert_eq!(state.get_caret().position(), Position::new(0, 0));
    state.redo().unwrap();
    assert_eq!(glyph(&state, 0, 1), Some(("👩\u{200d}🚀", 2)));
}

#[test]
fn unicode_selected_continuation_copies_and_erases_whole_glyph_with_undo() {
    let mut state = editor(8, 1);
    state.type_unicode_text("e\u{301}界").unwrap();
    state.set_selection(Rectangle::from(2, 0, 1, 1)).unwrap();
    assert_eq!(state.copy_text().as_deref(), Some("界\n"));
    state.erase_selection().unwrap();
    assert_eq!(glyph(&state, 1, 0), None);
    assert_eq!(cell(&state, 1, 0), ' ');
    state.undo().unwrap();
    assert_eq!(glyph(&state, 1, 0), Some(("界", 2)));
}

#[test]
fn unicode_brush_cell_writes_keep_metadata_through_undo() {
    let mut state = editor(8, 1);
    state.type_unicode_text("e\u{301}").unwrap();
    let mut attribute = TextAttribute::default();
    attribute.set_foreground(2);
    state.set_char(Position::new(0, 0), AttributedChar::new('e', attribute)).unwrap();
    assert_eq!(glyph(&state, 0, 0), Some(("e\u{301}", 1)));
    state.set_char(Position::new(0, 0), AttributedChar::new('界', attribute)).unwrap();
    assert_eq!(glyph(&state, 0, 0), Some(("界", 2)));
    state.undo().unwrap();
    assert_eq!(glyph(&state, 0, 0), Some(("e\u{301}", 1)));
}

#[test]
fn unicode_navigation_snaps_to_both_sides_of_wide_cells() {
    let mut state = editor(8, 1);
    state.type_key('界').unwrap();
    assert_eq!(state.unicode_caret_position(Position::new(1, 0), false), Position::new(0, 0));
    assert_eq!(state.unicode_caret_position(Position::new(1, 0), true), Position::new(2, 0));
    state.set_caret_position(Position::new(0, 0));
    state.move_caret_right(1);
    assert_eq!(state.get_caret().position(), Position::new(2, 0));
    state.move_caret_left(1);
    assert_eq!(state.get_caret().position(), Position::new(0, 0));
    state.set_caret_position(Position::new(1, 0));
    assert_eq!(state.get_caret().position(), Position::new(0, 0));
}

#[test]
fn unicode_insert_extending_an_emoji_shifts_only_the_additional_cell() {
    let mut state = editor(8, 1);
    state.type_unicode_text("❤z").unwrap();
    state.set_caret_position(Position::new(1, 0));
    state.toggle_insert_mode();
    state.type_key('\u{fe0f}').unwrap();
    assert_eq!(glyph(&state, 0, 0), Some(("❤️", 2)));
    assert_eq!(glyph(&state, 2, 0), Some(("z", 1)));
    assert_eq!(state.get_caret().position(), Position::new(2, 0));
    state.undo().unwrap();
    assert_eq!(glyph(&state, 0, 0), Some(("❤", 1)));
    assert_eq!(glyph(&state, 1, 0), Some(("z", 1)));
}

#[test]
fn unicode_paste_buffer_preserves_grapheme_metadata() {
    let mut source = editor(8, 1);
    source.type_unicode_text("e\u{301}界").unwrap();
    let mut target = editor(8, 1);
    target.paste_buffer(source.get_buffer(), "Unicode paste".into()).unwrap();
    let layer = target.get_cur_layer().unwrap();
    assert_eq!(layer.grapheme_at(Position::new(0, 0)), Some(("e\u{301}", 1)));
    assert_eq!(layer.grapheme_at(Position::new(1, 0)), Some(("界", 2)));
    target.anchor_layer().unwrap();
    assert_eq!(target.get_cur_layer().unwrap().grapheme_at(Position::new(1, 0)), Some(("界", 2)));
    target.undo().unwrap();
    assert_eq!(target.get_buffer().layers.len(), 2);
    assert_eq!(target.get_cur_layer().unwrap().grapheme_at(Position::new(1, 0)), Some(("界", 2)));
    target.redo().unwrap();
    assert_eq!(target.get_buffer().layers.len(), 1);
    assert_eq!(target.get_cur_layer().unwrap().grapheme_at(Position::new(1, 0)), Some(("界", 2)));
}

#[test]
fn unicode_canvas_resize_clips_complete_glyphs_and_repeated_undo_keeps_snapshots() {
    let mut state = editor(8, 2);
    state.type_unicode_text("a界e\u{301}語").unwrap();
    state.resize_buffer(true, (2, 1)).unwrap();
    assert_eq!(state.get_buffer().size(), icy_engine::Size::new(2, 1));
    assert_eq!(state.get_cur_layer().unwrap().size(), icy_engine::Size::new(2, 1));
    assert_eq!(glyph(&state, 1, 0), None);
    assert_eq!(cell(&state, 1, 0), ' ');
    for _ in 0..3 {
        state.undo().unwrap();
        assert_eq!(state.get_buffer().size(), icy_engine::Size::new(8, 2));
        assert_eq!(glyph(&state, 1, 0), Some(("界", 2)));
        assert_eq!(glyph(&state, 3, 0), Some(("e\u{301}", 1)));
        assert_eq!(glyph(&state, 4, 0), Some(("語", 2)));
        state.redo().unwrap();
        assert_eq!(state.get_cur_layer().unwrap().size(), icy_engine::Size::new(2, 1));
        assert_eq!(glyph(&state, 1, 0), None);
    }
    state.resize_buffer(true, (8, 2)).unwrap();
    assert_eq!(glyph(&state, 1, 0), None);
    assert_eq!(cell(&state, 3, 0), ' ');
}

#[test]
fn unicode_canvas_only_resize_preserves_layer_extent_and_hidden_graphemes() {
    let mut state = editor(8, 2);
    state.type_unicode_text("界語").unwrap();
    state.resize_buffer(false, (1, 1)).unwrap();
    assert_eq!(state.get_buffer().width(), 1);
    assert_eq!(state.get_cur_layer().unwrap().width(), 8);
    assert_eq!(glyph(&state, 0, 0), Some(("界", 2)));
    assert_eq!(glyph(&state, 2, 0), Some(("語", 2)));
    state.undo().unwrap();
    state.redo().unwrap();
    state.undo().unwrap();
    assert_eq!(glyph(&state, 0, 0), Some(("界", 2)));
    assert_eq!(glyph(&state, 2, 0), Some(("語", 2)));
}

#[test]
fn unicode_canvas_and_layer_resize_preserves_offset_layers() {
    let mut state = editor(8, 3);
    state.paste_text("a界").unwrap();
    state.get_buffer_mut().layers[1].set_offset(Position::new(2, 1));
    state.resize_buffer(true, (4, 2)).unwrap();
    let layer = &state.get_buffer().layers[1];
    assert_eq!(layer.offset(), Position::new(2, 1));
    assert_eq!(layer.size(), icy_engine::Size::new(2, 1));
    assert_eq!(layer.grapheme_at(Position::new(1, 0)), None);
    state.undo().unwrap();
    let layer = &state.get_buffer().layers[1];
    assert_eq!(layer.offset(), Position::new(2, 1));
    assert_eq!(layer.size(), icy_engine::Size::new(3, 1));
    assert_eq!(layer.grapheme_at(Position::new(1, 0)), Some(("界", 2)));
}

#[test]
fn unicode_layer_resize_keeps_offset_and_restores_clipped_text() {
    let mut state = editor(8, 2);
    state.type_unicode_text("a界").unwrap();
    state.get_buffer_mut().layers[0].set_offset(Position::new(-2, 3));
    state.set_layer_size(0, (2, 1)).unwrap();
    assert_eq!(state.get_cur_layer().unwrap().offset(), Position::new(-2, 3));
    assert_eq!(glyph(&state, 1, 0), None);
    for _ in 0..2 {
        state.undo().unwrap();
        assert_eq!(glyph(&state, 1, 0), Some(("界", 2)));
        assert_eq!(state.get_cur_layer().unwrap().offset(), Position::new(-2, 3));
        state.redo().unwrap();
        assert_eq!(glyph(&state, 1, 0), None);
    }
}

#[test]
fn unicode_plain_text_paste_floats_then_anchors_at_document_offset() {
    let mut state = editor(12, 3);
    state.get_buffer_mut().layers[0].set_offset(Position::new(2, 1));
    state.set_caret_position(Position::new(1, 0));
    state.paste_text("e\u{301}界\r\n👩\u{200d}🚀").unwrap();
    assert_eq!(state.get_buffer().layers.len(), 2);
    // Document's floating-paste wrapper translates local paste coordinates.
    state.move_layer(Position::new(3, 1)).unwrap();
    assert_eq!(state.get_cur_layer().unwrap().offset(), Position::new(3, 1));
    assert_eq!(state.get_cur_layer().unwrap().grapheme_at(Position::new(0, 0)), Some(("e\u{301}", 1)));
    state.anchor_layer().unwrap();
    assert_eq!(state.get_buffer().layers.len(), 1);
    assert_eq!(state.get_cur_layer().unwrap().offset(), Position::new(2, 1));
    assert_eq!(glyph(&state, 1, 0), Some(("e\u{301}", 1)));
    assert_eq!(glyph(&state, 2, 0), Some(("界", 2)));
    assert_eq!(glyph(&state, 1, 1), Some(("👩\u{200d}🚀", 2)));
    state.undo().unwrap();
    assert_eq!(state.get_buffer().layers.len(), 2);
    state.undo().unwrap();
    state.undo().unwrap();
    assert_eq!(state.get_buffer().layers.len(), 1);
    assert_eq!(glyph(&state, 1, 0), None);
    state.redo().unwrap();
    state.redo().unwrap();
    state.redo().unwrap();
    assert_eq!(glyph(&state, 2, 0), Some(("界", 2)));
}

#[test]
fn unicode_merge_overlapping_wide_cells_never_leaves_orphan_continuations() {
    let mut state = editor(8, 1);
    state.type_key('界').unwrap();
    state.set_caret_position(Position::new(2, 0));
    state.paste_text("e\u{301}").unwrap();
    state.get_buffer_mut().layers[1].set_offset(Position::new(1, 0));
    state.merge_layer_down(1).unwrap();
    assert_eq!(glyph(&state, 0, 0), None);
    assert_eq!(cell(&state, 0, 0), ' ');
    assert_eq!(glyph(&state, 1, 0), Some(("e\u{301}", 1)));
    assert!(!state.get_cur_layer().unwrap().is_grapheme_continuation(Position::new(1, 0)));
    state.undo().unwrap();
    assert_eq!(glyph(&state, 0, 0), Some(("界", 2)));
}

#[test]
fn unicode_public_grapheme_write_rejects_invalid_or_clipped_text_atomically() {
    let mut state = editor(4, 1);
    state.type_key('界').unwrap();
    let layer = &mut state.get_buffer_mut().layers[0];
    for (position, text) in [
        (Position::new(0, 0), ""),
        (Position::new(0, 0), "ab"),
        (Position::new(0, 0), "\u{301}"),
        (Position::new(0, 0), "\n"),
        (Position::new(3, 0), "界"),
    ] {
        assert!(layer.set_grapheme(position, text, TextAttribute::default()).is_err());
        assert_eq!(layer.grapheme_at(Position::new(0, 0)), Some(("界", 2)));
    }
}

#[test]
fn unicode_unsupported_transformations_fail_without_modifying_content() {
    let mut state = editor(8, 2);
    state.type_unicode_text("界e\u{301}").unwrap();
    assert!(state.paste_rotate().is_err());
    assert!(state.insert_column().is_err());
    assert!(state.scroll_area_left().is_err());
    assert!(state.center().is_err());
    assert_eq!(glyph(&state, 0, 0), Some(("界", 2)));
    assert_eq!(glyph(&state, 2, 0), Some(("e\u{301}", 1)));
}

#[test]
fn unicode_leading_combining_mark_gets_a_visible_base_and_controls_are_rejected() {
    let mut state = editor(8, 1);
    state.type_key('\u{301}').unwrap();
    assert_eq!(glyph(&state, 0, 0), Some(("◌\u{301}", 1)));
    assert!(state.paste_text("ok\u{1b}[31m").is_err());
    assert_eq!(state.get_caret().position(), Position::new(1, 0));
}

#[test]
fn legacy_scalar_editing_remains_one_cell_and_does_not_add_metadata() {
    let mut state = EditState::from_buffer(icy_engine::TextBuffer::create((8, 1)));
    state.type_key('界').unwrap();
    assert_eq!(state.get_caret().position(), Position::new(1, 0));
    assert_eq!(glyph(&state, 0, 0), None);
    assert_eq!(cell(&state, 0, 0), '界');
}

#[test]
fn unicode_cell_writes_undo_only_the_touched_cells_including_split_wide_graphemes() {
    let mut state = editor(8, 2);
    state.type_unicode_text("a界b界").unwrap();
    let before: Vec<_> = (0..6).map(|x| glyph(&state, x, 0).map(|(text, width)| (text.to_owned(), width))).collect();
    {
        let _undo = state.begin_atomic_undo("Shape");
        // The continuation of the first and the leading cell of the second wide character.
        state
            .set_char_in_atomic(Position::new(2, 0), AttributedChar::new('\u{2591}', TextAttribute::default()))
            .unwrap();
        state
            .set_char_in_atomic(Position::new(4, 0), AttributedChar::new('\u{2588}', TextAttribute::default()))
            .unwrap();
        state
            .set_char_in_atomic(Position::new(0, 1), AttributedChar::new('\u{754c}', TextAttribute::default()))
            .unwrap();
    }
    assert_eq!(glyph(&state, 1, 0), None, "a split wide character is cleared");
    assert_eq!(cell(&state, 1, 0), ' ');
    assert_eq!(glyph(&state, 2, 0), Some(("\u{2591}", 1)));
    assert_eq!(glyph(&state, 4, 0), Some(("\u{2588}", 1)));
    assert_eq!(cell(&state, 5, 0), ' ');
    assert_eq!(glyph(&state, 0, 1), Some(("\u{754c}", 2)));
    let after: Vec<_> = (0..6).map(|x| glyph(&state, x, 0).map(|(text, width)| (text.to_owned(), width))).collect();
    let serialized = serde_json::to_string(state.get_undo_stack().lock().unwrap().undo_stack().last().unwrap()).unwrap();
    assert!(serialized.len() < 2048, "the shape keeps row segments, not layers ({} bytes)", serialized.len());
    state.undo().unwrap();
    let undone: Vec<_> = (0..6).map(|x| glyph(&state, x, 0).map(|(text, width)| (text.to_owned(), width))).collect();
    assert_eq!(undone, before);
    assert_eq!(glyph(&state, 0, 1), None);
    state.redo().unwrap();
    let redone: Vec<_> = (0..6).map(|x| glyph(&state, x, 0).map(|(text, width)| (text.to_owned(), width))).collect();
    assert_eq!(redone, after);
    assert_eq!(glyph(&state, 0, 1), Some(("\u{754c}", 2)));
}
