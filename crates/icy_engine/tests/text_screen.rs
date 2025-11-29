//! Unit tests for TextScreen - testing Screen and EditableScreen trait implementations

use icy_engine::{AttributedChar, EditableScreen, IceMode, Position, Screen, Selection, Size, TextAttribute, TextPane, TextScreen};

// ============================================================================
// TextPane Tests
// ============================================================================

#[test]
fn test_text_pane_get_size() {
    let screen = TextScreen::new(Size::new(80, 25));
    assert_eq!(screen.get_width(), 80);
    assert_eq!(screen.get_height(), 25);
    assert_eq!(screen.get_size(), Size::new(80, 25));
}

#[test]
fn test_text_pane_get_rectangle() {
    let screen = TextScreen::new(Size::new(80, 25));
    let rect = screen.get_rectangle();
    assert_eq!(rect.start.x, 0);
    assert_eq!(rect.start.y, 0);
    assert_eq!(rect.size.width, 80);
    assert_eq!(rect.size.height, 25);
}

#[test]
fn test_text_pane_get_char_default() {
    let screen = TextScreen::new(Size::new(80, 25));
    let ch = screen.get_char(Position::new(0, 0));
    // Default char is space ' '
    assert_eq!(ch.ch, ' ');
}

#[test]
fn test_text_pane_get_line_count() {
    let screen = TextScreen::new(Size::new(80, 25));
    // Empty screen may have 0 lines until content is added
    let count = screen.get_line_count();
    assert!(count >= 0);
}

#[test]
fn test_text_pane_get_line_length() {
    let mut screen = TextScreen::new(Size::new(80, 25));
    // Initially, line length should be 0 (no characters set)
    assert_eq!(screen.get_line_length(0), 0);

    // Set a character
    screen.set_char(Position::new(5, 0), AttributedChar::new('X', TextAttribute::default()));
    assert_eq!(screen.get_line_length(0), 6); // 0-5 inclusive = 6 chars
}

// ============================================================================
// Screen Tests
// ============================================================================

#[test]
fn test_screen_buffer_type() {
    let screen = TextScreen::new(Size::new(80, 25));
    let _buffer_type = screen.buffer_type();
    // Just ensure it doesn't panic and returns a valid value
}

#[test]
fn test_screen_ice_mode() {
    let screen = TextScreen::new(Size::new(80, 25));
    // Default is Unlimited (not Blink)
    assert_eq!(screen.ice_mode(), IceMode::Unlimited);
}

#[test]
fn test_screen_ice_mode_mut() {
    let mut screen = TextScreen::new(Size::new(80, 25));
    *screen.ice_mode_mut() = IceMode::Ice;
    assert_eq!(screen.ice_mode(), IceMode::Ice);
}

#[test]
fn test_screen_caret() {
    let screen = TextScreen::new(Size::new(80, 25));
    let caret = screen.caret();
    assert_eq!(caret.position(), Position::new(0, 0));
}

#[test]
fn test_screen_caret_position() {
    let mut screen = TextScreen::new(Size::new(80, 25));
    assert_eq!(screen.caret_position(), Position::new(0, 0));

    screen.caret_mut().set_position(Position::new(10, 5));
    assert_eq!(screen.caret_position(), Position::new(10, 5));
}

#[test]
fn test_screen_terminal_state() {
    let screen = TextScreen::new(Size::new(80, 25));
    let state = screen.terminal_state();
    // Just ensure it doesn't panic
    let _width = state.get_width();
}

#[test]
fn test_screen_palette() {
    let screen = TextScreen::new(Size::new(80, 25));
    let palette = screen.palette();
    // Ensure palette has colors
    assert!(palette.len() > 0);
}

#[test]
fn test_screen_get_font() {
    let screen = TextScreen::new(Size::new(80, 25));
    // Default screen should have at least font 0
    let font = screen.get_font(0);
    assert!(font.is_some());
}

#[test]
fn test_screen_font_count() {
    let screen = TextScreen::new(Size::new(80, 25));
    assert!(screen.font_count() > 0);
}

#[test]
fn test_screen_get_font_dimensions() {
    let screen = TextScreen::new(Size::new(80, 25));
    let dims = screen.get_font_dimensions();
    assert!(dims.width > 0);
    assert!(dims.height > 0);
}

#[test]
fn test_screen_get_resolution() {
    let screen = TextScreen::new(Size::new(80, 25));
    let resolution = screen.get_resolution();
    let font_dims = screen.get_font_dimensions();
    assert_eq!(resolution.width, 80 * font_dims.width);
    assert_eq!(resolution.height, 25 * font_dims.height);
}

