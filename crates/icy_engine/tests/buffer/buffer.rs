use icy_engine::{
    BufferParser, Caret, Position, EditableScreen, TextAttribute, TextBuffer, TextPane, TextScreen, SelectionMask,
    parsers::ansi,
};

fn create_buffer<T: BufferParser>(parser: &mut T, input: &[u8]) -> (TextBuffer, Caret) {
    let mut screen = TextScreen {
        buffer: TextBuffer::create((80, 25)),
        caret: Caret::default(),
        current_layer: 0,
        selection_opt: None,
        selection_mask: SelectionMask::default(),
        mouse_fields: Vec::new(),
    };

    screen.terminal_state_mut().is_terminal_buffer = true;
    screen.buffer.layers.first_mut().unwrap().lines.clear();

    for b in input {
        parser.print_char(&mut screen, *b as char).unwrap();
    }

    while parser.get_next_action(&mut screen).is_some() {}

    (screen.buffer, screen.caret)
}

fn update_buffer<T: BufferParser>(buf: &mut TextBuffer, caret: &mut Caret, parser: &mut T, input: &[u8]) {
    let mut screen = TextScreen {
        buffer: std::mem::take(buf),
        caret: caret.clone(),
        current_layer: 0,
        selection_opt: None,
        selection_mask: SelectionMask::default(),
        mouse_fields: Vec::new(),
    };

    screen.terminal_state_mut().is_terminal_buffer = true;

    for b in input {
        parser.print_char(&mut screen, *b as char).unwrap();
    }

    while parser.get_next_action(&mut screen).is_some() {}

    *buf = screen.buffer;
    *caret = screen.caret;
}

#[test]
fn test_bs() {
    let (buf, caret) = create_buffer(&mut ansi::Parser::default(), b"\x1b[1;43mtest\x08\x08\x08\x08");
    assert_eq!(Position::default(), caret.pos);
    for i in 0..4 {
        assert_eq!(
            TextAttribute::from_color(15, 6),
            buf.get_char(Position::new(i, 0)).unwrap().attribute
        );
    }
}

#[test]
fn test_up() {
    let (mut buf, mut caret) = create_buffer(&mut ansi::Parser::default(), b"\x1b[10;10H");
    assert_eq!(9, caret.pos.y);
    caret.up(&mut buf, 100);
    assert_eq!(0, caret.pos.y);
}
/*
#[test]
fn test_down() {
    let (mut buf, mut caret) = create_buffer(&mut ansi::Parser::default(), b"\x1b[10;10H");
    assert_eq!(9, caret.pos.y);
    caret.down(&mut buf, 100);
    assert_eq!(24, caret.pos.y);
} */

#[test]
fn test_lf_beyond_terminal_height() {
    let (mut buf, mut caret) = create_buffer(&mut ansi::Parser::default(), b"");
    for _ in 0..30 {
        caret.lf(&mut buf);
    }
    assert_eq!(30, caret.pos.y);
    assert_eq!(6, buf.get_first_visible_line());
}

#[test]
fn test_margin_up() {
    let (mut buf, mut caret) = create_buffer(&mut ansi::Parser::default(), b"\x1b[10;10H");
    assert_eq!(9, caret.pos.y);
    caret.up(&mut buf, 100);
    assert_eq!(0, caret.pos.y);
}

#[test]
fn test_margin_scroll_up() {
    let (mut buf, mut caret) = create_buffer(&mut ansi::Parser::default(), b"\x1B[1;25r1\n2\n3\n4\n");
    caret.up(&mut buf, 5);
    assert_eq!(0, caret.pos.y);
    assert_eq!('1', buf.get_char(Position::new(0, 1)).ch);
}

#[test]
fn test_margin_scroll_down_bug() {
    let (mut buf, mut caret) =
        create_buffer(&mut ansi::Parser::default(), b"1\x1b[5;19r\x1b[17;1Hfoo\nbar");

    let ch = buf.get_char(Position::new(0, 16));
    assert_eq!(b'f', ch.ch as u8);
    let ch = buf.get_char(Position::new(0, 17));
    assert_eq!(b'b', ch.ch as u8);

    assert_eq!(17, caret.pos.y);
    update_buffer(
        &mut buf,
        &mut caret,
        &mut ansi::Parser::default(),
        b"\x1B[19H\r\n",
    );
    update_buffer(
        &mut buf,
        &mut caret,
        &mut ansi::Parser::default(),
        b"\x1B[19H\r\n",
    );
    update_buffer(
        &mut buf,
        &mut caret,
        &mut ansi::Parser::default(),
        b"\x1B[19H\r\n",
    );

    assert_eq!(18, caret.pos.y);

    let ch = buf.get_char(Position::new(0, 16 - 3));
    assert_eq!(b'f', ch.ch as u8);
    let ch = buf.get_char(Position::new(0, 17 - 3));
    assert_eq!(b'b', ch.ch as u8);
}

