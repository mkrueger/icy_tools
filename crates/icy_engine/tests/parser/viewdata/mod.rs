use icy_engine::parsers::viewdata::Parser;
use icy_engine::{BufferParser, Caret, Position, SelectionMask, TextBuffer, TextPane, TextScreen};

fn create_viewdata_buffer<T: BufferParser>(parser: &mut T, input: &[u8]) -> (TextBuffer, Caret) {
    let mut screen = TextScreen {
        buffer: TextBuffer::create((40, 24)),
        caret: Caret::default(),
        current_layer: 0,
        selection_opt: None,
        selection_mask: SelectionMask::default(),
        mouse_fields: Vec::new(),
    };
    screen.buffer.terminal_state.is_terminal_buffer = true;

    for &b in input {
        parser.print_char(&mut screen, b as char).unwrap();
    }

    while parser.get_next_action(&mut screen).is_some() {}

    (screen.buffer, screen.caret)
}

#[test]
fn test_bs() {
    let (_, caret) = create_viewdata_buffer(&mut Parser::default(), b"ab\x08");
    assert_eq!(Position::new(1, 0), caret.position());

    let (buf, caret) = create_viewdata_buffer(&mut Parser::default(), b"\x08");
    assert_eq!(Position::new(buf.get_width() - 1, 23), caret.position());
}

#[test]
fn test_ht() {
    let (_, caret) = create_viewdata_buffer(&mut Parser::default(), b"\x09");
    assert_eq!(Position::new(1, 0), caret.position());

    let (_, caret) = create_viewdata_buffer(&mut Parser::default(), b"\x08\x09");
    assert_eq!(Position::new(0, 0), caret.position());
}

#[test]
fn test_lf() {
    let (_, caret) = create_viewdata_buffer(&mut Parser::default(), b"test\x0A");
    assert_eq!(Position::new(4, 1), caret.position());

    let (_, caret) = create_viewdata_buffer(&mut Parser::default(), b"\x0B\x0A");
    assert_eq!(Position::new(0, 0), caret.position());
}

#[test]
fn test_vt() {
    let (_, caret) = create_viewdata_buffer(&mut Parser::default(), b"\n\n\x0B");
    assert_eq!(Position::new(0, 1), caret.position());

    let (buf, caret) = create_viewdata_buffer(&mut Parser::default(), b"\x0B");
    assert_eq!(Position::new(0, buf.get_height() - 1), caret.position());
}

#[test]
fn test_ff() {
    let (buf, caret) = create_viewdata_buffer(&mut Parser::default(), b"test\x0C");
    assert_eq!(Position::new(0, 0), caret.position());
    assert_eq!(' ', buf.get_char(Position::new(0, 0)).ch);
}

#[test]
fn test_set_fg_color() {
    let (_, caret) = create_viewdata_buffer(&mut Parser::default(), b"\x1BA");
    assert_eq!(1, caret.attribute.get_foreground());
}

#[test]
fn test_set_bg_color() {
    let (_, caret) = create_viewdata_buffer(&mut Parser::default(), b"\x1BA\x1B]");
    assert_eq!(1, caret.attribute.get_background());
}

#[test]
fn test_set_black_bg_color() {
    let (_, caret) = create_viewdata_buffer(&mut Parser::default(), b"\x1BA\x1B]\x1B\\");
    assert_eq!(0, caret.attribute.get_background());
}

#[test]
fn test_set_flash() {
    let (_, caret) = create_viewdata_buffer(&mut Parser::default(), b"\x1BH");
    assert!(caret.attribute.is_blinking());
}

#[test]
fn test_reset_flash() {
    let (_, caret) = create_viewdata_buffer(&mut Parser::default(), b"\x1BH\x1BI");
    assert!(!caret.attribute.is_blinking());
}

#[test]
fn test_set_double_height() {
    let (_, caret) = create_viewdata_buffer(&mut Parser::default(), b"\x1BM");
    assert!(caret.attribute.is_double_height());
}

#[test]
fn test_reset_double_height() {
    let (_, caret) = create_viewdata_buffer(&mut Parser::default(), b"\x1BM\x1BL");
    assert!(!caret.attribute.is_double_height());
}

#[test]
fn test_conceal() {
    let (_, caret) = create_viewdata_buffer(&mut Parser::default(), b"\x1BX");
    assert!(caret.attribute.is_concealed());
}