#[test]
fn test_screen_default_foreground_color() {
    let screen = TextScreen::new(Size::new(80, 25));
    assert_eq!(screen.default_foreground_color(), 7);
}

#[test]
fn test_screen_get_version() {
    let screen = TextScreen::new(Size::new(80, 25));
    let v1 = screen.get_version();
    screen.mark_dirty();
    let v2 = screen.get_version();
    assert!(v2 > v1);
}

#[test]
fn test_screen_selection() {
    let mut screen = TextScreen::new(Size::new(80, 25));

    // Initially no selection
    assert!(screen.get_selection().is_none());

    // Set a selection
    let sel = Selection::new(Position::new(0, 0));
    screen.set_selection(sel).unwrap();
    assert!(screen.get_selection().is_some());

    // Clear selection
    screen.clear_selection().unwrap();
    assert!(screen.get_selection().is_none());
}

#[test]
fn test_screen_as_editable() {
    let mut screen = TextScreen::new(Size::new(80, 25));
    let editable = screen.as_editable();
    assert!(editable.is_some());
}

#[test]
fn test_screen_clone_box() {
    let screen = TextScreen::new(Size::new(80, 25));
    let cloned = screen.clone_box();
    assert_eq!(cloned.get_width(), 80);
    assert_eq!(cloned.get_height(), 25);
}

#[test]
fn test_screen_hyperlinks() {
    let screen = TextScreen::new(Size::new(80, 25));
    let links = screen.hyperlinks();
    assert!(links.is_empty());
}

#[test]
fn test_screen_mouse_fields() {
    let screen = TextScreen::new(Size::new(80, 25));
    let fields = screen.mouse_fields();
    assert!(fields.is_empty());
}

#[test]
fn test_screen_scan_lines() {
    let screen = TextScreen::new(Size::new(80, 25));
    assert!(!screen.scan_lines());
}

// ============================================================================
// EditableScreen Tests
// ============================================================================

#[test]
fn test_editable_set_char() {
    let mut screen = TextScreen::new(Size::new(80, 25));
    let ch = AttributedChar::new('A', TextAttribute::default());
    screen.set_char(Position::new(5, 3), ch);

    let result = screen.get_char(Position::new(5, 3));
    assert_eq!(result.ch, 'A');
}

#[test]
fn test_editable_print_char() {
    let mut screen = TextScreen::new(Size::new(80, 25));
    screen.terminal_state_mut().is_terminal_buffer = true;

    let ch = AttributedChar::new('H', TextAttribute::default());
    screen.print_char(ch);

    // Caret should have moved
    assert_eq!(screen.caret_position(), Position::new(1, 0));

    // Character should be at position 0
    let result = screen.get_char(Position::new(0, 0));
    assert_eq!(result.ch, 'H');
}

#[test]
fn test_editable_print_char_multiple() {
    let mut screen = TextScreen::new(Size::new(80, 25));
    screen.terminal_state_mut().is_terminal_buffer = true;

    for c in "Hello".chars() {
        screen.print_char(AttributedChar::new(c, TextAttribute::default()));
    }

    assert_eq!(screen.caret_position(), Position::new(5, 0));
    assert_eq!(screen.get_char(Position::new(0, 0)).ch, 'H');
    assert_eq!(screen.get_char(Position::new(1, 0)).ch, 'e');
    assert_eq!(screen.get_char(Position::new(2, 0)).ch, 'l');
    assert_eq!(screen.get_char(Position::new(3, 0)).ch, 'l');
    assert_eq!(screen.get_char(Position::new(4, 0)).ch, 'o');
}

#[test]
fn test_editable_set_size() {
    let mut screen = TextScreen::new(Size::new(80, 25));
    screen.set_size(Size::new(132, 50));
    assert_eq!(screen.get_width(), 132);
    assert_eq!(screen.get_height(), 50);
}

#[test]
fn test_editable_set_height() {
    let mut screen = TextScreen::new(Size::new(80, 25));
    screen.set_height(50);
    assert_eq!(screen.get_height(), 50);
}

#[test]
fn test_editable_caret_mut() {
    let mut screen = TextScreen::new(Size::new(80, 25));
    screen.caret_mut().set_position(Position::new(10, 5));
    assert_eq!(screen.caret_position(), Position::new(10, 5));
}

#[test]
fn test_editable_set_caret_position() {
    let mut screen = TextScreen::new(Size::new(80, 25));
    screen.set_caret_position(Position::new(15, 10));
    assert_eq!(screen.caret_position(), Position::new(15, 10));
}