#[test]
fn test_clear_screen_reset() {
    let (mut buf, mut caret) = create_buffer(&mut ansi::Parser::default(), b"");
    for _ in 0..100 {
        caret.lf(&mut buf);
    }
    buf.clear_screen(&mut caret);
    assert_eq!(Position::default(), caret.pos);
    assert_eq!(0, buf.get_first_visible_line());
}
/*
#[test]
fn test_margins_scroll_down() {
    let (buf, _) = create_buffer(&mut ansi::Parser::default(), b"\x1B[2J\n\n\n\x1B[4;8rTEST\r\nt1\r\nt2\r\nt3\r\nt4\r\nt5\r\nt6\r\nt7\r\nt8\r\nt9\r\nt10\r\nt11");
    assert_eq!(
        "TEST\r\nt1\r\nt2\r\nt7\r\nt8\r\nt9\r\nt10\r\nt11\r\n",
        get_string_from_buffer(&buf)
    );
}

#[test]
fn test_margins_edit_outofarea() {
    let (buf, _) = create_buffer(
        &mut ansi::Parser::default(),
        b"\x1B[2J\n\n\n\x1B[4;8r\x1B[4;1H1\r\n2\r\n3\r\n4\x1B[9;1H1\r\n2\r\n3\r\n4",
    );
    assert_eq!(
        "\r\n\r\n\r\n1\r\n2\r\n3\r\n4\r\n\r\n1\r\n2\r\n3\r\n4",
        get_string_from_buffer(&buf)
    );
}

#[test]
fn test_margins_scrolling() {
    let (buf, _) = create_buffer(&mut ansi::Parser::default(), b"\x1B[2J\x1B[5;19r\x1B[19H1\x1B[19H\r\n2\x1B[19H\r\n3\x1B[19H\r\n4\x1B[19H\r\n5\x1B[19H\r\n");
    assert_eq!(
        "\r\n\r\n\r\n\r\n\r\n\r\n\r\n\r\n\r\n\r\n\r\n\r\n\r\n1\r\n2\r\n3\r\n4\r\n5\r\n\r\n",
        get_string_from_buffer(&buf)
    );
}

#[test]
fn test_margins_scrolling2() {
    let (mut buf, mut caret) = create_buffer(&mut ansi::Parser::default(), b"\x1B[2J\x1B[5;19r\x1B[5;1H");

    for i in 5..19 {
        update_buffer(
            &mut buf,
            &mut caret,
            &mut ansi::Parser::default(),
            format!("{i}\n\r").as_bytes(),
        );
    }

    assert_eq!("\r\n\r\n\r\n\r\n5\r\n6\r\n7\r\n8\r\n9\r\n10\r\n11\r\n12\r\n13\r\n14\r\n15\r\n16\r\n17\r\n18\r\n", get_string_from_buffer(&buf));
}
*/
#[test]
fn test_margins_clear_line_bug() {
    // insertion of the last 'e' character clears the 2nd line
    let (mut buf, mut caret) = create_buffer(&mut ansi::Parser::default(), b"\x1B[2;21r\x1B[4l\x1B[2H\x1B[20L\r\n\x1B[4h\x1B[2H0123456789012345678901234567890123456789012345678901234567890123456789012345678\n\x1B[L\x1B[79D0123456789012345678901234567890123456789012345678901234567890123456789012345678\x1B[A\x1B[D\x1B[D\x1B[D\x1B[D\x1B[D\x1B[P");
    assert_eq!('0', buf.get_char(Position::new(0, 2)).ch);
    update_buffer(&mut buf, &mut caret, &mut ansi::Parser::default(), b"e");
    assert_eq!('0', buf.get_char(Position::new(0, 2)).ch);
}

#[test]
fn test_clear_buffer_down() {
    let (mut buf, mut caret) = create_buffer(
        &mut ansi::Parser::default(),
        b"\x1B[2J\x1B[5;19r\x1B[25;1H1\x1B[1;1H",
    );
    assert_eq!('1', buf.get_char(Position::new(0, 24)).ch);
    update_buffer(&mut buf, &mut caret, &mut ansi::Parser::default(), b"\x1B[J");
    assert_eq!(' ', buf.get_char(Position::new(0, 24)).ch);
}

#[test]
fn test_clear_buffer_up() {
    let (mut buf, mut caret) =
        create_buffer(&mut ansi::Parser::default(), b"\x1B[2J1\x1B[5;19r\x1B[25;1H");
    assert_eq!('1', buf.get_char(Position::new(0, 0)).ch);
    update_buffer(&mut buf, &mut caret, &mut ansi::Parser::default(), b"\x1B[1J");
    assert_eq!(' ', buf.get_char(Position::new(0, 0)).ch);
}