#[test]
fn test_line_lose_color_bug() {
    let (buf, _) = create_viewdata_buffer(&mut Parser::default(), b"\x1BAfoo\x1BBbar\x1E\x1E");
    assert_eq!(1, buf.get_char(Position::new(1, 0)).attribute.get_foreground());
}

#[test]
fn testpage_bug_1() {
    let (buf, _) = create_viewdata_buffer(&mut Parser::default(), b"\x1BT\x1BZ\x1B^s\x1BQ\x1BY\x1BU\x1B@\x1BU\x1BA\x1BM");
    assert_eq!(' ', buf.get_char(Position::new(10, 0)).ch);
}

#[test]
fn testpage_bug_2() {
    // bg color changes immediately
    let (buf, _) = create_viewdata_buffer(&mut Parser::default(), b"\x1BM \x1BE\x1B]\x1BBT");
    assert_eq!(5, buf.get_char(Position::new(3, 0)).attribute.get_background());
}

#[test]
fn testpage_bug_3() {
    // bg reset color changes immediately
    let (buf, _) = create_viewdata_buffer(&mut Parser::default(), b"\x1BM \x1BE\x1B]\x1BBT\x1B\\X");
    assert_eq!(0, buf.get_char(Position::new(6, 0)).attribute.get_background());
    assert_eq!(0, buf.get_char(Position::new(7, 0)).attribute.get_background());
}

#[test]
fn testpage_bug_4() {
    // conceal has no effect in graphics mode
    let (buf, _) = create_viewdata_buffer(&mut Parser::default(), b"\x1B^\x1BRs\x1BV\x1BX\x1BS\x1B@\x1BW\x1BX\x1BA05");
    for i in 0..10 {
        assert!(!buf.get_char(Position::new(i, 0)).attribute.is_concealed());
    }
}

#[test]
fn test_cr_at_eol() {
    // conceal has no effect in graphics mode
    let (buf, _) = create_viewdata_buffer(&mut Parser::default(), b"\x1BA\x1BA\x1BA\x1BA\x1BA\x1BA\x1BA\x1BA\x1BA\x1BA\x1BA\x1BA\x1BA\x1BA\x1BA\x1BA\x1BA\x1BA\x1BA\x1BA\x1BA\x1BA\x1BA\x1BA\x1BA\x1BA\x1BA\x1BA\x1BA\x1BA\x1BA\x1BA\x1BA\x1BA\x1BA\x1BA\x1BA\x1BA01\x1B\x08\r");
    for x in 1..buf.get_width() {
        assert_eq!(1, buf.get_char((x, 0).into()).attribute.get_foreground(), "wrong color at {x}");
    }
}

#[test]
fn test_lf_fill_bg_bug() {
    // conceal has no effect in graphics mode
    let (buf, _) = create_viewdata_buffer(&mut Parser::default(), b"\x1BD\x1B] \x1B\\\r\n");
    assert_eq!(0, buf.get_char(Position::new(5, 0)).attribute.get_background());
}

#[test]
fn test_drop_shadow() {
    // conceal has no effect in graphics mode
    let (buf, _) = create_viewdata_buffer(&mut Parser::default(), b"\x1B^\x1BT\x1B]\x1BGDrop Shadow\x1BTk\x1BV\x1B\\\x7F\x7F");
    assert_eq!('«', buf.get_char((18, 0).into()).ch);
    assert_eq!(0, buf.get_char(Position::new(18, 0)).attribute.get_background());
}

#[test]
fn test_color_on_clreol() {
    // conceal has no effect in graphics mode
    let (buf, _) = create_viewdata_buffer(
        &mut Parser::default(),
        b"\x1E\x0B\x1BAACCESS DENIED.\x11\x1E\n\n\n\n\n\n\n\n\n\n\n\n\n\n\n\n\n\n\n\n\n\n\n\n\x1E\x0B\x1BB*1\x14\x1E\x09\n",
    );
    assert_eq!(2, buf.get_char(Position::new(3, buf.get_height() - 1)).attribute.get_foreground());
}

#[test]
fn test_caret_visibility() {
    let (_, caret) = create_viewdata_buffer(&mut Parser::default(), b"\x14\n\n\n");
    assert!(!caret.visible);

    let (_, caret) = create_viewdata_buffer(&mut Parser::default(), b"\x14\n\n\n\x11");
    assert!(caret.visible);

    let (_, caret) = create_viewdata_buffer(&mut Parser::default(), b"\x14\x0C");
    assert!(!caret.visible);
}