#[test]
fn test_editable_clear_screen() {
    let mut screen = TextScreen::new(Size::new(80, 25));
    screen.terminal_state_mut().is_terminal_buffer = true;

    // Set some characters
    screen.set_char(Position::new(0, 0), AttributedChar::new('X', TextAttribute::default()));
    screen.set_char(Position::new(1, 0), AttributedChar::new('Y', TextAttribute::default()));
    screen.set_caret_position(Position::new(5, 5));

    screen.clear_screen();

    // Caret should be at origin
    assert_eq!(screen.caret_position(), Position::new(0, 0));

    // Characters should be cleared (back to default ' ')
    assert_eq!(screen.get_char(Position::new(0, 0)).ch, ' ');
}

#[test]
fn test_editable_clear_line() {
    let mut screen = TextScreen::new(Size::new(80, 25));

    // Set some characters on line 0
    for i in 0..10 {
        screen.set_char(Position::new(i, 0), AttributedChar::new('X', TextAttribute::default()));
    }

    screen.set_caret_position(Position::new(5, 0));
    screen.clear_line();

    // Line should be cleared
    assert_eq!(screen.get_line_length(0), 0);
}

#[test]
fn test_editable_clear_line_end() {
    let mut screen = TextScreen::new(Size::new(80, 25));

    // Set some characters on line 0
    for i in 0..10 {
        screen.set_char(Position::new(i, 0), AttributedChar::new('X', TextAttribute::default()));
    }

    screen.set_caret_position(Position::new(5, 0));
    screen.clear_line_end();

    // Only first 5 characters should remain
    assert_eq!(screen.get_line_length(0), 5);
}

#[test]
fn test_editable_clear_line_start() {
    let mut screen = TextScreen::new(Size::new(80, 25));

    // Set some characters on line 0
    for i in 0..10 {
        screen.set_char(Position::new(i, 0), AttributedChar::new('X', TextAttribute::default()));
    }

    screen.set_caret_position(Position::new(5, 0));
    screen.clear_line_start();

    // First 5 characters should be cleared (default char is ' ')
    for i in 0..5 {
        assert_eq!(screen.get_char(Position::new(i, 0)).ch, ' ');
    }
    // Rest should still be 'X'
    for i in 5..10 {
        assert_eq!(screen.get_char(Position::new(i, 0)).ch, 'X');
    }
}

#[test]
fn test_editable_lf() {
    let mut screen = TextScreen::new(Size::new(80, 25));
    screen.terminal_state_mut().is_terminal_buffer = true;

    screen.set_caret_position(Position::new(5, 0));
    screen.lf();

    // Should move to beginning of next line
    assert_eq!(screen.caret_position(), Position::new(0, 1));
}

#[test]
fn test_editable_cr() {
    let mut screen = TextScreen::new(Size::new(80, 25));

    screen.set_caret_position(Position::new(10, 5));
    screen.cr();

    // Should move to column 0, same line
    assert_eq!(screen.caret_position(), Position::new(0, 5));
}

#[test]
fn test_editable_ff() {
    let mut screen = TextScreen::new(Size::new(80, 25));
    screen.terminal_state_mut().is_terminal_buffer = true;

    screen.set_char(Position::new(0, 0), AttributedChar::new('X', TextAttribute::default()));
    screen.set_caret_position(Position::new(10, 10));

    screen.ff();

    // Should reset terminal and clear screen
    assert_eq!(screen.caret_position(), Position::new(0, 0));
}

#[test]
fn test_editable_home() {
    let mut screen = TextScreen::new(Size::new(80, 25));
    screen.terminal_state_mut().is_terminal_buffer = true;

    screen.set_caret_position(Position::new(10, 10));
    screen.home();

    assert_eq!(screen.caret_position(), Position::new(0, 0));
}

#[test]
fn test_editable_eol() {
    let mut screen = TextScreen::new(Size::new(80, 25));

    screen.set_caret_position(Position::new(0, 5));
    screen.eol();

    assert_eq!(screen.caret_position().x, 79); // Width - 1
}

#[test]
fn test_editable_bs() {
    let mut screen = TextScreen::new(Size::new(80, 25));

    screen.set_caret_position(Position::new(5, 0));
    screen.bs();

    assert_eq!(screen.caret_position(), Position::new(4, 0));
}

#[test]
fn test_editable_del() {
    let mut screen = TextScreen::new(Size::new(80, 25));

    // Set "HELLO" on line 0
    for (i, c) in "HELLO".chars().enumerate() {
        screen.set_char(Position::new(i as i32, 0), AttributedChar::new(c, TextAttribute::default()));
    }

    screen.set_caret_position(Position::new(1, 0)); // Position at 'E'
    screen.del();

    // 'E' should be deleted, 'L' should shift left
    assert_eq!(screen.get_char(Position::new(0, 0)).ch, 'H');
    assert_eq!(screen.get_char(Position::new(1, 0)).ch, 'L');
    assert_eq!(screen.get_char(Position::new(2, 0)).ch, 'L');
    assert_eq!(screen.get_char(Position::new(3, 0)).ch, 'O');
}

