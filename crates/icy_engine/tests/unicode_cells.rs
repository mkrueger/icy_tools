use icy_engine::{BufferType, EditableScreen, Position, Screen, TextPane, TextScreen};
use icy_parser_core::{AnsiParser, CommandParser};

fn unicode_screen(width: i32, height: i32) -> TextScreen {
    let mut screen = TextScreen::new((width, height));
    screen.buffer.buffer_type = BufferType::Unicode;
    screen.buffer.terminal_state.is_terminal_buffer = true;
    screen.set_unicode_width(true);
    screen
}

#[test]
fn unicode_cells_join_across_parser_calls() {
    let mut screen = unicode_screen(80, 25);
    let mut parser = AnsiParser::default();
    for byte in "a\u{308}\u{754c}|".bytes() {
        parser.parse(&[byte], &mut icy_engine::ScreenSink::new(&mut screen));
    }
    assert_eq!(screen.grapheme_at(Position::new(0, 0)), Some(("a\u{308}", 1)));
    assert_eq!(screen.grapheme_at(Position::new(1, 0)), Some(("\u{754c}", 2)));
    assert!(screen.is_grapheme_continuation(Position::new(2, 0)));
    assert_eq!(screen.caret_position(), Position::new(4, 0));
}

fn feed(screen: &mut TextScreen, text: &str) {
    AnsiParser::default().parse(text.as_bytes(), &mut icy_engine::ScreenSink::new(screen));
}

#[test]
fn unicode_cells_width_matrix_and_packet_splits() {
    for (text, width) in [
        ("AB", 2),
        ("\u{e4}", 1),
        ("a\u{308}", 1),
        ("\u{754c}\u{96ea}", 4),
        ("\u{b7}", 1),
        ("\u{1f469}\u{200d}\u{1f4bb}", 2),
        ("\u{1f44d}\u{1f3fd}", 2),
        ("\u{2764}\u{fe0f}", 2),
        ("\u{2648}\u{fe0e}", 1),
    ] {
        for split in 0..=text.len() {
            let mut screen = unicode_screen(80, 25);
            let mut parser = AnsiParser::default();
            parser.parse(&text.as_bytes()[..split], &mut icy_engine::ScreenSink::new(&mut screen));
            parser.parse(&text.as_bytes()[split..], &mut icy_engine::ScreenSink::new(&mut screen));
            assert_eq!(screen.caret_position(), Position::new(width, 0), "{text:?}, split {split}");
        }
    }
}

#[test]
fn unicode_cells_cursor_commands_break_attachment() {
    let mut screen = unicode_screen(80, 25);
    feed(&mut screen, "a\x1b[1;2H\u{308}");
    assert_eq!(screen.grapheme_at(Position::new(0, 0)), Some(("a", 1)));
    assert_eq!(screen.grapheme_at(Position::new(1, 0)), Some(("\u{25cc}\u{308}", 1)));
    screen.set_caret_position(Position::new(2, 0));
    feed(&mut screen, "\u{301}");
    assert_eq!(screen.grapheme_at(Position::new(2, 0)), Some(("\u{25cc}\u{301}", 1)));
}

#[test]
fn unicode_cells_wrap_and_no_wrap_at_both_edges() {
    for (width, height) in [(80, 25), (132, 43)] {
        let mut screen = unicode_screen(width, height);
        feed(&mut screen, &format!("\x1b[{};{}H\u{754c}|", height - 1, width));
        assert_eq!(screen.grapheme_at(Position::new(0, height - 1)), Some(("\u{754c}", 2)));
        assert_eq!(screen.caret_position(), Position::new(3, height - 1));
        feed(&mut screen, &format!("\x1b[1;{}Ha\u{308}|", width));
        assert_eq!(screen.grapheme_at(Position::new(width - 1, 0)), Some(("a\u{308}", 1)));
        assert_eq!(screen.caret_position(), Position::new(1, 1));
        feed(&mut screen, &format!("\x1b[1;{}H\u{2764}\u{fe0f}|", width));
        assert_eq!(screen.grapheme_at(Position::new(0, 1)), Some(("\u{2764}\u{fe0f}", 2)));
        feed(&mut screen, &format!("\x1b[?7l\x1b[1;{}H\u{754c}", width));
        assert_eq!(screen.grapheme_at(Position::new(width - 1, 0)), Some((".", 1)));
    }
}

