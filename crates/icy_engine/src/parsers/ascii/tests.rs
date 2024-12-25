use crate::{
    parsers::{ascii::Parser, create_buffer, update_buffer},
    OutputFormat, Position, SaveOptions, TextPane,
};

fn test_ascii(data: &[u8]) {
    let (buf, _) = create_buffer(&mut Parser::default(), data);
    let converted = crate::Ascii::default().to_bytes(&buf, &SaveOptions::new()).unwrap();

    // more gentle output.
    let b: Vec<u8> = converted.iter().map(|&x| if x == 27 { b'x' } else { x }).collect();
    let converted = String::from_utf8_lossy(b.as_slice());

    let b: Vec<u8> = data.iter().map(|&x| if x == 27 { b'x' } else { x }).collect();
    let expected = String::from_utf8_lossy(b.as_slice());

    assert_eq!(expected, converted);
}

#[test]
fn test_full_line_height() {
    let mut vec = Vec::new();
    vec.resize(80, b'-');
    let (mut buf, mut caret) = create_buffer(&mut Parser::default(), &vec);
    assert_eq!(2, buf.get_line_count());
    vec.push(b'-');
    update_buffer(&mut buf, &mut caret, &mut Parser::default(), &vec);
    assert_eq!(3, buf.get_line_count());
}

#[test]
fn test_emptylastline_height() {
    let mut vec = Vec::new();
    vec.resize(80, b'-');
    vec.resize(80 * 2, b' ');
    let (buf, _) = create_buffer(&mut Parser::default(), &vec);
    assert_eq!(3, buf.get_line_count());
}

/*
#[test]
fn test_emptylastline_roundtrip() {
    let mut vec = Vec::new();
    vec.resize(80, b'-');
    vec.resize(80 * 2, b' ');

    let (buf, _) = create_buffer(&mut AsciiParser::new(), &vec);
    assert_eq!(2, buf.get_real_buffer_height());
    let vec2 = buf.to_bytes("asc", &SaveOptions::new()).unwrap();
    let (buf2, _) = create_buffer(&mut AsciiParser::new(), &vec2);
    assert_eq!(2, buf2.get_real_buffer_height());
}

 */
#[test]
fn test_eol() {
    let data = b"foo\r\n";
    let (buf, _) = create_buffer(&mut Parser::default(), data);
    assert_eq!(2, buf.get_line_count());
}

/*
#[test]
fn test_ws_skip() {
    let data = b"123456789012345678901234567890123456789012345678901234567890123456789012345678902ndline";
    test_ascii(data);
}

#[test]
fn test_ws_skip_empty_line() {
    let data = b"12345678901234567890123456789012345678901234567890123456789012345678901234567890\r\n\r\n2ndline";
    test_ascii(data);
}
*/
#[test]
fn test_eol_start() {
    let data = b"\r\n2ndline";
    test_ascii(data);
}

#[test]
fn test_eol_line_break() {
    let (mut buf, mut caret) = create_buffer(
        &mut Parser::default(),
        b"################################################################################\r\n",
    );
    assert_eq!(Position::new(0, 2), caret.pos);

    update_buffer(&mut buf, &mut caret, &mut Parser::default(), b"#");
    assert_eq!(Position::new(1, 2), caret.pos);
    assert_eq!(b'#', buf.get_char(Position::new(0, 2)).ch as u8);
}

#[test]
fn test_url_scanner_simple() {
    let (buf, _) = create_buffer(&mut Parser::default(), b"\n\r http://www.example.com");

    let hyperlinks = buf.parse_hyperlinks();

    assert_eq!(1, hyperlinks.len());
    assert_eq!("http://www.example.com", hyperlinks[0].get_url(&buf));
    assert_eq!(Position::new(1, 1), hyperlinks[0].position);
}

#[test]
fn test_url_scanner_multiple() {
    let (buf, _) = create_buffer(
        &mut Parser::default(),
        b"\n\r http://www.example.com https://www.google.com\n\rhttps://github.com/mkrueger/icy_engine",
    );

    let hyperlinks = buf.parse_hyperlinks();

    assert_eq!(3, hyperlinks.len());
    assert_eq!("http://www.example.com", hyperlinks[2].get_url(&buf));
    assert_eq!(Position::new(1, 1), hyperlinks[2].position);

    assert_eq!("https://www.google.com", hyperlinks[1].get_url(&buf));
    assert_eq!(Position::new(24, 1), hyperlinks[1].position);

    assert_eq!("https://github.com/mkrueger/icy_engine", hyperlinks[0].get_url(&buf));
    assert_eq!(Position::new(0, 2), hyperlinks[0].position);
}

#[test]
fn test_tab() {
    let data = b"\ta";
    let (buf, _) = create_buffer(&mut Parser::default(), data);
    assert_eq!(b'a', buf.get_char(Position::new(8, 0)).ch as u8);
}