#[test]
fn test_editable_ins() {
    let mut screen = TextScreen::new(Size::new(80, 25));

    // Set "HLLO" on line 0
    for (i, c) in "HLLO".chars().enumerate() {
        screen.set_char(Position::new(i as i32, 0), AttributedChar::new(c, TextAttribute::default()));
    }

    screen.set_caret_position(Position::new(1, 0)); // Position at first 'L'
    screen.ins();

    // Space should be inserted at position 1, shifting 'L', 'L', 'O' right
    assert_eq!(screen.get_char(Position::new(0, 0)).ch, 'H');
    assert_eq!(screen.get_char(Position::new(1, 0)).ch, ' '); // Inserted space
    assert_eq!(screen.get_char(Position::new(2, 0)).ch, 'L');
    assert_eq!(screen.get_char(Position::new(3, 0)).ch, 'L');
    assert_eq!(screen.get_char(Position::new(4, 0)).ch, 'O');

    // Caret should NOT have moved (fixed bug!)
    assert_eq!(screen.caret_position(), Position::new(1, 0));
}

#[test]
fn test_editable_ins_does_not_move_caret() {
    let mut screen = TextScreen::new(Size::new(80, 25));

    screen.set_caret_position(Position::new(5, 0));
    screen.ins();

    // Caret should stay at the same position
    assert_eq!(screen.caret_position(), Position::new(5, 0));
}

#[test]
fn test_editable_left() {
    let mut screen = TextScreen::new(Size::new(80, 25));

    screen.set_caret_position(Position::new(10, 5));
    screen.left(3, false);

    assert_eq!(screen.caret_position(), Position::new(7, 5));
}

#[test]
fn test_editable_right() {
    let mut screen = TextScreen::new(Size::new(80, 25));

    screen.set_caret_position(Position::new(10, 5));
    screen.right(3, false);

    assert_eq!(screen.caret_position(), Position::new(13, 5));
}

#[test]
fn test_editable_up() {
    let mut screen = TextScreen::new(Size::new(80, 25));
    screen.terminal_state_mut().is_terminal_buffer = true;

    screen.set_caret_position(Position::new(10, 5));
    screen.up(2, false);

    assert_eq!(screen.caret_position(), Position::new(10, 3));
}

#[test]
fn test_editable_down() {
    let mut screen = TextScreen::new(Size::new(80, 25));
    screen.terminal_state_mut().is_terminal_buffer = true;

    screen.set_caret_position(Position::new(10, 5));
    screen.down(2, false);

    assert_eq!(screen.caret_position(), Position::new(10, 7));
}

#[test]
fn test_editable_tab_forward() {
    let mut screen = TextScreen::new(Size::new(80, 25));

    screen.set_caret_position(Position::new(3, 0));
    screen.tab_forward();

    assert_eq!(screen.caret_position().x, 8);
}

#[test]
fn test_editable_scroll_up() {
    let mut screen = TextScreen::new(Size::new(80, 25));
    screen.terminal_state_mut().is_terminal_buffer = true;

    // Set characters on lines 0, 1, 2
    screen.set_char(Position::new(0, 0), AttributedChar::new('A', TextAttribute::default()));
    screen.set_char(Position::new(0, 1), AttributedChar::new('B', TextAttribute::default()));
    screen.set_char(Position::new(0, 2), AttributedChar::new('C', TextAttribute::default()));

    screen.scroll_up();

    // Line 0 should now have 'B', line 1 should have 'C'
    assert_eq!(screen.get_char(Position::new(0, 0)).ch, 'B');
    assert_eq!(screen.get_char(Position::new(0, 1)).ch, 'C');
}

#[test]
fn test_editable_scroll_down() {
    let mut screen = TextScreen::new(Size::new(80, 25));
    screen.terminal_state_mut().is_terminal_buffer = true;

    // Set characters on lines 0, 1, 2
    screen.set_char(Position::new(0, 0), AttributedChar::new('A', TextAttribute::default()));
    screen.set_char(Position::new(0, 1), AttributedChar::new('B', TextAttribute::default()));
    screen.set_char(Position::new(0, 2), AttributedChar::new('C', TextAttribute::default()));

    screen.scroll_down();

    // Line 0 should be empty (default ' '), line 1 should have 'A', line 2 should have 'B'
    assert_eq!(screen.get_char(Position::new(0, 0)).ch, ' ');
    assert_eq!(screen.get_char(Position::new(0, 1)).ch, 'A');
    assert_eq!(screen.get_char(Position::new(0, 2)).ch, 'B');
}