#[test]
fn unicode_cells_erase_continuation_and_shift_whole_graphemes() {
    let mut screen = unicode_screen(8, 4);
    feed(&mut screen, "A\u{754c}B\x1b[1;3H\x1b[X");
    assert_eq!(screen.char_at(Position::new(1, 0)).ch, ' ');
    assert_eq!(screen.char_at(Position::new(2, 0)).ch, ' ');
    assert_eq!(screen.char_at(Position::new(3, 0)).ch, 'B');
    feed(&mut screen, "\x1b[2J\x1b[HA\u{754c}B\x1b[H\x1b[P");
    assert_eq!(screen.grapheme_at(Position::new(0, 0)), Some(("\u{754c}", 2)));
    assert!(screen.is_grapheme_continuation(Position::new(1, 0)));
    assert_eq!(screen.char_at(Position::new(2, 0)).ch, 'B');
    feed(&mut screen, "\x1b[@");
    assert_eq!(screen.grapheme_at(Position::new(1, 0)), Some(("\u{754c}", 2)));
}

#[test]
fn unicode_cells_scroll_and_resize_preserve_metadata() {
    let mut screen = unicode_screen(8, 4);
    feed(&mut screen, "\x1b[2;1Ha\u{308}\u{754c}");
    screen.scroll_up();
    assert_eq!(screen.grapheme_at(Position::new(0, 0)), Some(("a\u{308}", 1)));
    assert_eq!(screen.grapheme_at(Position::new(1, 0)), Some(("\u{754c}", 2)));
    screen.scroll_down();
    assert_eq!(screen.grapheme_at(Position::new(1, 1)), Some(("\u{754c}", 2)));
    screen.set_width(2);
    assert_eq!(screen.grapheme_at(Position::new(1, 1)), None);
    screen.set_width(8);
    assert_eq!(screen.char_at(Position::new(1, 1)).ch, ' ');
    assert!(!screen.is_grapheme_continuation(Position::new(2, 1)));
}

#[test]
fn unicode_cells_legacy_is_opt_in() {
    let mut screen = TextScreen::new((80, 25));
    screen.buffer.buffer_type = BufferType::Unicode;
    feed(&mut screen, "a\u{308}\u{754c}");
    assert_eq!(screen.caret_position(), Position::new(3, 0));
    assert_eq!(screen.char_at(Position::new(1, 0)).ch, '\u{308}');
    assert_eq!(screen.grapheme_at(Position::new(0, 0)), None);
}

#[test]
fn unicode_cells_insert_mode_tracks_late_width_changes() {
    let mut screen = unicode_screen(12, 4);
    feed(&mut screen, "ABC\x1b[H\x1b[4h\u{2764}\u{fe0f}");
    assert_eq!(screen.grapheme_at(Position::new(0, 0)), Some(("\u{2764}\u{fe0f}", 2)));
    assert_eq!(screen.char_at(Position::new(2, 0)).ch, 'A');
    assert_eq!(screen.char_at(Position::new(4, 0)).ch, 'C');
    feed(&mut screen, "\x1b[2;1HXYZ\x1b[2;1H\u{2648}\u{fe0e}");
    assert_eq!(screen.grapheme_at(Position::new(0, 1)), Some(("\u{2648}\u{fe0e}", 1)));
    assert_eq!(screen.char_at(Position::new(1, 1)).ch, 'X');
    assert_eq!(screen.char_at(Position::new(3, 1)).ch, 'Z');
}

#[test]
fn unicode_cells_line_operations_and_scroll_margins() {
    let mut screen = unicode_screen(8, 4);
    feed(&mut screen, "\x1b[2;1Ha\u{308}\u{754c}\x1b[H\x1b[L");
    assert_eq!(screen.grapheme_at(Position::new(0, 2)), Some(("a\u{308}", 1)));
    feed(&mut screen, "\x1b[M");
    assert_eq!(screen.grapheme_at(Position::new(1, 1)), Some(("\u{754c}", 2)));
    feed(&mut screen, "\x1b[2;4r\x1b[2;1H\x1b[L");
    assert_eq!(screen.grapheme_at(Position::new(1, 2)), Some(("\u{754c}", 2)));
    feed(&mut screen, "\x1b[M");
    assert_eq!(screen.grapheme_at(Position::new(1, 1)), Some(("\u{754c}", 2)));
    screen.scroll_right();
    assert_eq!(screen.grapheme_at(Position::new(2, 1)), Some(("\u{754c}", 2)));
    screen.scroll_left();
    assert_eq!(screen.grapheme_at(Position::new(1, 1)), Some(("\u{754c}", 2)));
}

#[test]
fn unicode_cells_direct_cursor_and_clear_break_attachment() {
    for command in [TextScreen::cr, TextScreen::bs, TextScreen::clear_line] {
        let mut screen = unicode_screen(8, 4);
        feed(&mut screen, "a");
        command(&mut screen);
        let position = screen.caret.position();
        feed(&mut screen, "\u{308}");
        assert_eq!(screen.grapheme_at(position), Some(("\u{25cc}\u{308}", 1)));
    }
}