#[test]
fn test_editable_scroll_left() {
    let mut screen = TextScreen::new(Size::new(80, 25));
    screen.terminal_state_mut().is_terminal_buffer = true;

    // Set characters on columns 0, 1, 2
    screen.set_char(Position::new(0, 0), AttributedChar::new('A', TextAttribute::default()));
    screen.set_char(Position::new(1, 0), AttributedChar::new('B', TextAttribute::default()));
    screen.set_char(Position::new(2, 0), AttributedChar::new('C', TextAttribute::default()));

    screen.scroll_left();

    // Column 0 should now have 'B', column 1 should have 'C'
    assert_eq!(screen.get_char(Position::new(0, 0)).ch, 'B');
    assert_eq!(screen.get_char(Position::new(1, 0)).ch, 'C');
}

#[test]
fn test_editable_scroll_right() {
    let mut screen = TextScreen::new(Size::new(80, 25));
    screen.terminal_state_mut().is_terminal_buffer = true;

    // Set characters on columns 0, 1, 2
    screen.set_char(Position::new(0, 0), AttributedChar::new('A', TextAttribute::default()));
    screen.set_char(Position::new(1, 0), AttributedChar::new('B', TextAttribute::default()));
    screen.set_char(Position::new(2, 0), AttributedChar::new('C', TextAttribute::default()));

    screen.scroll_right();

    // Column 0 should be empty (default ' '), column 1 should have 'A', column 2 should have 'B'
    assert_eq!(screen.get_char(Position::new(0, 0)).ch, ' ');
    assert_eq!(screen.get_char(Position::new(1, 0)).ch, 'A');
    assert_eq!(screen.get_char(Position::new(2, 0)).ch, 'B');
}

#[test]
fn test_editable_index() {
    let mut screen = TextScreen::new(Size::new(80, 25));
    screen.terminal_state_mut().is_terminal_buffer = true;

    screen.set_caret_position(Position::new(5, 3));
    screen.index();

    // Should move down one line, keeping x position
    assert_eq!(screen.caret_position(), Position::new(5, 4));
}

#[test]
fn test_editable_next_line() {
    let mut screen = TextScreen::new(Size::new(80, 25));
    screen.terminal_state_mut().is_terminal_buffer = true;

    screen.set_caret_position(Position::new(10, 3));
    screen.next_line(false);

    // Should move to beginning of next line
    assert_eq!(screen.caret_position(), Position::new(0, 4));
}

#[test]
fn test_editable_reset_terminal() {
    let mut screen = TextScreen::new(Size::new(80, 25));

    screen.caret_mut().set_position(Position::new(10, 10));
    screen.caret_mut().attribute.set_foreground(3);

    screen.reset_terminal();

    assert_eq!(screen.caret_position(), Position::new(0, 0));
}

#[test]
fn test_editable_layer_management() {
    let mut screen = TextScreen::new(Size::new(80, 25));

    // Should have at least one layer
    assert!(screen.layer_count() >= 1);
    assert_eq!(screen.get_current_layer(), 0);

    // Can get layer
    assert!(screen.get_layer(0).is_some());
    assert!(screen.get_layer_mut(0).is_some());

    // Out of range layer returns None
    assert!(screen.get_layer(999).is_none());
}

#[test]
fn test_editable_editable_lines() {
    let screen = TextScreen::new(Size::new(80, 25));

    assert_eq!(screen.get_first_visible_line(), 0);
    assert_eq!(screen.get_first_editable_line(), 0);
    assert!(screen.get_last_visible_line() >= 0);
    assert!(screen.get_last_editable_line() >= 0);
}

#[test]
fn test_editable_editable_columns() {
    let screen = TextScreen::new(Size::new(80, 25));

    assert_eq!(screen.get_first_editable_column(), 0);
    assert_eq!(screen.get_last_editable_column(), 79);
}

#[test]
fn test_editable_get_line() {
    let mut screen = TextScreen::new(Size::new(80, 25));

    screen.set_char(Position::new(0, 0), AttributedChar::new('X', TextAttribute::default()));

    let line = screen.get_line(0);
    assert!(line.is_some());
}

#[test]
fn test_editable_line_count() {
    let mut screen = TextScreen::new(Size::new(80, 25));
    // Set terminal buffer to get proper line count
    screen.terminal_state_mut().is_terminal_buffer = true;
    // Line count depends on buffer initialization
    let _count = screen.line_count();
    // Just ensure the method works without panicking
}

#[test]
fn test_editable_saved_caret_pos() {
    let mut screen = TextScreen::new(Size::new(80, 25));

    *screen.saved_caret_pos() = Position::new(10, 5);
    assert_eq!(*screen.saved_caret_pos(), Position::new(10, 5));
}

#[test]
fn test_editable_saved_cursor_state() {
    let mut screen = TextScreen::new(Size::new(80, 25));

    screen.saved_cursor_state().caret.set_position(Position::new(15, 8));
    assert_eq!(screen.saved_cursor_state().caret.position(), Position::new(15, 8));
}

#[test]
fn test_editable_terminal_state_mut() {
    let mut screen = TextScreen::new(Size::new(80, 25));

    screen.terminal_state_mut().is_terminal_buffer = true;
    assert!(screen.terminal_state().is_terminal_buffer);
}

#[test]
fn test_editable_palette_mut() {
    let mut screen = TextScreen::new(Size::new(80, 25));

    // Modify palette
    let _palette = screen.palette_mut();
    // Just ensure it doesn't panic
}

#[test]
fn test_editable_buffer_type_mut() {
    let mut screen = TextScreen::new(Size::new(80, 25));

    let _buffer_type = screen.buffer_type_mut();
    // Just ensure it doesn't panic
}

#[test]
fn test_editable_mark_dirty() {
    let screen = TextScreen::new(Size::new(80, 25));

    let v1 = screen.get_version();
    screen.mark_dirty();
    let v2 = screen.get_version();

    assert!(v2 > v1);
}

#[test]
fn test_editable_update_hyperlinks() {
    let mut screen = TextScreen::new(Size::new(80, 25));

    screen.update_hyperlinks();
    // Just ensure it doesn't panic
}

#[test]
fn test_editable_add_hyperlink() {
    let mut screen = TextScreen::new(Size::new(80, 25));

    let link = icy_engine::HyperLink {
        url: Some("https://example.com".to_string()),
        position: Position::new(0, 0),
        length: 10,
    };
    screen.add_hyperlink(link);

    assert_eq!(screen.hyperlinks().len(), 1);
}

#[test]
fn test_editable_clear_mouse_fields() {
    let mut screen = TextScreen::new(Size::new(80, 25));

    // Add and clear mouse fields
    screen.clear_mouse_fields();
    assert!(screen.mouse_fields().is_empty());
}

#[test]
fn test_editable_insert_line() {
    let mut screen = TextScreen::new(Size::new(80, 25));

    // Create a line with a character
    let mut line = icy_engine::Line::new();
    line.set_char(0, AttributedChar::new('Z', TextAttribute::default()));

    screen.insert_line(0, line);

    // Verify the line was inserted by checking that we can get it
    let retrieved = screen.get_line(0);
    assert!(retrieved.is_some());
    // The inserted line should have the character 'Z' at position 0
    let chars: Vec<_> = retrieved.unwrap().chars.iter().take(1).collect();
    assert_eq!(chars.len(), 1);
    assert_eq!(chars[0].ch, 'Z');
}

#[test]
fn test_editable_remove_terminal_line() {
    let mut screen = TextScreen::new(Size::new(80, 25));

    // Set a character on line 0
    screen.set_char(Position::new(0, 0), AttributedChar::new('X', TextAttribute::default()));

    screen.remove_terminal_line(0);

    // Line 0 should now have what was on line 1 (empty)
    // Behavior depends on margins, but should not panic
}

#[test]
fn test_editable_insert_terminal_line() {
    let mut screen = TextScreen::new(Size::new(80, 25));

    // Set a character on line 0
    screen.set_char(Position::new(0, 0), AttributedChar::new('X', TextAttribute::default()));

    screen.insert_terminal_line(0);

    // Line 0 should now be empty (default ' '), 'X' should have moved to line 1
    assert_eq!(screen.get_char(Position::new(0, 0)).ch, ' ');
}

#[test]
fn test_editable_set_font() {
    let mut screen = TextScreen::new(Size::new(80, 25));

    // Get a font to set
    if let Some(font) = screen.get_font(0).cloned() {
        screen.set_font(1, font);
        assert!(screen.get_font(1).is_some());
    }
}

#[test]
fn test_editable_remove_font() {
    let mut screen = TextScreen::new(Size::new(80, 25));

    // Try to remove font 0
    let removed = screen.remove_font(0);
    assert!(removed.is_some());
}

#[test]
fn test_editable_clear_font_table() {
    let mut screen = TextScreen::new(Size::new(80, 25));

    screen.clear_font_table();
    // Font table should be empty now
    assert_eq!(screen.font_count(), 0);
}

#[test]
fn test_editable_scrollback_buffer_size() {
    let mut screen = TextScreen::new(Size::new(80, 25));

    screen.set_scrollback_buffer_size(1000);
    // Just ensure it doesn't panic
}