#[test]
fn unicode_cells_clone_and_pcboard_snapshot_round_trip() {
    use icy_engine::{CharacterFormatOptions, FileFormat, FormatOptions, SaveOptions};
    let mut screen = unicode_screen(80, 25);
    feed(&mut screen, "a\u{308}\u{754c}\u{1f469}\u{200d}\u{1f4bb}|");
    let clone = screen.buffer.clone();
    let restored = TextScreen::from_buffer(clone.clone());
    assert!(restored.unicode_width());
    let pane: &dyn TextPane = &clone;
    assert_eq!(pane.grapheme_at(Position::new(0, 0)), Some(("a\u{308}", 1)));
    assert!(pane.is_grapheme_continuation(Position::new(2, 0)));
    let options = SaveOptions {
        format: FormatOptions::Character(CharacterFormatOptions {
            unicode: true,
            ..Default::default()
        }),
        ..Default::default()
    };
    let bytes = FileFormat::PCBoard.to_bytes(&clone, &options).unwrap();
    assert!(String::from_utf8_lossy(&bytes).contains("a\u{308}\u{754c}\u{1f469}\u{200d}\u{1f4bb}|"));
    let mut replay = unicode_screen(80, 25);
    icy_parser_core::PcBoardParser::default().parse(
        bytes.strip_prefix(b"\xef\xbb\xbf").unwrap_or(&bytes),
        &mut icy_engine::ScreenSink::new(&mut replay),
    );
    for column in 0..6 {
        let position = Position::new(column, 0);
        assert_eq!(screen.grapheme_at(position), replay.grapheme_at(position));
        assert_eq!(screen.is_grapheme_continuation(position), replay.is_grapheme_continuation(position));
    }
    assert_eq!(screen.caret_position(), replay.caret_position());
}

#[test]
fn unicode_cells_bound_combining_storage_and_resume() {
    let mut screen = unicode_screen(80, 25);
    feed(&mut screen, &format!("a{}|", "\u{308}".repeat(10_000)));
    let (text, width) = screen.grapheme_at(Position::new(0, 0)).unwrap();
    assert!(text.len() <= icy_engine::MAX_GRAPHEME_BYTES);
    assert_eq!(width, 1);
    assert_eq!(screen.grapheme_at(Position::new(1, 0)), Some(("|", 1)));
    assert_eq!(screen.caret.position(), Position::new(2, 0));
}

#[test]
fn unicode_cells_sgr_keeps_base_style_and_attachment() {
    let mut screen = unicode_screen(80, 25);
    feed(&mut screen, "\x1b[31ma\x1b[32m\u{308}B");
    assert_eq!(screen.grapheme_at(Position::new(0, 0)), Some(("a\u{308}", 1)));
    assert_ne!(screen.char_at(Position::new(0, 0)).attribute, screen.char_at(Position::new(1, 0)).attribute);
    assert_eq!(screen.caret.position(), Position::new(2, 0));
}

#[test]
fn unicode_cells_overwrite_either_half_without_moving_neighbors() {
    for column in [2, 3] {
        let mut screen = unicode_screen(80, 25);
        feed(&mut screen, &format!("A\u{754c}B\x1b[1;{column}HX"));
        assert_eq!(screen.char_at(Position::new(0, 0)).ch, 'A');
        assert_eq!(screen.char_at(Position::new(3, 0)).ch, 'B');
        assert_eq!(screen.char_at(Position::new(column - 1, 0)).ch, 'X');
        assert_eq!(screen.char_at(Position::new(4 - column, 0)).ch, ' ');
        assert!(!screen.is_grapheme_continuation(Position::new(2, 0)));
    }
}

#[test]
fn unicode_cells_dch_drops_partial_grapheme_but_shifts_cells() {
    let mut screen = unicode_screen(8, 4);
    feed(&mut screen, "A\u{754c}BCD\x1b[1;3H\x1b[P");
    assert_eq!(screen.char_at(Position::new(0, 0)).ch, 'A');
    assert_eq!(screen.char_at(Position::new(1, 0)).ch, ' ');
    assert_eq!(screen.char_at(Position::new(2, 0)).ch, 'B');
    assert_eq!(screen.char_at(Position::new(4, 0)).ch, 'D');
}

#[test]
fn unicode_cells_cp437_remains_byte_oriented() {
    let mut screen = TextScreen::new((80, 25));
    let bytes = [0x84, 0x94, b'.', b'.', b'|'];
    AnsiParser::default().parse(&bytes, &mut icy_engine::ScreenSink::new(&mut screen));
    for (column, byte) in bytes.iter().enumerate() {
        assert_eq!(screen.char_at(Position::new(column as i32, 0)).ch as u32, u32::from(*byte));
    }
    assert!(!screen.unicode_width());
    assert_eq!(screen.caret.position(), Position::new(5, 0));
}