#[test]
fn test_editable_clear_scrollback() {
    let mut screen = TextScreen::new(Size::new(80, 25));

    screen.clear_scrollback();
    // Just ensure it doesn't panic
}

#[test]
fn test_editable_clear_buffer_down() {
    let mut screen = TextScreen::new(Size::new(80, 25));
    screen.terminal_state_mut().is_terminal_buffer = true;

    // Set characters on multiple lines
    for y in 0..10 {
        screen.set_char(Position::new(0, y), AttributedChar::new('X', TextAttribute::default()));
    }

    screen.set_caret_position(Position::new(0, 5));
    screen.clear_buffer_down();

    // Lines 5+ should be cleared (default is ' ')
    for y in 5..10 {
        assert_eq!(screen.get_char(Position::new(0, y)).ch, ' ');
    }
    // Lines 0-4 should still have 'X'
    for y in 0..5 {
        assert_eq!(screen.get_char(Position::new(0, y)).ch, 'X');
    }
}

#[test]
fn test_editable_clear_buffer_up() {
    let mut screen = TextScreen::new(Size::new(80, 25));
    screen.terminal_state_mut().is_terminal_buffer = true;

    // Set characters on multiple lines
    for y in 0..10 {
        screen.set_char(Position::new(0, y), AttributedChar::new('X', TextAttribute::default()));
    }

    screen.set_caret_position(Position::new(0, 5));
    screen.clear_buffer_up();

    // Lines 0-4 should be cleared (default is ' ')
    for y in 0..5 {
        assert_eq!(screen.get_char(Position::new(0, y)).ch, ' ');
    }
    // Position (0, 5) - cursor position - should ALSO be cleared per DEC spec (ED 1 erases inclusive)
    assert_eq!(screen.get_char(Position::new(0, 5)).ch, ' ');

    // Lines 6+ should still have 'X' (positions AFTER cursor)
    for y in 6..10 {
        assert_eq!(screen.get_char(Position::new(0, y)).ch, 'X');
    }
}

#[test]
fn test_editable_limit_caret_pos() {
    let mut screen = TextScreen::new(Size::new(80, 25));
    screen.terminal_state_mut().is_terminal_buffer = true;

    // Set caret outside bounds
    screen.caret_mut().set_position(Position::new(100, 30));
    screen.limit_caret_pos(false);

    // Caret should be limited to screen bounds
    assert!(screen.caret_position().x <= 79);
    assert!(screen.caret_position().y <= 24);
}

#[test]
fn test_editable_upper_left_position() {
    let screen = TextScreen::new(Size::new(80, 25));

    let pos = screen.upper_left_position();
    assert_eq!(pos, Position::new(0, 0));
}

#[test]
fn test_editable_caret_default_colors() {
    let mut screen = TextScreen::new(Size::new(80, 25));

    screen.caret_mut().attribute.set_foreground(3);
    screen.caret_mut().attribute.set_background(5);

    screen.caret_default_colors();

    assert_eq!(screen.caret().attribute.get_foreground(), 7);
}

#[test]
fn test_editable_sgr_reset() {
    let mut screen = TextScreen::new(Size::new(80, 25));

    screen.caret_mut().attribute.set_is_bold(true);
    screen.terminal_state_mut().inverse_video = true;

    screen.sgr_reset();

    assert!(!screen.caret().attribute.is_bold());
    assert!(!screen.terminal_state().inverse_video);
}

// ============================================================================
// Insert Mode Tests (verifying the bug fix)
// ============================================================================

#[test]
fn test_insert_mode_print_char() {
    let mut screen = TextScreen::new(Size::new(80, 25));
    screen.terminal_state_mut().is_terminal_buffer = true;

    // Enable insert mode
    screen.caret_mut().insert_mode = true;

    // Type "HELLO"
    for c in "HELLO".chars() {
        screen.print_char(AttributedChar::new(c, TextAttribute::default()));
    }

    // Check result - should be "HELLO" without gaps
    assert_eq!(screen.get_char(Position::new(0, 0)).ch, 'H');
    assert_eq!(screen.get_char(Position::new(1, 0)).ch, 'E');
    assert_eq!(screen.get_char(Position::new(2, 0)).ch, 'L');
    assert_eq!(screen.get_char(Position::new(3, 0)).ch, 'L');
    assert_eq!(screen.get_char(Position::new(4, 0)).ch, 'O');

    // Caret should be at position 5
    assert_eq!(screen.caret_position(), Position::new(5, 0));
}

#[test]
fn test_insert_mode_no_gaps() {
    let mut screen = TextScreen::new(Size::new(80, 25));
    screen.terminal_state_mut().is_terminal_buffer = true;

    // First type "WORLD" normally
    for c in "WORLD".chars() {
        screen.print_char(AttributedChar::new(c, TextAttribute::default()));
    }

    // Now move to beginning and enable insert mode
    screen.set_caret_position(Position::new(0, 0));
    screen.caret_mut().insert_mode = true;

    // Type "HELLO "
    for c in "HELLO ".chars() {
        screen.print_char(AttributedChar::new(c, TextAttribute::default()));
    }

    // Result should be "HELLO WORLD" (WORLD shifted right)
    assert_eq!(screen.get_char(Position::new(0, 0)).ch, 'H');
    assert_eq!(screen.get_char(Position::new(1, 0)).ch, 'E');
    assert_eq!(screen.get_char(Position::new(2, 0)).ch, 'L');
    assert_eq!(screen.get_char(Position::new(3, 0)).ch, 'L');
    assert_eq!(screen.get_char(Position::new(4, 0)).ch, 'O');
    assert_eq!(screen.get_char(Position::new(5, 0)).ch, ' ');
    assert_eq!(screen.get_char(Position::new(6, 0)).ch, 'W');
    assert_eq!(screen.get_char(Position::new(7, 0)).ch, 'O');
    assert_eq!(screen.get_char(Position::new(8, 0)).ch, 'R');
    assert_eq!(screen.get_char(Position::new(9, 0)).ch, 'L');
    assert_eq!(screen.get_char(Position::new(10, 0)).ch, 'D');
}

#[test]
fn test_insert_line_in_scroll_region() {
    let mut screen = TextScreen::new(Size::new(80, 25));
    screen.terminal_state_mut().set_margins_top_bottom(1, 20); // 0-based, rows 1-20

    // Put text on row 1
    screen.set_char(Position::new(0, 1), AttributedChar::new('A', TextAttribute::default()));

    // Insert a line at row 1 - should push 'A' to row 2
    screen.insert_terminal_line(1);

    // Row 1 should be empty, row 2 should have 'A'
    let ch1 = screen.get_char(Position::new(0, 1));
    let ch2 = screen.get_char(Position::new(0, 2));

    assert!(ch1.is_transparent() || ch1.ch == '\0' || ch1.ch == ' ', "Row 1 should be empty, got: {:?}", ch1);
    assert_eq!(ch2.ch, 'A', "Row 2 should have 'A', got: {:?}", ch2);
}

#[test]
fn test_insert_line_outside_scroll_region() {
    let mut screen = TextScreen::new(Size::new(80, 25));
    screen.terminal_state_mut().set_margins_top_bottom(1, 20); // 0-based, rows 1-20

    // Put text on row 1
    screen.set_char(Position::new(0, 1), AttributedChar::new('A', TextAttribute::default()));

    // Try to insert a line at row 0 (outside scroll region) - should do nothing
    screen.insert_terminal_line(0);

    // Row 1 should still have 'A'
    let ch1 = screen.get_char(Position::new(0, 1));
    assert_eq!(ch1.ch, 'A', "Row 1 should still have 'A', got: {:?}", ch1);
}

#[test]
fn test_cpbug2_sequence() {
    let mut screen = TextScreen::new(Size::new(80, 25));

    // The sequence from cpbug2.ans end:
    // 1. Set scroll region [2;21r (1-based) = rows 1-20 (0-based)
    screen.terminal_state_mut().set_margins_top_bottom(1, 20);

    // 2. Move to row 2 (1-based) = row 1 (0-based)
    screen.set_caret_position(Position::new(0, 1));

    // 3. Insert 20 lines - should clear the scroll region
    for _ in 0..20 {
        screen.insert_terminal_line(1);
    }

    // 4. Write "Screen END !" on row 1
    let text = "Screen END !";
    for (i, c) in text.chars().enumerate() {
        screen.set_char(Position::new(i as i32, 1), AttributedChar::new(c, TextAttribute::default()));
    }

    // 5. Do the [A[L[B sequence 18 times (cursor at row 1)
    // [A - cursor up to row 0 (outside scroll region!)
    // [L - insert line (should have NO effect since row 0 is outside region)
    // [B - cursor down to row 1
    for _ in 0..18 {
        // These insert lines should have NO effect since cursor is at row 0
        screen.insert_terminal_line(0); // This should do nothing!
    }

    // "Screen END !" should still be on row 1
    let ch = screen.get_char(Position::new(0, 1));
    assert_eq!(ch.ch, 'S', "Expected 'S' at row 1, col 0, got: {:?}", ch);

    // Row 2 should be empty
    let ch2 = screen.get_char(Position::new(0, 2));
    assert!(ch2.is_transparent() || ch2.ch == ' ' || ch2.ch == '\0', "Row 2 should be empty, got: {:?}", ch2);
}