#[test]
fn unicode_cells_localized_snapshot_edges_and_bytewise_replay() {
    use icy_engine::{CharacterFormatOptions, FileFormat, FormatOptions, SaveOptions};
    for label in ["Width", "Breite"] {
        for (width, height) in [(80, 25), (132, 43)] {
            let mut screen = unicode_screen(width, height);
            feed(
                &mut screen,
                &format!("{label}\x1b[4;3Ha\u{308}\u{754c}|\x1b[{};{}H\u{96ea}|", height - 1, width - 1),
            );
            assert_eq!(screen.grapheme_at(Position::new(width - 2, height - 2)), Some(("\u{96ea}", 2)));
            assert_eq!(screen.caret.position(), Position::new(1, height - 1));
            let options = SaveOptions {
                format: FormatOptions::Character(CharacterFormatOptions {
                    unicode: true,
                    ..Default::default()
                }),
                ..Default::default()
            };
            let output = FileFormat::PCBoard.to_bytes(&screen.buffer, &options).unwrap();
            let mut replay = unicode_screen(width, height);
            let mut parser = icy_parser_core::PcBoardParser::default();
            for byte in output.strip_prefix(b"\xef\xbb\xbf").unwrap_or(&output) {
                parser.parse(&[*byte], &mut icy_engine::ScreenSink::new(&mut replay));
            }
            for row in 0..height {
                for column in 0..width {
                    let position = Position::new(column, row);
                    assert_eq!(
                        screen.char_at(position).ch,
                        replay.char_at(position).ch,
                        "{label}, {width}x{height}, {position:?}"
                    );
                    assert_eq!(screen.is_grapheme_continuation(position), replay.is_grapheme_continuation(position));
                    if let Some(grapheme) = screen.grapheme_at(position) {
                        assert_eq!(Some(grapheme), replay.grapheme_at(position));
                    }
                }
            }
            assert_eq!(screen.caret.position(), replay.caret.position());
        }
    }
}

#[test]
fn unicode_cells_direct_buffer_resize_and_layer_locks() {
    let mut screen = unicode_screen(8, 4);
    feed(&mut screen, "a");
    screen.buffer.layers[0].properties.is_locked = true;
    feed(&mut screen, "\u{308}");
    assert_eq!(screen.grapheme_at(Position::new(0, 0)), Some(("a", 1)));
    screen.buffer.layers[0].properties.is_locked = false;
    feed(&mut screen, "\x1b[H");
    feed(&mut screen, "A\u{754c}");
    screen.buffer.set_size((2, 4));
    screen.buffer.set_size((8, 4));
    assert_eq!(screen.grapheme_at(Position::new(1, 0)), None);
    assert!(!screen.is_grapheme_continuation(Position::new(2, 0)));
    screen.buffer.layers[0].properties.is_locked = true;
    feed(&mut screen, "\x1b[2;1Ha\u{308}");
    assert_eq!(screen.grapheme_at(Position::new(0, 1)), None);
    screen.buffer.layers[0].properties.is_locked = false;
    screen.buffer.layers[0].properties.has_alpha_channel = true;
    screen.buffer.layers[0].properties.is_alpha_channel_locked = true;
    feed(&mut screen, "\x1b[3;1H\u{754c}");
    assert_eq!(screen.grapheme_at(Position::new(0, 2)), None);
}

#[test]
fn unicode_cells_ansi_erase_respects_cursor_range() {
    let mut screen = unicode_screen(8, 4);
    feed(&mut screen, "A\u{754c}B\x1b[1;2H\x1b[1K");
    assert_eq!(screen.char_at(Position::new(1, 0)).ch, ' ');
    assert_eq!(screen.char_at(Position::new(2, 0)).ch, ' ');
    assert_eq!(screen.char_at(Position::new(3, 0)).ch, 'B');
    feed(&mut screen, "\x1b[HA\u{754c}B\x1b[1;3H\x1b[J");
    assert_eq!(screen.char_at(Position::new(0, 0)).ch, 'A');
    assert_eq!(screen.char_at(Position::new(1, 0)).ch, ' ');
    assert_eq!(screen.char_at(Position::new(3, 0)).ch, ' ');
}

#[test]
fn unicode_cells_noop_cursor_commands_break_attachment() {
    let mut screen = unicode_screen(1, 2);
    feed(&mut screen, "\x1b[?7la");
    screen.cr();
    feed(&mut screen, "\u{308}");
    assert_eq!(screen.grapheme_at(Position::new(0, 0)), Some(("\u{25cc}\u{308}", 1)));
}
